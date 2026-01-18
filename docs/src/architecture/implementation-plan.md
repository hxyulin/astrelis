# Detailed Implementation Plan

This document provides a concrete, step-by-step implementation plan for the architectural redesign, with detailed explanations of Rust borrow checking considerations.

## Table of Contents

1. [Phase 1: Foundation](#phase-1-foundation)
   - [Step 1.1: RenderContext Trait](#step-11-rendercontext-trait)
   - [Step 1.2: MockRenderContext](#step-12-mockrendercontext)
   - [Step 1.3: Owned GraphicsContext](#step-13-owned-graphicscontext)
2. [Phase 2: Widget System](#phase-2-widget-system)
   - [Step 2.1: Capability Traits](#step-21-capability-traits)
   - [Step 2.2: Typed Handles](#step-22-typed-handles)
   - [Step 2.3: Widget Storage](#step-23-widget-storage)

---

## Phase 1: Foundation

### Step 1.1: RenderContext Trait

**Goal**: Abstract GPU operations behind a trait to enable mocking.

#### Borrow Checking Considerations

**Key Challenge**: WGPU types like `Buffer`, `Texture`, etc. have lifetimes tied to `Device`. We need to abstract these without losing compile-time safety.

**Solution**: Use owned, reference-counted types internally. The trait returns owned types, not references.

#### Implementation

**File**: `crates/astrelis-test-utils/Cargo.toml` (new crate)

```toml
[package]
name = "astrelis-test-utils"
version = "0.1.0"
edition = "2024"

[dependencies]
wgpu = "27.0.1"
parking_lot = "0.12"
```

**File**: `crates/astrelis-test-utils/src/render_context.rs`

```rust
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor,
    Buffer, BufferDescriptor, ComputePipeline, ComputePipelineDescriptor,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerDescriptor,
    ShaderModule, ShaderModuleDescriptor, Texture, TextureDescriptor,
};

/// Trait abstracting GPU resource creation and operations.
///
/// # Lifetime Considerations
///
/// This trait does NOT use lifetimes because:
/// 1. All returned types are owned (not borrowed from Device)
/// 2. WGPU uses Arc internally, so cloning is cheap
/// 3. Resources are reference-counted and live until dropped
///
/// This makes the trait object-safe and easy to mock.
pub trait RenderContext: Send + Sync {
    // Buffer operations
    fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer;
    fn write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]);

    // Texture operations
    fn create_texture(&self, desc: &TextureDescriptor) -> Texture;

    // Shader operations
    fn create_shader_module(&self, desc: &ShaderModuleDescriptor) -> ShaderModule;

    // Pipeline operations
    fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> RenderPipeline;
    fn create_compute_pipeline(&self, desc: &ComputePipelineDescriptor) -> ComputePipeline;

    // Bind group operations
    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDescriptor) -> BindGroupLayout;
    fn create_bind_group(&self, desc: &BindGroupDescriptor) -> BindGroup;

    // Sampler operations
    fn create_sampler(&self, desc: &SamplerDescriptor) -> Sampler;

    // Queue operations (for submitting work)
    fn queue_write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]);
    fn queue_write_texture(
        &self,
        texture: wgpu::ImageCopyTexture,
        data: &[u8],
        layout: wgpu::ImageDataLayout,
        size: wgpu::Extent3d,
    );
}
```

**Why This Works with Borrow Checker**:

1. **No Lifetimes**: All methods take `&self` (shared reference) and return owned types
2. **Reference Counting**: WGPU internally uses `Arc` for `Device` and `Queue`, so multiple `RenderContext` instances can share the same device
3. **Object Safety**: Trait is object-safe (`dyn RenderContext` works) because:
   - No `Self: Sized` bounds
   - No generic methods
   - No associated types with `Self` in return position

**Trait Object Usage**:

```rust
// This compiles because RenderContext is object-safe
fn render_ui(ctx: &dyn RenderContext) {
    let buffer = ctx.create_buffer(&desc);
    // buffer is owned, so no lifetime issues
}

// Can also use generic version for monomorphization
fn render_ui_generic<R: RenderContext>(ctx: &R) {
    let buffer = ctx.create_buffer(&desc);
}
```

---

### Step 1.2: MockRenderContext

**Goal**: Implement a mock for testing without GPU.

#### Borrow Checking Considerations

**Key Challenge**: Mock needs to store created resources and track calls, but methods take `&self` (not `&mut self`).

**Solution**: Use interior mutability with `RefCell` or `Mutex`. We'll use `parking_lot::Mutex` for `Send + Sync`.

#### Implementation

**File**: `crates/astrelis-test-utils/src/mock_render.rs`

```rust
use super::render_context::RenderContext;
use parking_lot::Mutex;
use wgpu::*;

/// Records a GPU operation call for verification in tests.
#[derive(Debug, Clone)]
pub enum RenderCall {
    CreateBuffer { size: u64, usage: BufferUsages },
    WriteBuffer { buffer_id: usize, offset: u64, size: usize },
    CreateTexture { width: u32, height: u32, format: TextureFormat },
    CreateRenderPipeline { label: Option<String> },
}

/// Mock implementation of RenderContext for testing.
///
/// # Borrow Checking Pattern: Interior Mutability
///
/// Methods take `&self` but need to mutate internal state (record calls).
/// Solution: Use `Mutex<Vec<RenderCall>>` for interior mutability.
///
/// Why Mutex instead of RefCell?
/// - Mutex is Send + Sync (required for RenderContext trait)
/// - RefCell is !Sync, so can't be used in multi-threaded contexts
/// - parking_lot::Mutex has less overhead than std::sync::Mutex
pub struct MockRenderContext {
    /// Recorded calls for verification
    calls: Mutex<Vec<RenderCall>>,

    /// Mock buffers (we don't create real GPU buffers)
    /// Each buffer gets an ID, stored here
    buffers: Mutex<Vec<MockBuffer>>,
}

#[derive(Debug)]
struct MockBuffer {
    id: usize,
    size: u64,
    usage: BufferUsages,
}

impl MockRenderContext {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            buffers: Mutex::new(Vec::new()),
        }
    }

    /// Get a copy of all recorded calls (for test assertions)
    pub fn calls(&self) -> Vec<RenderCall> {
        self.calls.lock().clone()
    }

    /// Count calls of a specific type
    pub fn count_buffer_creates(&self) -> usize {
        self.calls.lock()
            .iter()
            .filter(|call| matches!(call, RenderCall::CreateBuffer { .. }))
            .count()
    }

    /// Clear recorded calls (useful between test steps)
    pub fn clear_calls(&self) {
        self.calls.lock().clear();
    }
}

impl RenderContext for MockRenderContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer {
        // Record the call
        self.calls.lock().push(RenderCall::CreateBuffer {
            size: desc.size,
            usage: desc.usage,
        });

        // For mocking, we can't create a real Buffer without a Device.
        // Options:
        // 1. Return a dummy buffer (won't work - Buffer isn't constructible)
        // 2. Create a real Device in the mock (defeats the purpose)
        // 3. Change RenderContext to return wrapper types

        // We'll use option 3 - see next section for wrapper types

        // For now, panic with clear message
        unimplemented!("Mock buffer creation - see wrapper types section")
    }

    fn write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]) {
        self.calls.lock().push(RenderCall::WriteBuffer {
            buffer_id: 0, // Will be properly tracked with wrapper types
            offset,
            size: data.len(),
        });
    }

    // Similar implementations for other methods...
}
```

**Borrow Checking Analysis**:

```rust
// This works because:
impl MockRenderContext {
    pub fn calls(&self) -> Vec<RenderCall> {
        // 1. &self (shared reference)
        self.calls.lock()  // 2. lock() temporarily locks Mutex, returns MutexGuard
            .clone()       // 3. clone the Vec, MutexGuard drops, lock released
    }                      // 4. Return owned Vec (no lifetime issues)
}

// Why interior mutability is safe here:
// - Mutex ensures no data races
// - Each method call locks, modifies, unlocks
// - No references escape the lock scope
```

**Problem**: We can't return real WGPU types from mock without a Device.

**Solution**: Introduce wrapper types (next section).

---

### Step 1.2b: GPU Resource Wrappers

**Goal**: Wrap WGPU types so we can mock them.

#### Borrow Checking Considerations

**Key Insight**: We need a way to represent GPU resources that can be either real or mock, without exposing which.

**Solution**: Use opaque wrapper types with an internal enum.

#### Implementation

**File**: `crates/astrelis-test-utils/src/gpu_types.rs`

```rust
use wgpu;

/// Wrapper around GPU buffer that can be real or mock.
///
/// # Borrow Checking Pattern: Opaque Wrapper
///
/// This type hides whether it contains a real wgpu::Buffer or a mock.
/// Users hold owned `GpuBuffer`, which is cheap to clone (Arc inside).
///
/// Benefits:
/// 1. No lifetimes - users own the buffer
/// 2. Can be mock or real without user knowing
/// 3. Clone is cheap (Arc internally)
#[derive(Clone)]
pub struct GpuBuffer {
    inner: GpuBufferInner,
}

#[derive(Clone)]
enum GpuBufferInner {
    Real(wgpu::Buffer),
    Mock { id: usize, size: u64 },
}

impl GpuBuffer {
    /// Create from real WGPU buffer
    pub fn from_wgpu(buffer: wgpu::Buffer) -> Self {
        Self {
            inner: GpuBufferInner::Real(buffer),
        }
    }

    /// Create mock buffer (for testing)
    pub fn mock(id: usize, size: u64) -> Self {
        Self {
            inner: GpuBufferInner::Mock { id, size },
        }
    }

    /// Get the underlying wgpu::Buffer (if real)
    ///
    /// # Panics
    /// Panics if this is a mock buffer (test code should never call this)
    pub fn as_wgpu(&self) -> &wgpu::Buffer {
        match &self.inner {
            GpuBufferInner::Real(buffer) => buffer,
            GpuBufferInner::Mock { .. } => {
                panic!("Attempted to get wgpu::Buffer from mock buffer")
            }
        }
    }

    /// Check if this is a mock (useful in tests)
    pub fn is_mock(&self) -> bool {
        matches!(self.inner, GpuBufferInner::Mock { .. })
    }
}

// Similar wrappers for other GPU types
pub struct GpuTexture { /* ... */ }
pub struct GpuSampler { /* ... */ }
pub struct GpuRenderPipeline { /* ... */ }
// etc.
```

**Updated RenderContext Trait**:

```rust
pub trait RenderContext: Send + Sync {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer;
    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]);
    fn create_texture(&self, desc: &TextureDescriptor) -> GpuTexture;
    // ... etc
}
```

**Updated MockRenderContext**:

```rust
impl RenderContext for MockRenderContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer {
        let mut buffers = self.buffers.lock();
        let id = buffers.len();

        buffers.push(MockBuffer {
            id,
            size: desc.size,
            usage: desc.usage,
        });

        self.calls.lock().push(RenderCall::CreateBuffer {
            size: desc.size,
            usage: desc.usage,
        });

        GpuBuffer::mock(id, desc.size)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]) {
        // Can now extract mock buffer ID
        if buffer.is_mock() {
            self.calls.lock().push(RenderCall::WriteBuffer {
                buffer_id: 0, // Extract from buffer.inner
                offset,
                size: data.len(),
            });
        }
    }
}
```

**Borrow Checking Analysis**:

```rust
// Users can freely pass around GpuBuffer:
fn do_rendering(ctx: &dyn RenderContext) {
    let buffer = ctx.create_buffer(&desc);  // Owned GpuBuffer

    // Can pass by reference
    ctx.write_buffer(&buffer, 0, &data);

    // Can clone (cheap - Arc inside)
    let buffer2 = buffer.clone();

    // Can move
    let buffer3 = buffer;

    // No lifetime issues!
}
```

---

### Step 1.3: Owned GraphicsContext

**Goal**: Add `GraphicsContext::new()` that returns owned context (no Box::leak).

#### Borrow Checking Considerations

**Key Challenge**: Current code expects `&'static GraphicsContext`. We need to support both patterns during migration.

**Solution**: Use `Arc<GraphicsContext>` as the "owned" type. Clone is cheap and satisfies all use cases.

#### Implementation

**File**: `crates/astrelis-render/src/context.rs`

```rust
use std::sync::Arc;
use wgpu::{Adapter, Device, Queue};

/// GPU context containing device, queue, and adapter.
///
/// # Ownership Patterns
///
/// This type supports two ownership patterns:
///
/// ## 1. Owned (Recommended)
/// ```rust
/// let ctx = GraphicsContext::new().await;  // Returns Arc<Self>
/// let ctx2 = ctx.clone();  // Cheap clone (Arc)
/// ```
///
/// ## 2. Static (Deprecated, for migration)
/// ```rust
/// let ctx = GraphicsContext::new_sync();  // Returns &'static Self (leaked)
/// ```
///
/// The owned pattern is preferred because:
/// - No memory leak
/// - Proper cleanup on drop
/// - Better for testing (can create/destroy contexts)
///
/// # Borrow Checking Considerations
///
/// Q: Why Arc instead of Box?
/// A: Multiple parts of the engine need to hold a context reference:
///    - RenderableWindow needs it
///    - UiRenderer needs it
///    - TextRenderer needs it
///    With Arc, all can share ownership cheaply.
///
/// Q: Why not just pass &GraphicsContext everywhere?
/// A: Lifetimes would propagate through entire codebase:
///    - RenderableWindow<'ctx>
///    - UiRenderer<'ctx>
///    - Every user struct that contains these
///    Arc avoids this lifetime complexity while still being safe.
pub struct GraphicsContext {
    device: Arc<Device>,
    queue: Arc<Queue>,
    adapter: Arc<Adapter>,
}

impl GraphicsContext {
    /// Create a new graphics context (owned).
    ///
    /// Returns Arc<Self> so it can be cheaply cloned and shared.
    ///
    /// # Example
    /// ```rust
    /// let ctx = GraphicsContext::new().await?;
    /// let window = RenderableWindow::new(window, ctx.clone());
    /// let ui = UiSystem::new(ctx.clone());
    /// ```
    pub async fn new() -> Result<Arc<Self>, GraphicsContextError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(GraphicsContextError::AdapterNotFound)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .map_err(|e| GraphicsContextError::DeviceCreationFailed(e.to_string()))?;

        Ok(Arc::new(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter: Arc::new(adapter),
        }))
    }

    /// Create a new graphics context (synchronous, deprecated).
    ///
    /// Returns &'static Self using Box::leak.
    ///
    /// # Warning
    /// This leaks memory and should only be used during migration.
    /// Use `new()` instead for new code.
    #[deprecated(
        since = "0.2.0",
        note = "Use GraphicsContext::new() instead. This leaks memory."
    )]
    pub fn new_sync() -> &'static Self {
        let ctx = pollster::block_on(async {
            Self::new().await.expect("Failed to create graphics context")
        });

        // Leak the Arc to get &'static
        // SAFETY: This is intentionally leaking memory for backwards compatibility
        Box::leak(Box::new(Arc::try_unwrap(ctx).unwrap()))
    }

    /// Get reference to device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get reference to queue
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Get reference to adapter
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }
}

/// Implement RenderContext trait
impl RenderContext for GraphicsContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer {
        let buffer = self.device.create_buffer(desc);
        GpuBuffer::from_wgpu(buffer)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]) {
        self.queue.write_buffer(buffer.as_wgpu(), offset, data);
    }

    // ... other methods
}

