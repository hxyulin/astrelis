//! Configurable layout engine supporting synchronous and asynchronous computation.
//!
//! The layout engine provides:
//! - Synchronous mode: Layout computed on main thread (current behavior)
//! - Asynchronous mode: Layout computed on background thread with double-buffering
//! - Hybrid mode: Small subtrees sync, large subtrees async
//!
//! # Architecture
//!
//! In async mode, the engine maintains two layout caches:
//! - Front buffer: Read by renderer (last completed layout)
//! - Back buffer: Written by worker thread (in-progress layout)
//!
//! When layout completes, buffers are swapped atomically.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::layout_engine::{LayoutEngine, LayoutMode};
//!
//! let mut engine = LayoutEngine::new(LayoutMode::Asynchronous {
//!     max_stale_frames: 2,
//! });
//!
//! // Request layout computation
//! engine.request_layout(&tree, viewport_size);
//!
//! // In render loop: get current layout (may be slightly stale in async mode)
//! let layout = engine.get_layout(node_id);
//!
//! // Poll for completed async results
//! let completed = engine.poll_results();
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use astrelis_core::geometry::Size;
use astrelis_core::profiling::profile_function;

use crate::plugin::registry::WidgetTypeRegistry;
use crate::tree::{LayoutRect, NodeId, UiTree};

/// Return type for `spawn_worker`: (request sender, result receiver, thread handle).
type WorkerChannels = (
    Option<std::sync::mpsc::Sender<WorkerMessage>>,
    Option<std::sync::mpsc::Receiver<LayoutResult>>,
    Option<JoinHandle<()>>,
);

/// Layout computation mode.
#[derive(Debug, Clone, Default)]
pub enum LayoutMode {
    /// Compute layout synchronously on the main thread.
    /// This is the default and simplest mode.
    #[default]
    Synchronous,

    /// Compute layout asynchronously on a background thread.
    /// Layout results may be 1-N frames stale.
    Asynchronous {
        /// Maximum number of frames layout can be stale before
        /// falling back to synchronous computation.
        max_stale_frames: u32,
    },

    /// Hybrid mode: Use sync for small subtrees, async for large ones.
    Hybrid {
        /// Node count threshold for async processing.
        async_threshold: usize,
        /// Maximum stale frames for async portion.
        max_stale_frames: u32,
    },
}

/// Snapshot of node data for async layout computation.
#[derive(Debug, Clone)]
pub struct NodeSnapshot {
    /// Node identifier.
    pub node_id: NodeId,
    /// Node's Taffy style.
    pub style: taffy::Style,
    /// Parent node (if any).
    pub parent: Option<usize>,
    /// Child node indices.
    pub children: Vec<usize>,
    /// Whether measurement results should be cached for this widget type.
    pub caches_measurement: bool,
    /// Cached text measurement (width, height) if available.
    pub text_measurement: Option<(f32, f32)>,
}

/// Complete snapshot of tree state for async layout.
#[derive(Debug, Clone)]
pub struct TreeSnapshot {
    /// All nodes in the snapshot.
    pub nodes: Vec<NodeSnapshot>,
    /// Root node index.
    pub root: Option<usize>,
    /// Set of dirty node indices.
    pub dirty_nodes: Vec<usize>,
    /// Mapping from NodeId to index.
    pub id_to_index: HashMap<NodeId, usize>,
}

impl TreeSnapshot {
    /// Create a snapshot from a UiTree.
    pub fn from_tree(tree: &UiTree, widget_registry: &WidgetTypeRegistry) -> Self {
        let mut nodes = Vec::new();
        let mut id_to_index = HashMap::new();
        let mut dirty_nodes = Vec::new();

        // First pass: collect all nodes
        for (node_id, node) in tree.iter() {
            let index = nodes.len();
            id_to_index.insert(node_id, index);

            if !node.dirty_flags.is_empty() {
                dirty_nodes.push(index);
            }

            // Check if this widget type caches measurements (via registry)
            let caches_measurement =
                widget_registry.caches_measurement(node.widget.as_any().type_id());

            nodes.push(NodeSnapshot {
                node_id,
                style: node.widget.style().layout.clone(),
                parent: None,         // Will be set in second pass
                children: Vec::new(), // Will be set in second pass
                caches_measurement,
                text_measurement: node.text_measurement,
            });
        }

        // Second pass: set parent/child relationships
        for (node_id, node) in tree.iter() {
            if let Some(&index) = id_to_index.get(&node_id) {
                // Set parent
                if let Some(parent_id) = node.parent
                    && let Some(&parent_index) = id_to_index.get(&parent_id)
                {
                    nodes[index].parent = Some(parent_index);
                }

                // Set children
                let child_indices: Vec<usize> = node
                    .children
                    .iter()
                    .filter_map(|child_id| id_to_index.get(child_id).copied())
                    .collect();
                nodes[index].children = child_indices;
            }
        }

        let root = tree.root().and_then(|id| id_to_index.get(&id).copied());

        Self {
            nodes,
            root,
            dirty_nodes,
            id_to_index,
        }
    }

