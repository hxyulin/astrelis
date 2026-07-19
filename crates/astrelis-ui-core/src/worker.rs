//! Background text-shaping worker and the public async-shaping API.
//!
//! Shaping (BiDi, itemization, font fallback, kerning) is the costly half of
//! text layout. Under [`ShapePolicy::Async`] the `Ui` offloads eligible
//! reshapes to a single background thread, keeping the previous `TextLayout` on
//! screen until the worker delivers the new one, so a burst of text changes no
//! longer stalls event handling or presentation on the main thread.
//!
//! # Why a single worker owning its own fonts
//!
//! [`TextLayoutContext::layout`] needs `&mut FontDatabase` (parley's ranged
//! builder takes `&mut FontContext`, and fontique collection queries are
//! `&mut self`), so a `FontDatabase` cannot be shared across threads behind an
//! `Arc`. Instead the worker builds its *own* `FontDatabase` from a factory the
//! application supplies, and that database never crosses the thread boundary —
//! it is born, used, and dropped entirely on the worker. Only the
//! already-`Send` [`TextLayoutRequest`] and [`TextLayout`] travel over the
//! channels. A worker *pool* would need fontique's `shared: true` collections
//! and is left to a later phase.
//!
//! The factory must reconstruct a database that shapes byte-identically to the
//! main thread's, or text would visibly shift when a node toggles between the
//! sync (focused/force-synced) and async paths.

use std::sync::mpsc::{Receiver, Sender};

use astrelis_text::FontDatabase;

use super::*;

/// A unit of work sent to the shaping worker.
///
/// On wasm the worker is never spawned, so these are only ever constructed by
/// the native path; the allow keeps the wasm build (which still compiles the
/// enqueue site) warning-free until the web-worker backend lands.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) enum WorkerJob {
    /// Shape `request` for `id`; the result is tagged with `request_id` so a
    /// superseded reshape can be dropped on arrival.
    Shape {
        /// The node the result belongs to.
        id: ElementId,
        /// Identifies this reshape so a stale result can be discarded.
        request_id: RequestId,
        /// The shaping input.
        request: TextLayoutRequest,
    },
    /// Ask the worker to exit its loop; sent when the `Ui` is dropped.
    Stop,
}

/// A completed reshape returned by the worker.
pub(crate) struct WorkerDone {
    /// The node the result belongs to.
    pub(crate) id: ElementId,
    /// The request this result was shaped for.
    pub(crate) request_id: RequestId,
    /// The shaped layout, or the shaping error rendered as a string (kept
    /// `Send`-friendly; the main thread only needs to know it failed).
    pub(crate) layout: Result<TextLayout, String>,
}

/// Handle to the background shaping thread and its result channel.
///
/// Only constructed off-wasm; on wasm the field stays `None` and async shaping
/// falls back to synchronous behaviour until the web-worker backend lands.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) struct ShapeWorker {
    job_tx: Sender<WorkerJob>,
    done_rx: Receiver<WorkerDone>,
    #[cfg(not(target_arch = "wasm32"))]
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ShapeWorker {
    /// Spawns the worker thread. It runs `make_fonts` once to build its own
    /// font database, then shapes each incoming request and calls `wake` after
    /// posting the result so a reactive host can schedule a frame.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn spawn<F, W>(make_fonts: F, wake: W) -> Self
    where
        F: FnOnce() -> FontDatabase + Send + 'static,
        W: Fn() + Send + Sync + 'static,
    {
        let (job_tx, job_rx) = std::sync::mpsc::channel::<WorkerJob>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<WorkerDone>();
        let handle = std::thread::Builder::new()
            .name("astrelis-shape".to_owned())
            .spawn(move || run(&job_rx, &done_tx, make_fonts, wake))
            .expect("failed to spawn text-shaping worker");
        Self {
            job_tx,
            done_rx,
            handle: Some(handle),
        }
    }

    /// Enqueues a job. A send failure means the worker thread has already gone;
    /// the job is dropped and the node keeps its previous layout until a later
    /// synchronous pass reshapes it.
    pub(crate) fn send(&self, job: WorkerJob) {
        let _ = self.job_tx.send(job);
    }

    /// Non-blocking drain of every result ready right now.
    pub(crate) fn try_drain(&self) -> impl Iterator<Item = WorkerDone> + '_ {
        self.done_rx.try_iter()
    }

    /// Blocks for the next result, or returns `None` if the worker has gone.
    pub(crate) fn recv(&self) -> Option<WorkerDone> {
        self.done_rx.recv().ok()
    }
}

/// The worker loop: build fonts once, then shape until the channel closes or a
/// `Stop` arrives.
#[cfg(not(target_arch = "wasm32"))]
fn run<F, W>(job_rx: &Receiver<WorkerJob>, done_tx: &Sender<WorkerDone>, make_fonts: F, wake: W)
where
    F: FnOnce() -> FontDatabase,
    W: Fn(),
{
    let mut fonts = make_fonts();
    let mut text_context = TextLayoutContext::new();
    while let Ok(job) = job_rx.recv() {
        let WorkerJob::Shape {
            id,
            request_id,
            request,
        } = job
        else {
            break;
        };
        let layout = crate::text::shape_request(&mut text_context, &mut fonts, &request)
            .map_err(|error| error.to_string());
        if done_tx
            .send(WorkerDone {
                id,
                request_id,
                layout,
            })
            .is_err()
        {
            break;
        }
        wake();
    }
}