/// Implement RenderContext for Arc<GraphicsContext> as well
/// This allows passing Arc<GraphicsContext> directly as &dyn RenderContext
impl RenderContext for Arc<GraphicsContext> {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer {
        (**self).create_buffer(desc)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]) {
        (**self).write_buffer(buffer, offset, data)
    }

    // ... other methods
}
```

**Borrow Checking Analysis**:

```rust
// Pattern 1: Arc ownership (new code)
async fn main() {
    let ctx = GraphicsContext::new().await.unwrap();
    // ctx: Arc<GraphicsContext>

    // Can clone cheaply
    let ctx2 = ctx.clone();

    // Can pass to functions
    setup_renderer(ctx.clone());
    setup_ui(ctx.clone());

    // No lifetime issues!
} // ctx drops here, cleans up properly

fn setup_renderer(ctx: Arc<GraphicsContext>) {
    // Takes ownership of one Arc reference
    // When this function returns, ref count decrements
}

// Pattern 2: Trait object (for polymorphism)
fn render(ctx: &dyn RenderContext) {
    let buffer = ctx.create_buffer(&desc);
    // Works with both GraphicsContext and MockRenderContext
}

// Pattern 3: Static reference (old code, deprecated)
fn old_code() {
    #[allow(deprecated)]
    let ctx = GraphicsContext::new_sync();
    // ctx: &'static GraphicsContext
    // Still works, but leaks memory
}
```

**Migration Path for Existing Code**:

```rust
// Before:
struct MyRenderer {
    ctx: &'static GraphicsContext,
}