    /// Get node count.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Request for layout computation.
#[derive(Debug)]
pub struct LayoutRequest {
    /// Tree snapshot to compute layout for.
    tree_snapshot: TreeSnapshot,
    /// Viewport size.
    viewport_size: Size<f32>,
    /// Frame ID for this request.
    frame_id: u64,
    /// Timestamp when request was made.
    _timestamp: Instant,
}

/// Result of layout computation.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Frame ID this result is for.
    pub frame_id: u64,
    /// Computed layouts by node ID.
    pub layouts: HashMap<NodeId, LayoutRect>,
    /// Computation time.
    pub compute_time: std::time::Duration,
    /// Whether this was a full or partial update.
    pub is_partial: bool,
}

/// Cache for layout results (double-buffered).
struct LayoutCache {
    /// Primary layout data (read by renderer).
    front: RwLock<HashMap<NodeId, LayoutRect>>,
    /// Secondary layout data (written by worker).
    back: Mutex<HashMap<NodeId, LayoutRect>>,
    /// Frame ID of front buffer.
    front_frame_id: AtomicU64,
    /// Whether a swap is pending.
    swap_pending: AtomicBool,
}

impl LayoutCache {
    fn new() -> Self {
        Self {
            front: RwLock::new(HashMap::new()),
            back: Mutex::new(HashMap::new()),
            front_frame_id: AtomicU64::new(0),
            swap_pending: AtomicBool::new(false),
        }
    }

    /// Get layout from front buffer.
    fn get(&self, node_id: NodeId) -> Option<LayoutRect> {
        self.front.read().ok()?.get(&node_id).copied()
    }

    /// Write layout to back buffer.
    fn write_back(&self, node_id: NodeId, layout: LayoutRect) {
        if let Ok(mut back) = self.back.lock() {
            back.insert(node_id, layout);
        }
    }

    /// Swap front and back buffers.
    fn swap(&self, frame_id: u64) {
        if let (Ok(mut front), Ok(mut back)) = (self.front.write(), self.back.lock()) {
            std::mem::swap(&mut *front, &mut *back);
            self.front_frame_id.store(frame_id, Ordering::SeqCst);
            back.clear();
            self.swap_pending.store(false, Ordering::SeqCst);
        }
    }

    /// Mark swap as pending.
    fn mark_swap_pending(&self) {
        self.swap_pending.store(true, Ordering::SeqCst);
    }

    /// Check if swap is pending.
    fn is_swap_pending(&self) -> bool {
        self.swap_pending.load(Ordering::SeqCst)
    }

    /// Get frame ID of front buffer.
    fn front_frame_id(&self) -> u64 {
        self.front_frame_id.load(Ordering::SeqCst)
    }
}

/// Message types for worker thread.
enum WorkerMessage {
    /// Request layout computation.
    Compute(LayoutRequest),
    /// Shut down the worker.
    Shutdown,
}

/// Configurable layout engine.
pub struct LayoutEngine {
    /// Current layout mode.
    mode: LayoutMode,
    /// Double-buffered layout cache.
    cache: Arc<LayoutCache>,
    /// Current frame ID.
    frame_id: u64,
    /// Frame ID of last completed layout.
    last_completed_frame: u64,

    // Async mode fields
    /// Sender for layout requests.
    request_sender: Option<std::sync::mpsc::Sender<WorkerMessage>>,
    /// Receiver for layout results.
    result_receiver: Option<std::sync::mpsc::Receiver<LayoutResult>>,
    /// Worker thread handle.
    worker_handle: Option<JoinHandle<()>>,
    /// Whether async layout is in progress.
    layout_in_progress: Arc<AtomicBool>,
}