/// Closes the job channel and joins the worker so its thread and fonts are
/// released before the `Ui` finishes dropping.
#[cfg(not(target_arch = "wasm32"))]
impl Drop for ShapeWorker {
    fn drop(&mut self) {
        let _ = self.job_tx.send(WorkerJob::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl<Message: 'static> Ui<Message> {
    /// Enables background text reshaping (opt-in; off by default).
    ///
    /// Eligible reshapes are offloaded to a single worker thread that builds
    /// its own font database by calling `make_fonts`, while the previous layout
    /// stays on screen until the result is applied. After posting each result
    /// the worker calls `wake`, which a reactive host wires to its redraw
    /// request (e.g. an event-loop proxy) so the frame that calls
    /// [`Ui::poll_async`] is scheduled.
    ///
    /// `make_fonts` must produce a database that shapes identically to the one
    /// passed to [`Ui::new`], or text will shift when a node moves between the
    /// synchronous and asynchronous paths (focused fields, never-shaped nodes,
    /// and resweeps always shape synchronously).
    ///
    /// On wasm this is a no-op: there is no worker thread yet, so shaping stays
    /// synchronous. Calling it again replaces any existing worker.
    pub fn enable_async_shaping<F, W>(&mut self, make_fonts: F, wake: W)
    where
        F: FnOnce() -> FontDatabase + Send + 'static,
        W: Fn() + Send + Sync + 'static,
    {
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Results belong to the worker that produced them. Settle the old
            // channel before replacing it so its outstanding count and node
            // pending markers cannot become unreachable from the new worker.
            if self.worker.is_some() {
                self.flush_async();
            }
            self.worker = Some(ShapeWorker::spawn(make_fonts, wake));
            self.shape_policy = ShapePolicy::Async;
        }
        #[cfg(target_arch = "wasm32")]
        {
            // No worker threads on wasm yet (the web-worker backend is a later
            // Milestone 20 phase); stay synchronous. Bind the arguments so the
            // signature and drop timing match the native path.
            let _ = (make_fonts, wake);
        }
    }

    /// Applies any completed background reshapes without blocking, returning
    /// whether any node's layout changed so a host can decide to repaint. Cheap
    /// and safe to call every frame; a no-op when async shaping is not enabled.
    pub fn poll_async(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let results: Vec<WorkerDone> = worker.try_drain().collect();
        let mut changed = false;
        for result in results {
            self.async_outstanding = self.async_outstanding.saturating_sub(1);
            changed |= self.apply_worker_result(result);
        }
        changed
    }

    /// Blocks until every in-flight reshape has been applied, returning whether
    /// any node changed. Intended for tests and headless runs that need the
    /// async path to settle deterministically; a reactive host uses
    /// [`Ui::poll_async`] on the wake instead. A no-op when async shaping is not
    /// enabled.
    pub fn flush_async(&mut self) -> bool {
        let mut changed = false;
        while self.async_outstanding > 0 {
            let Some(worker) = &self.worker else {
                break;
            };
            let Some(result) = worker.recv() else {
                // Worker gone; nothing more will arrive.
                self.async_outstanding = 0;
                break;
            };
            self.async_outstanding -= 1;
            changed |= self.apply_worker_result(result);
        }
        changed
    }

    /// Applies one worker result to its node, or drops it if the node was
    /// despawned or the request has been superseded. Returns whether the node's
    /// layout changed.
    fn apply_worker_result(&mut self, result: WorkerDone) -> bool {
        // The slot may have been despawned (and possibly recycled) while the
        // reshape was in flight; the generational id makes that a clean miss.
        let pending_id = match self.node(result.id) {
            Ok(node) => node.pending.as_ref().map(|(request_id, _)| *request_id),
            Err(_) => return false,
        };
        // Only the request still in flight for this node may apply. An edit
        // after enqueue (or a force-synced reshape) replaced `pending` with a
        // newer id or cleared it, so this result is stale.
        if pending_id != Some(result.request_id) {
            return false;
        }
        let layout = match result.layout {
            Ok(layout) => layout,
            Err(_) => {
                // Shaping failed on the worker: drop the in-flight marker and
                // keep the previous layout rather than blanking the node.
                if let Ok(node) = self.node_mut(result.id) {
                    node.pending = None;
                }
                return false;
            }
        };
        let node = self
            .node_mut(result.id)
            .expect("node presence was checked above");
        let request = node.pending.take().map(|(_, request)| request);
        node.text_layout = Some(layout);
        node.text_request = request;
        // The new extent can change this node's measured size, so re-run its
        // measure and layout (and repaint). Shaping itself is skipped next pass
        // because the showing layout now matches the request.
        self.invalidate_node(result.id, Dirty::MEASURE | Dirty::LAYOUT | Dirty::PAINT);
        true
    }
}