impl MyRenderer {
    fn new() -> Self {
        Self {
            ctx: GraphicsContext::new_sync(),
        }
    }
}

// After (step 1 - minimal change):
struct MyRenderer {
    ctx: Arc<GraphicsContext>,
}

impl MyRenderer {
    async fn new() -> Self {
        Self {
            ctx: GraphicsContext::new().await.unwrap(),
        }
    }
}

// After (step 2 - use trait):
struct MyRenderer {
    // Can now be mocked!
    ctx: Arc<dyn RenderContext>,
}

impl MyRenderer {
    fn new(ctx: Arc<dyn RenderContext>) -> Self {
        Self { ctx }
    }
}
```

**Key Borrow Checker Insights**:

1. **Arc vs &'static**:
   - `&'static` requires memory leak
   - `Arc` provides shared ownership without leak
   - Clone cost: atomic increment (very cheap)

2. **Arc vs Rc**:
   - `Arc` is `Send + Sync` (required for RenderContext trait)
   - `Rc` is `!Send` and would fail trait bounds

3. **Arc<T> vs Arc<dyn Trait>**:
   - `Arc<GraphicsContext>` allows `ctx.device()` direct calls
   - `Arc<dyn RenderContext>` allows mock swapping
   - Can convert: `ctx as Arc<dyn RenderContext>`