impl LayoutEngine {
    /// Create a new layout engine with the specified mode.
    pub fn new(mode: LayoutMode) -> Self {
        let cache = Arc::new(LayoutCache::new());
        let layout_in_progress = Arc::new(AtomicBool::new(false));

        let (request_sender, result_receiver, worker_handle) = match &mode {
            LayoutMode::Synchronous => (None, None, None),
            LayoutMode::Asynchronous { .. } | LayoutMode::Hybrid { .. } => {
                Self::spawn_worker(cache.clone(), layout_in_progress.clone())
            }
        };

        Self {
            mode,
            cache,
            frame_id: 0,
            last_completed_frame: 0,
            request_sender,
            result_receiver,
            worker_handle,
            layout_in_progress,
        }
    }

    /// Spawn the layout worker thread.
    fn spawn_worker(cache: Arc<LayoutCache>, in_progress: Arc<AtomicBool>) -> WorkerChannels {
        let (request_tx, request_rx) = std::sync::mpsc::channel::<WorkerMessage>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<LayoutResult>();

        let handle = thread::Builder::new()
            .name("layout-worker".to_string())
            .spawn(move || {
                Self::worker_loop(request_rx, result_tx, cache, in_progress);
            })
            .expect("Failed to spawn layout worker thread");

        (Some(request_tx), Some(result_rx), Some(handle))
    }

    /// Worker thread main loop.
    fn worker_loop(
        request_rx: std::sync::mpsc::Receiver<WorkerMessage>,
        result_tx: std::sync::mpsc::Sender<LayoutResult>,
        cache: Arc<LayoutCache>,
        in_progress: Arc<AtomicBool>,
    ) {
        while let Ok(msg) = request_rx.recv() {
            match msg {
                WorkerMessage::Compute(request) => {
                    in_progress.store(true, Ordering::SeqCst);
                    let start = Instant::now();

                    // Perform layout computation
                    let layouts =
                        Self::compute_layout_sync(&request.tree_snapshot, request.viewport_size);

                    // Write results to back buffer
                    for (node_id, layout) in &layouts {
                        cache.write_back(*node_id, *layout);
                    }

                    // Mark swap pending
                    cache.mark_swap_pending();

                    let result = LayoutResult {
                        frame_id: request.frame_id,
                        layouts,
                        compute_time: start.elapsed(),
                        is_partial: false,
                    };

                    let _ = result_tx.send(result);
                    in_progress.store(false, Ordering::SeqCst);
                }
                WorkerMessage::Shutdown => break,
            }
        }
    }

    /// Compute layout synchronously from a snapshot.
    fn compute_layout_sync(
        snapshot: &TreeSnapshot,
        viewport_size: Size<f32>,
    ) -> HashMap<NodeId, LayoutRect> {
        let mut taffy = taffy::TaffyTree::new();
        let mut taffy_nodes: HashMap<usize, taffy::NodeId> = HashMap::new();
        let mut results = HashMap::new();

        // Build Taffy tree from snapshot
        for (index, node) in snapshot.nodes.iter().enumerate() {
            let taffy_node = if node.children.is_empty() {
                taffy.new_leaf(node.style.clone()).unwrap()
            } else {
                taffy.new_with_children(node.style.clone(), &[]).unwrap()
            };
            taffy_nodes.insert(index, taffy_node);
        }

        // Set up parent-child relationships
        for (index, node) in snapshot.nodes.iter().enumerate() {
            if !node.children.is_empty() {
                let children: Vec<taffy::NodeId> = node
                    .children
                    .iter()
                    .filter_map(|&child_idx| taffy_nodes.get(&child_idx).copied())
                    .collect();

                if let Some(&parent_taffy) = taffy_nodes.get(&index) {
                    let _ = taffy.set_children(parent_taffy, &children);
                }
            }
        }

        // Compute layout
        if let Some(root_idx) = snapshot.root
            && let Some(&root_taffy) = taffy_nodes.get(&root_idx)
        {
            let available = taffy::Size {
                width: taffy::AvailableSpace::Definite(viewport_size.width),
                height: taffy::AvailableSpace::Definite(viewport_size.height),
            };

            // Simple measure function using cached measurements
            let measure_fn = |known_dimensions: taffy::Size<Option<f32>>,
                              _available_space: taffy::Size<taffy::AvailableSpace>,
                              node_id: taffy::NodeId,
                              _node_context: Option<&mut ()>,
                              _style: &taffy::Style|
             -> taffy::Size<f32> {
                // Find the snapshot node for this taffy node
                for (idx, &tn) in &taffy_nodes {
                    if tn == node_id
                        && let Some(node) = snapshot.nodes.get(*idx)
                        && let Some((w, h)) = node.text_measurement
                    {
                        return taffy::Size {
                            width: known_dimensions.width.unwrap_or(w),
                            height: known_dimensions.height.unwrap_or(h),
                        };
                    }
                }
                taffy::Size {
                    width: known_dimensions.width.unwrap_or(0.0),
                    height: known_dimensions.height.unwrap_or(0.0),
                }
            };

            let _ = taffy.compute_layout_with_measure(root_taffy, available, measure_fn);
        }

        // Extract layouts
        for (index, node) in snapshot.nodes.iter().enumerate() {
            if let Some(&taffy_node) = taffy_nodes.get(&index)
                && let Ok(layout) = taffy.layout(taffy_node)
            {
                results.insert(
                    node.node_id,
                    LayoutRect {
                        x: layout.location.x,
                        y: layout.location.y,
                        width: layout.size.width,
                        height: layout.size.height,
                    },
                );
            }
        }

        results
    }