---

## Phase 2: Widget System

### Step 2.1: Capability Traits

**Goal**: Define traits for widget capabilities instead of downcasting.

#### Borrow Checking Considerations

**Key Challenge**: Widget tree is `Vec<Box<dyn Widget>>`. How do we get `&mut dyn TextWidget` from `&mut dyn Widget`?

**Solution**: Use trait upcasting (nightly) or query methods.

#### Implementation

**File**: `crates/astrelis-ui/src/widget/capability.rs`

```rust
use taffy::prelude::*;

/// Base trait for all widgets.
///
/// # Borrow Checking Pattern: Trait Object Trees
///
/// Widgets are stored as `Vec<Box<dyn Widget>>`.
/// Challenge: How to access specific capabilities?
///
/// Solution 1: Trait upcasting (requires nightly Rust)
/// ```ignore
/// let widget: &mut dyn Widget = &mut button;
/// let text: &mut dyn TextWidget = widget;  // Upcast
/// ```
///
/// Solution 2: Query methods (stable Rust)
/// ```rust
/// if let Some(text) = widget.as_text_widget_mut() {
///     text.set_text("...");
/// }
/// ```
///
/// We use Solution 2 for stability.
pub trait Widget: Send + Sync {
    /// Get widget ID
    fn id(&self) -> WidgetId;