    /// Set the layout mode.
    pub fn set_mode(&mut self, mode: LayoutMode) {
        // Shut down existing worker if switching away from async
        if let Some(sender) = self.request_sender.take() {
            let _ = sender.send(WorkerMessage::Shutdown);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        self.result_receiver = None;

        // Spawn new worker if switching to async
        let (request_sender, result_receiver, worker_handle) = match &mode {
            LayoutMode::Synchronous => (None, None, None),
            LayoutMode::Asynchronous { .. } | LayoutMode::Hybrid { .. } => {
                Self::spawn_worker(self.cache.clone(), self.layout_in_progress.clone())
            }
        };

        self.mode = mode;
        self.request_sender = request_sender;
        self.result_receiver = result_receiver;
        self.worker_handle = worker_handle;
    }

    /// Get the current layout mode.
    pub fn mode(&self) -> &LayoutMode {
        &self.mode
    }

    /// Compute layout for the tree.
    ///
    /// In synchronous mode, this blocks until layout is complete.
    /// In async mode, this queues a layout request and returns immediately.
    pub fn compute_layout(
        &mut self,
        tree: &UiTree,
        viewport_size: Size<f32>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        profile_function!();
        self.frame_id += 1;

        match &self.mode {
            LayoutMode::Synchronous => {
                self.compute_layout_synchronous(tree, viewport_size, widget_registry);
            }
            LayoutMode::Asynchronous { max_stale_frames } => {
                let frames_stale = self.frame_id.saturating_sub(self.last_completed_frame);
                if frames_stale > *max_stale_frames as u64 {
                    // Too stale, fall back to sync
                    self.compute_layout_synchronous(tree, viewport_size, widget_registry);
                } else {
                    self.compute_layout_async(tree, viewport_size, widget_registry);
                }
            }
            LayoutMode::Hybrid {
                async_threshold,
                max_stale_frames,
            } => {
                let node_count = tree.iter().count();
                if node_count < *async_threshold {
                    self.compute_layout_synchronous(tree, viewport_size, widget_registry);
                } else {
                    let frames_stale = self.frame_id.saturating_sub(self.last_completed_frame);
                    if frames_stale > *max_stale_frames as u64 {
                        self.compute_layout_synchronous(tree, viewport_size, widget_registry);
                    } else {
                        self.compute_layout_async(tree, viewport_size, widget_registry);
                    }
                }
            }
        }
    }

    /// Compute layout synchronously.
    fn compute_layout_synchronous(
        &mut self,
        tree: &UiTree,
        viewport_size: Size<f32>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        let snapshot = TreeSnapshot::from_tree(tree, widget_registry);
        let layouts = Self::compute_layout_sync(&snapshot, viewport_size);

        // Write directly to front buffer
        if let Ok(mut front) = self.cache.front.write() {
            front.clear();
            front.extend(layouts);
        }

        self.cache
            .front_frame_id
            .store(self.frame_id, Ordering::SeqCst);
        self.last_completed_frame = self.frame_id;
    }

    /// Queue async layout computation.
    fn compute_layout_async(
        &mut self,
        tree: &UiTree,
        viewport_size: Size<f32>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        // Don't queue if already in progress
        if self.layout_in_progress.load(Ordering::SeqCst) {
            return;
        }

        if let Some(sender) = &self.request_sender {
            let snapshot = TreeSnapshot::from_tree(tree, widget_registry);
            let request = LayoutRequest {
                tree_snapshot: snapshot,
                viewport_size,
                frame_id: self.frame_id,
                _timestamp: Instant::now(),
            };
            let _ = sender.send(WorkerMessage::Compute(request));
        }
    }

    /// Poll for async layout results.
    ///
    /// Returns the number of results processed.
    pub fn poll_results(&mut self) -> usize {
        let mut count = 0;

        if let Some(receiver) = &self.result_receiver {
            while let Ok(result) = receiver.try_recv() {
                self.last_completed_frame = result.frame_id;
                count += 1;
            }
        }

        // Swap buffers if pending
        if self.cache.is_swap_pending() {
            self.cache.swap(self.last_completed_frame);
        }

        count
    }

    /// Get layout for a node.
    ///
    /// In async mode, this may return a slightly stale layout.
    pub fn get_layout(&self, node_id: NodeId) -> Option<LayoutRect> {
        self.cache.get(node_id)
    }

    /// Check if layout is current (not stale).
    pub fn is_layout_current(&self) -> bool {
        self.cache.front_frame_id() >= self.frame_id
    }

    /// Check if async layout is in progress.
    pub fn is_layout_in_progress(&self) -> bool {
        self.layout_in_progress.load(Ordering::SeqCst)
    }

    /// Get the number of frames layout is stale by.
    pub fn frames_stale(&self) -> u64 {
        self.frame_id.saturating_sub(self.cache.front_frame_id())
    }

    /// Clear the layout cache.
    pub fn clear(&mut self) {
        if let Ok(mut front) = self.cache.front.write() {
            front.clear();
        }
        if let Ok(mut back) = self.cache.back.lock() {
            back.clear();
        }
    }
}

impl Drop for LayoutEngine {
    fn drop(&mut self) {
        // Shut down worker thread
        if let Some(sender) = self.request_sender.take() {
            let _ = sender.send(WorkerMessage::Shutdown);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new(LayoutMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_mode_default() {
        let mode = LayoutMode::default();
        assert!(matches!(mode, LayoutMode::Synchronous));
    }

    #[test]
    fn test_tree_snapshot() {
        let registry = WidgetTypeRegistry::new();
        let mut tree = UiTree::new();
        let root = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let child = tree.add_widget(Box::new(crate::widgets::Container::new()));
        tree.add_child(root, child);
        tree.set_root(root);

        let snapshot = TreeSnapshot::from_tree(&tree, &registry);
        assert_eq!(snapshot.node_count(), 2);
        assert!(snapshot.root.is_some());
    }

    #[test]
    fn test_layout_engine_sync() {
        let registry = WidgetTypeRegistry::new();
        let mut engine = LayoutEngine::new(LayoutMode::Synchronous);

        let mut tree = UiTree::new();
        let root = tree.add_widget(Box::new(crate::widgets::Container::new()));
        tree.set_root(root);

        engine.compute_layout(&tree, Size::new(800.0, 600.0), &registry);

        assert!(engine.is_layout_current());
        assert!(!engine.is_layout_in_progress());
    }

    #[test]
    fn test_layout_engine_mode_switch() {
        let mut engine = LayoutEngine::new(LayoutMode::Synchronous);
        assert!(matches!(engine.mode(), LayoutMode::Synchronous));

        engine.set_mode(LayoutMode::Asynchronous {
            max_stale_frames: 2,
        });
        assert!(matches!(engine.mode(), LayoutMode::Asynchronous { .. }));

        engine.set_mode(LayoutMode::Synchronous);
        assert!(matches!(engine.mode(), LayoutMode::Synchronous));
    }

    #[test]
    fn test_layout_cache() {
        let cache = LayoutCache::new();
        let node_id = NodeId(1);
        let layout = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };

        // Write to back buffer
        cache.write_back(node_id, layout);

        // Not in front buffer yet
        assert!(cache.get(node_id).is_none());

        // Swap
        cache.mark_swap_pending();
        cache.swap(1);

        // Now in front buffer
        let result = cache.get(node_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().width, 100.0);
    }

    #[test]
    fn test_frames_stale() {
        let registry = WidgetTypeRegistry::new();
        let mut engine = LayoutEngine::new(LayoutMode::Synchronous);

        let mut tree = UiTree::new();
        let root = tree.add_widget(Box::new(crate::widgets::Container::new()));
        tree.set_root(root);

        engine.compute_layout(&tree, Size::new(800.0, 600.0), &registry);
        assert_eq!(engine.frames_stale(), 0);
    }
}