    /// Get layout node
    fn layout_node(&self) -> NodeId;

    /// Query if this widget supports text operations
    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        None
    }

    /// Query if this widget supports text operations (mutable)
    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        None
    }

    /// Query if this widget is a container
    fn as_container(&self) -> Option<&dyn ParentWidget> {
        None
    }

    /// Query if this widget is a container (mutable)
    fn as_container_mut(&mut self) -> Option<&mut dyn ParentWidget> {
        None
    }

    /// Query if this widget supports color operations
    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        None
    }

    /// Query if this widget supports color operations (mutable)
    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        None
    }
}

/// Capability: Widget that can contain children.
///
/// # Borrow Checking: Mutable Access to Children
///
/// Q: Why return `&[Box<dyn Widget>]` instead of `&[&dyn Widget]`?
/// A: Box ownership is needed for adding/removing children.
///    Users can deref to get `&dyn Widget`.
///
/// Q: How to iterate mutably?
/// A: Use `children_mut()` which returns `&mut [Box<dyn Widget>]`.
///    Then iter_mut() gives `&mut Box<dyn Widget>`.
///    Deref gives `&mut dyn Widget`.
pub trait ParentWidget: Widget {
    /// Get children (immutable)
    fn children(&self) -> &[Box<dyn Widget>];

    /// Get children (mutable)
    fn children_mut(&mut self) -> &mut [Box<dyn Widget>];

    /// Add a child widget
    ///
    /// Takes ownership of the child widget.
    fn add_child(&mut self, child: Box<dyn Widget>);

    /// Remove a child by ID
    ///
    /// Returns the removed child if found.
    fn remove_child(&mut self, id: WidgetId) -> Option<Box<dyn Widget>>;
}

/// Capability: Widget that displays text.
pub trait TextWidget: Widget {
    /// Get the current text
    fn text(&self) -> &str;

    /// Set the text
    ///
    /// This will invalidate text shaping cache.
    fn set_text(&mut self, text: impl Into<String>);

    /// Get text style
    fn text_style(&self) -> &TextStyle;

    /// Set text style
    fn set_text_style(&mut self, style: TextStyle);
}

/// Capability: Widget with a background color.
pub trait ColorWidget: Widget {
    /// Get the current color
    fn color(&self) -> Color;

    /// Set the color
    ///
    /// This only invalidates color, not layout or geometry.
    fn set_color(&mut self, color: Color);
}

// Widget ID type
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);
```

**Example Widget Implementation**:

```rust
/// Button widget implementing multiple capabilities
pub struct Button {
    id: WidgetId,
    node: NodeId,
    text: String,
    text_style: TextStyle,
    background_color: Color,
}

impl Widget for Button {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> NodeId {
        self.node
    }

    // Implement query methods to expose capabilities
    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }
}

impl TextWidget for Button {
    fn text(&self) -> &str {
        &self.text
    }

    fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        // Mark dirty flag here
    }

    fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    fn set_text_style(&mut self, style: TextStyle) {
        self.text_style = style;
        // Mark dirty flag here
    }
}

impl ColorWidget for Button {
    fn color(&self) -> Color {
        self.background_color
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        // Mark dirty flag here
    }
}
```

**Usage Example**:

```rust
fn update_widget_text(widget: &mut dyn Widget, new_text: &str) {
    // Type-safe capability check
    if let Some(text_widget) = widget.as_text_widget_mut() {
        text_widget.set_text(new_text);
    } else {
        // Widget doesn't support text
        eprintln!("Widget {} doesn't support text", widget.id());
    }
}

// Borrow checking analysis:
fn process_children(parent: &mut dyn ParentWidget) {
    // Get mutable slice of children
    let children = parent.children_mut();

    // Iterate mutably
    for child in children.iter_mut() {
        // child: &mut Box<dyn Widget>
        // Deref to get &mut dyn Widget
        let widget: &mut dyn Widget = &mut **child;

        // Try to update text
        if let Some(text) = widget.as_text_widget_mut() {
            text.set_text("Updated");
        }
    }
}
```

**Borrow Checker Insights**:

1. **Why query methods instead of trait upcasting?**
   - Trait upcasting is unstable (requires nightly)
   - Query methods work on stable Rust
   - Pattern: Option<&dyn Trait> for optional capabilities

2. **Mutable borrowing through trait objects**:
   ```rust
   let widget: &mut dyn Widget = ...;

   // This works:
   if let Some(text) = widget.as_text_widget_mut() {
       text.set_text("...");
   }

   // Borrow checker is happy because:
   // 1. as_text_widget_mut() takes &mut self
   // 2. Returns Option<&mut dyn TextWidget> with same lifetime
   // 3. When Some() block ends, mutable borrow ends
   ```

3. **Why `&[Box<dyn Widget>]` not `&[&dyn Widget]`?**
   - Need ownership for add/remove operations
   - Box provides owned trait object
   - Can deref Box to get &dyn Widget when needed

---

### Step 2.2: Typed Handles

**Goal**: Replace string IDs with typed handles that provide compile-time safety.

#### Borrow Checking Considerations

**Key Challenge**: Handle must be usable across different widget types while maintaining type safety.

**Solution**: Generic handle with phantom type parameter.

#### Implementation

**File**: `crates/astrelis-ui/src/widget/handle.rs`

```rust
use std::marker::PhantomData;
use super::capability::*;

/// Type-safe handle to a widget.
///
/// # Borrow Checking Pattern: Phantom Types
///
/// This handle doesn't actually contain a `T`, but uses PhantomData
/// to track the type at compile time.
///
/// Benefits:
/// 1. Zero-cost abstraction (same size as WidgetId + generation)
/// 2. Type safety - can't pass wrong handle type
/// 3. Handles are Copy (no borrowing issues)
///
/// # Generational Safety
///
/// The generation counter prevents use-after-free:
/// - Widget removed → generation increments
/// - Old handle → generation mismatch → returns None
///
/// Similar to generational arenas / slotmaps.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct WidgetHandle<T: ?Sized> {
    id: WidgetId,
    generation: u32,
    _phantom: PhantomData<*const T>,  // *const for variance
}

// SAFETY: WidgetHandle is just an ID + generation, no actual T pointer
unsafe impl<T: ?Sized> Send for WidgetHandle<T> {}
unsafe impl<T: ?Sized> Sync for WidgetHandle<T> {}

impl<T: ?Sized> WidgetHandle<T> {
    /// Create a new handle (internal use only)
    pub(crate) fn new(id: WidgetId, generation: u32) -> Self {
        Self {
            id,
            generation,
            _phantom: PhantomData,
        }
    }

    /// Get the widget ID
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// Get the generation
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Upcast to a trait object handle
    ///
    /// # Example
    /// ```rust
    /// let button: WidgetHandle<Button> = ui.add(Button::new());
    /// let text: WidgetHandle<dyn TextWidget> = button.upcast();
    /// ```
    pub fn upcast<U: ?Sized>(self) -> WidgetHandle<U>
    where
        T: Unsize<U>,
    {
        WidgetHandle {
            id: self.id,
            generation: self.generation,
            _phantom: PhantomData,
        }
    }

    /// Try to downcast to a concrete type
    ///
    /// Returns None if the widget is not of type U.
    pub fn downcast<U>(self) -> Option<WidgetHandle<U>>
    where
        U: Widget,
    {
        // This is a type-level operation only
        // Actual type checking happens when dereferencing
        Some(WidgetHandle {
            id: self.id,
            generation: self.generation,
            _phantom: PhantomData,
        })
    }
}

// Implement Copy since it's just an ID
impl<T: ?Sized> Copy for WidgetHandle<T> {}
```

**Why PhantomData<*const T> instead of PhantomData<T>?**

```rust
// With PhantomData<T>:
struct Handle1<T> {
    id: u64,
    _phantom: PhantomData<T>,  // Implies ownership
}

// Problem: Compiler treats this as if Handle owns a T
// - Not Copy if T is not Copy
// - Drop glue even though no T exists
// - Wrong variance

// With PhantomData<*const T>:
struct Handle2<T> {
    id: u64,
    _phantom: PhantomData<*const T>,  // Implies borrowed reference
}

// Benefits:
// - Always Copy (pointer is Copy)
// - Correct variance (covariant in T)
// - No drop glue
// - Makes it clear: this is an ID, not ownership
```

**Usage Example**:

```rust
use astrelis_ui::*;

fn example(ui: &mut UiSystem) {
    // Create a button - returns typed handle
    let button: WidgetHandle<Button> = ui.add(Button::new("Click me"));

    // Can pass handle by value (it's Copy)
    update_button_text(button, "New text");

    // Can upcast to trait
    let text_handle: WidgetHandle<dyn TextWidget> = button.upcast();
    update_text(text_handle, "Another text");

    // Compile-time type safety!
    let rect: WidgetHandle<Rect> = ui.add(Rect::new());
    // update_button_text(rect, "text");  // ERROR: expected Button, found Rect
}

fn update_button_text(handle: WidgetHandle<Button>, text: &str) {
    // Type-safe: we know this is a Button
}

fn update_text(handle: WidgetHandle<dyn TextWidget>, text: &str) {
    // Works with any widget that implements TextWidget
}
```

**Borrow Checker Insights**:

1. **Handles are Copy**:
   ```rust
   let h1 = button;
   let h2 = button;  // h1 still valid (Copy, not Move)
   ```

2. **No lifetime issues**:
   ```rust
   fn get_handle(ui: &UiSystem) -> WidgetHandle<Button> {
       // Can return handle without lifetime parameter
       // Handle doesn't borrow from ui
       ui.find_button("id")
   }
   ```

3. **Type safety at compile time**:
   ```rust
   fn set_text<T: TextWidget>(handle: WidgetHandle<T>, text: &str) {
       // Can only pass handles to widgets that impl TextWidget
   }

   let button: WidgetHandle<Button> = ...;  // Button: TextWidget ✓
   set_text(button, "ok");  // Compiles

   let rect: WidgetHandle<Rect> = ...;  // Rect: !TextWidget ✗
   // set_text(rect, "no");  // Compile error!
   ```

---

### Step 2.3: Widget Storage

**Goal**: Store widgets in a way that allows efficient lookup by handle.

#### Borrow Checking Considerations

**Key Challenge**: Need to:
1. Store heterogeneous widgets (different types)
2. Look up by ID efficiently
3. Support mutable access
4. Track generations

**Solution**: Generational arena pattern with trait objects.

#### Implementation

**File**: `crates/astrelis-ui/src/widget/storage.rs`

```rust
use super::{WidgetHandle, Widget, WidgetId};
use std::collections::HashMap;

/// Entry in widget storage with generation tracking.
struct WidgetEntry {
    /// The widget (trait object)
    widget: Box<dyn Widget>,
    /// Current generation (incremented on remove)
    generation: u32,
}

/// Storage for widgets with generational safety.
///
/// # Borrow Checking Pattern: Generational Arena
///
/// This is similar to generational-arena or slotmap crates.
///
/// Key insight: We can safely give out handles because:
/// 1. Handles are Copy (just IDs)
/// 2. Storage checks generation before access
/// 3. Mutable access requires &mut self (exclusive borrow)
///
/// This prevents:
/// - Use-after-free (stale handles return None)
/// - Aliasing violations (can't get multiple &mut)
pub struct WidgetStorage {
    /// Map from widget ID to entry
    widgets: HashMap<WidgetId, WidgetEntry>,

    /// Next ID to allocate
    next_id: u64,
}

impl WidgetStorage {
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a widget and return a typed handle.
    ///
    /// # Borrow Checking
    ///
    /// Takes `&mut self` (exclusive borrow) to modify storage.
    /// Returns handle (Copy) so no borrow conflicts.
    pub fn add<T: Widget + 'static>(&mut self, widget: T) -> WidgetHandle<T> {
        let id = WidgetId(self.next_id);
        self.next_id += 1;

        let entry = WidgetEntry {
            widget: Box::new(widget),
            generation: 0,
        };

        self.widgets.insert(id, entry);

        WidgetHandle::new(id, 0)
    }

    /// Get immutable reference to widget.
    ///
    /// Returns None if:
    /// - Widget doesn't exist
    /// - Generation mismatch (widget was removed and slot reused)
    ///
    /// # Borrow Checking
    ///
    /// Takes &self (shared borrow).
    /// Returns Option<&dyn Widget> with same lifetime as &self.
    /// Multiple get() calls can coexist (all shared borrows).
    pub fn get<T: ?Sized>(&self, handle: WidgetHandle<T>) -> Option<&dyn Widget> {
        let entry = self.widgets.get(&handle.id())?;

        // Check generation
        if entry.generation != handle.generation() {
            return None;  // Stale handle
        }

        Some(&*entry.widget)
    }

    /// Get mutable reference to widget.
    ///
    /// # Borrow Checking
    ///
    /// Takes &mut self (exclusive borrow).
    /// Returns Option<&mut dyn Widget> with same lifetime as &mut self.
    ///
    /// Only one get_mut() can exist at a time (exclusive borrow).
    /// This prevents aliasing.
    pub fn get_mut<T: ?Sized>(&mut self, handle: WidgetHandle<T>) -> Option<&mut dyn Widget> {
        let entry = self.widgets.get_mut(&handle.id())?;

        // Check generation
        if entry.generation != handle.generation() {
            return None;  // Stale handle
        }

        Some(&mut *entry.widget)
    }

    /// Remove a widget.
    ///
    /// Increments generation so existing handles become invalid.
    ///
    /// # Borrow Checking
    ///
    /// Takes &mut self (exclusive borrow).
    /// Returns owned Box<dyn Widget> (no lifetime issues).
    pub fn remove<T: ?Sized>(&mut self, handle: WidgetHandle<T>) -> Option<Box<dyn Widget>> {
        let mut entry = self.widgets.remove(&handle.id())?;

        // Check generation
        if entry.generation != handle.generation() {
            // Put it back (wrong generation)
            self.widgets.insert(handle.id(), entry);
            return None;
        }

        // Increment generation for this ID
        // (If we reuse this ID later, old handles won't match)
        entry.generation += 1;

        Some(entry.widget)
    }

    /// Iterate over all widgets.
    ///
    /// # Borrow Checking
    ///
    /// Takes &self (shared borrow).
    /// Returns iterator yielding (WidgetId, &dyn Widget).
    pub fn iter(&self) -> impl Iterator<Item = (WidgetId, &dyn Widget)> {
        self.widgets
            .iter()
            .map(|(id, entry)| (*id, &*entry.widget))
    }

    /// Iterate over all widgets mutably.
    ///
    /// # Borrow Checking
    ///
    /// Takes &mut self (exclusive borrow).
    /// Returns iterator yielding (WidgetId, &mut dyn Widget).
    ///
    /// Only one iter_mut() can exist at a time.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (WidgetId, &mut dyn Widget)> {
        self.widgets
            .iter_mut()
            .map(|(id, entry)| (*id, &mut *entry.widget))
    }
}
```

**Usage Example**:

```rust
let mut storage = WidgetStorage::new();

// Add widgets
let button: WidgetHandle<Button> = storage.add(Button::new("Click"));
let text: WidgetHandle<Text> = storage.add(Text::new("Hello"));

// Get immutable reference
if let Some(widget) = storage.get(button) {
    println!("Widget ID: {:?}", widget.id());
}

// Get mutable reference
if let Some(widget) = storage.get_mut(button) {
    if let Some(text_widget) = widget.as_text_widget_mut() {
        text_widget.set_text("Updated");
    }
}

// Remove widget
let removed = storage.remove(button);

// Old handle now invalid
assert!(storage.get(button).is_none());
```

**Borrow Checker Analysis**:

```rust
// Scenario 1: Multiple immutable accesses (OK)
fn read_widgets(storage: &WidgetStorage) {
    let w1 = storage.get(handle1);  // &WidgetStorage -> Option<&Widget>
    let w2 = storage.get(handle2);  // &WidgetStorage -> Option<&Widget>

    // Both w1 and w2 can coexist (shared borrows)
    if let (Some(widget1), Some(widget2)) = (w1, w2) {
        println!("{} {}", widget1.id(), widget2.id());
    }
}

// Scenario 2: Mutable access (exclusive)
fn write_widget(storage: &mut WidgetStorage) {
    let w1 = storage.get_mut(handle1);  // &mut WidgetStorage -> Option<&mut Widget>

    // Cannot call get_mut again while w1 exists:
    // let w2 = storage.get_mut(handle2);  // ERROR: already borrowed

    // Must drop w1 first:
    drop(w1);
    let w2 = storage.get_mut(handle2);  // OK now
}

// Scenario 3: Why generations prevent use-after-free
fn use_after_remove() {
    let mut storage = WidgetStorage::new();
    let handle = storage.add(Button::new("Click"));

    // handle.generation == 0

    storage.remove(handle);  // Increments generation for this ID

    // Reuse the ID (in real code, might add different widget)
    let new_handle = storage.add(Text::new("New"));  // Same ID, generation 1

    // Old handle has generation 0, new entry has generation 1
    assert!(storage.get(handle).is_none());  // Stale handle returns None

    // Type safety still works
    let button_handle: WidgetHandle<Button> = handle;
    let text_handle: WidgetHandle<Text> = new_handle;
    // These are different types at compile time!
}
```

**Why This Pattern is Safe**:

1. **Handles are Copy**: No ownership/borrowing of handles themselves
2. **Exclusive mutable access**: `get_mut()` requires `&mut self`
3. **Generation checks**: Prevent using handles after removal
4. **Type safety**: Handle<Button> vs Handle<Text> enforced at compile time

---

## Summary of Phase 1-2 Borrow Checking Patterns

### Pattern 1: Interior Mutability with Mutex
- **Used in**: MockRenderContext
- **Why**: Methods take `&self` but need to mutate state
- **Safety**: Mutex prevents data races

### Pattern 2: Arc for Shared Ownership
- **Used in**: GraphicsContext
- **Why**: Multiple systems need to share context
- **Benefits**: No lifetimes, cheap cloning, proper cleanup

### Pattern 3: Opaque Wrappers
- **Used in**: GpuBuffer, GpuTexture
- **Why**: Hide real vs mock implementations
- **Benefits**: No lifetimes, can be owned, cheap cloning

### Pattern 4: Trait Query Methods
- **Used in**: Widget capabilities
- **Why**: Trait upcasting is unstable
- **Benefits**: Works on stable Rust, optional capabilities

### Pattern 5: Generational Handles
- **Used in**: WidgetHandle, WidgetStorage
- **Why**: Type-safe, Copy handles with use-after-free prevention
- **Benefits**: No lifetimes, Copy handles, compile-time type safety

---

## Next Steps

This document covers Phase 1 (Foundation) and Phase 2 (Widget System) in detail. Subsequent phases will follow similar patterns:

- **Phase 3**: Apply RenderContext to existing renderers
- **Phase 4**: Simplify text/UI integration (remove wrappers)
- **Phase 5**: Improve FrameContext RAII
- **Phase 6**: Migration and cleanup

Each phase will have similar detailed documentation explaining:
1. What we're changing
2. Why it's safe (borrow checker analysis)
3. How to migrate existing code
4. Example usage

The patterns established in Phases 1-2 will be reused throughout.
