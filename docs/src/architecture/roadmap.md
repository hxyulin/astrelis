# Architectural Redesign Roadmap

This document outlines the planned architectural improvements for Astrelis, addressing systemic design issues while maintaining backwards compatibility during migration.

## Overview

The current architecture has several patterns that, while functional, limit testability, type safety, and idiomatic Rust usage. This roadmap addresses five core areas:

1. **Lifetime Management** - Eliminating memory leaks from `Box::leak` pattern
2. **Widget Type Safety** - Replacing runtime downcasts with compile-time guarantees
3. **Testability** - Abstracting GPU operations for unit testing
4. **Text/UI Decoupling** - Simplifying integration complexity
5. **RAII Improvements** - Better resource management for render passes

**Timeline**: 10 weeks across 6 phases
**Breaking Changes**: Allowed, with staged migration strategy
**Status**: Planning phase - implementation not yet started

## Current Architecture Issues

### Issue 1: Box::leak Memory Leak Pattern

**Location**: `crates/astrelis-render/src/context.rs`

```rust
// Current implementation
pub fn new_sync() -> &'static GraphicsContext {
    let context = /* create context */;
    Box::leak(Box::new(context))  // Never freed!
}
```

**Problems**:
- Permanent memory leak (context never freed)
- Prevents graceful shutdown and cleanup
- Forces `'static` lifetime throughout codebase
- Makes testing difficult (can't clean up between tests)
- Violates RAII principles

**Root Cause**: Attempting to avoid lifetime parameters in API

### Issue 2: Widget Downcast Ceremony

**Location**: `crates/astrelis-ui/src/widget/base.rs`

```rust
// Current pattern - type-unsafe!
trait Widget {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// Usage requires runtime downcasts
if let Some(text) = widget.as_any_mut().downcast_mut::<Text>() {
    text.set_text("...");  // Can fail at runtime
}
```

**Problems**:
- Type-unsafe runtime downcasts
- String-based widget IDs prone to typos
- No compile-time verification of widget capabilities
- Boilerplate `as_any()` impl for every widget

### Issue 3: GPU Coupling Prevents Testing

**Problem**: All rendering code directly calls WGPU APIs

- Cannot unit test layout without GPU
- Cannot test draw list generation without GPU
- Integration tests require graphics hardware
- Impossible to mock due to concrete WGPU types

### Issue 4: Text/UI Complexity

**Location**: `crates/astrelis-ui/src/glyph_atlas.rs`

- `TextValue` wrapper adds indirection without clear benefit
- Dual version tracking (UI layer + text layer)
- Tight coupling between renderers
- Complex `Arc<ShapedTextData>` versioning

### Issue 5: RenderPass Creation Jank

```rust
// Current pattern - requires manual scoping
let mut frame = window.begin_drawing();
{  // Manual block required!
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .build(&mut frame);
    // render...
} // pass MUST drop before frame.finish()
frame.finish();
```

**Problems**:
- Requires manual `{ }` blocks to control lifetimes
- Easy to accidentally hold pass too long (compile error)
- Unclear error messages when misused
- Lifetime management footgun for users

## Proposed Solutions

### Solution 1: Owned GraphicsContext

**New API** (alongside existing for compatibility):

```rust
pub struct GraphicsContext {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    adapter: Arc<wgpu::Adapter>,
}

impl GraphicsContext {
    /// New: Owned context with normal lifetime
    pub fn new() -> Self { /* ... */ }

    /// Deprecated: Old static lifetime API
    #[deprecated(note = "Use new() instead")]
    pub fn new_sync() -> &'static Self { /* ... */ }
}
```

**Benefits**:
- ✅ No memory leak
- ✅ Graceful shutdown possible
- ✅ Better testing (create/destroy contexts per test)
- ✅ More idiomatic Rust
- ✅ `Arc` internally makes cloning cheap

### Solution 2: Capability-Based Widget System

**New Trait Design**:

```rust
/// Base trait for all widgets
pub trait Widget {
    fn id(&self) -> WidgetId;
    fn layout_node(&self) -> &taffy::Node;
}

/// Capability traits
pub trait ParentWidget: Widget {
    fn children(&self) -> &[Box<dyn Widget>];
    fn add_child(&mut self, child: Box<dyn Widget>);
}

pub trait TextWidget: Widget {
    fn text(&self) -> &str;
    fn set_text(&mut self, text: impl Into<String>);
    fn text_style(&self) -> &TextStyle;
}

pub trait ColorWidget: Widget {
    fn color(&self) -> Color;
    fn set_color(&mut self, color: Color);
}
```

**Typed Handle System**:

```rust
pub struct WidgetHandle<T: Widget + ?Sized> {
    id: WidgetId,
    generation: u32,
    _phantom: PhantomData<*const T>,
}

// Usage - compile-time type safety!
let button: WidgetHandle<Button> = ui.add(Button::new("Click"));
let text: WidgetHandle<dyn TextWidget> = button.upcast();

// Type-safe operations
ui.set_text(text, "New text");  // ✅ Works!
ui.set_text(button, "Text");    // ✅ Also works (Button: TextWidget)
// ui.set_text(rect, "Text");   // ❌ Compile error!
```

**Benefits**:
- ✅ Compile-time type safety
- ✅ No downcast ceremony
- ✅ Clear widget capabilities
- ✅ Generational handles prevent use-after-free

### Solution 3: RenderContext Trait for Testing

**Abstraction Layer**:

```rust
/// Trait abstracting GPU operations
pub trait RenderContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer;
    fn create_texture(&self, desc: &TextureDescriptor) -> Texture;
    fn create_bind_group(&self, desc: &BindGroupDescriptor) -> BindGroup;
    fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> RenderPipeline;
    fn write_buffer(&self, buffer: &Buffer, offset: u64, data: &[u8]);
    // ... other GPU operations
}

/// Production implementation
impl RenderContext for GraphicsContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer {
        self.device.create_buffer(desc)
    }
    // ...
}

/// Mock implementation for testing
pub struct MockRenderContext {
    buffers: RefCell<Vec<MockBuffer>>,
    textures: RefCell<Vec<MockTexture>>,
    calls: RefCell<Vec<RenderCall>>,
}
```

**Usage in Tests**:

```rust
#[test]
fn test_ui_rendering() {
    let mock_ctx = MockRenderContext::new();
    let mut ui = UiSystem::new(&mock_ctx);

    ui.build(|root| {
        root.button("Click me").build();
    });

    ui.render(&mock_ctx);

    // Verify rendering calls without GPU!
    assert_eq!(mock_ctx.buffer_writes(), 1);
    assert!(mock_ctx.draw_calls() > 0);
}
```

**Benefits**:
- ✅ Unit tests without GPU
- ✅ Fast test execution
- ✅ CI/CD without graphics hardware
- ✅ Better test coverage (>80% target)

### Solution 4: Text/UI Decoupling

**Remove TextValue Wrapper**:

```rust
// OLD - complex dual tracking
pub struct TextWidget {
    text_value: TextValue,  // Wrapper with version
    cached_shaped: Arc<ShapedTextData>,  // Separate cache
    text_version: u64,  // Duplicate version!
}

// NEW - direct ownership
pub struct TextWidget {
    text: String,
    shaped_cache: Option<ShapedTextResult>,
    cache_valid: bool,  // Simple flag
}
```

**Benefits**:
- ✅ Simpler mental model
- ✅ Less indirection
- ✅ Easier debugging
- ✅ Reduced `Arc` overhead

### Solution 5: Improved RAII for RenderPass

**Better API**:

```rust
impl FrameContext {
    /// Returns a scoped render pass that auto-drops
    pub fn render_pass(&mut self, desc: RenderPassDescriptor) -> RenderPass<'_> {
        // RenderPass borrows &mut FrameContext, preventing finish() while active
        RenderPass {
            pass: self.encoder.begin_render_pass(&desc),
            _marker: PhantomData,
        }
    }

    pub fn finish(self) {
        // Can't call this while RenderPass exists (borrowed!)
        self.queue.submit(Some(self.encoder.finish()));
        self.surface_texture.present();
    }
}
```

**Usage**:

```rust
let mut frame = window.begin_drawing();

// No manual scoping needed!
let mut pass = frame.render_pass(desc);
ui.render(&mut pass);
drop(pass);  // Explicit or auto-drop

frame.finish();  // Compile error if pass still exists!
```

**Benefits**:
- ✅ Compile-time enforcement
- ✅ No manual `{ }` blocks
- ✅ Better error messages
- ✅ More idiomatic Rust

## Additional Improvements

### Pipeline Caching

Add `PipelineCache` to avoid redundant pipeline creation:

```rust
pub struct PipelineCache {
    render_pipelines: HashMap<PipelineDescriptor, RenderPipeline>,
    compute_pipelines: HashMap<ComputePipelineDescriptor, ComputePipeline>,
}
```

**Benefits**: Faster startup, reduced GPU memory thrashing

### Error Handling

Comprehensive error types with `Result`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Failed to create surface: {0}")]
    SurfaceCreation(String),
    #[error("Device lost: {0}")]
    DeviceLost(String),
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),
}

pub type RenderResult<T> = Result<T, RenderError>;
```

**Benefits**: Graceful error handling, better diagnostics

## Implementation Strategy

### Phase 1: Foundation (Week 1-2)

**Goal**: Add new APIs without breaking existing code

1. Create `astrelis-test-utils` crate
   - `RenderContext` trait
   - `MockRenderContext` implementation
   - Test helper utilities

2. Add `GraphicsContext::new()` alongside `new_sync()`
   - Keep `Box::leak` version for backwards compat
   - Add deprecation warning

3. Implement `RenderContext` trait for `GraphicsContext`

**Verification**:
- ✅ All existing examples still compile and run
- ✅ New `MockRenderContext` tests pass

### Phase 2: Widget System Redesign (Week 3-4)

**Goal**: Introduce capability-based widget system

1. Add capability traits (`Widget`, `ParentWidget`, `TextWidget`, etc.)
   - Keep old trait for compatibility
   - Mark old methods as deprecated

2. Implement `WidgetHandle<T>` with typed handles
   - Coexists with string IDs initially

3. Update core widgets (`Container`, `Button`, `Text`) to implement new traits

4. Add widget tests using `MockRenderContext`

**Verification**:
- ✅ Old API still works
- ✅ New API compiles and passes tests
- ✅ Examples work with both APIs

### Phase 3: Renderer Abstraction (Week 5-6)

**Goal**: Make renderers use `RenderContext` trait

1. Update `UiRenderer` to accept `&dyn RenderContext`
2. Update `TextRenderer` similarly
3. Add comprehensive unit tests (>80% coverage target)

**Verification**:
- ✅ Tests run without GPU in CI
- ✅ Coverage >80% for UI/text crates

### Phase 4: Text/UI Simplification (Week 7-8)

**Goal**: Remove complexity from text/UI integration

1. Remove `TextValue` wrapper
2. Consolidate shaped text caching
3. Refactor glyph_atlas integration

**Verification**:
- ✅ Text rendering still works
- ✅ Performance unchanged or improved

### Phase 5: RAII Improvements (Week 9)

**Goal**: Better lifetime management for render resources

1. Refactor `FrameContext::render_pass()`
2. Add compile-time checks
3. Update all examples

**Verification**:
- ✅ All examples compile with new API
- ✅ Clear compile errors for misuse

### Phase 6: Migration & Cleanup (Week 10)

**Goal**: Complete migration and remove deprecated code

1. Migrate all examples to new APIs
2. Remove deprecated APIs
3. Update documentation
4. Performance benchmarking

**Verification**:
- ✅ `cargo build --workspace` succeeds
- ✅ All tests pass
- ✅ All examples run correctly
- ✅ Performance benchmarks pass

## Module Organization

Planned structure:

```
astrelis-render/
  src/
    context.rs        - GraphicsContext, RenderContext trait
    frame.rs          - FrameContext, RenderPass RAII
    pipeline.rs       - Pipeline creation, caching
    buffer.rs         - Buffer utilities
    texture.rs        - Texture utilities
    error.rs          - Error types

astrelis-test-utils/  - NEW CRATE
  src/
    mock_render.rs    - MockRenderContext
    test_helpers.rs   - Test utilities

astrelis-ui/
  src/
    widget/
      base.rs         - Widget, capability traits
      handle.rs       - WidgetHandle<T>
      container.rs    - Container (impl ParentWidget)
      button.rs       - Button (impl TextWidget + ColorWidget)
      text.rs         - Text (impl TextWidget)
    renderer.rs       - UiRenderer (uses RenderContext)
    layout.rs         - Layout logic (GPU-independent)
    draw_list.rs      - Draw commands (GPU-independent)
```

## Success Criteria

- ✅ No `Box::leak` in `GraphicsContext` (zero memory leaks)
- ✅ Widget system uses typed handles (compile-time safety)
- ✅ UI/text tests run without GPU (>80% coverage)
- ✅ RenderPass RAII prevents footguns (compile-time checks)
- ✅ All examples work with new APIs
- ✅ Performance same or better than before
- ✅ Documentation complete and accurate
- ✅ Zero breaking changes in existing code until Phase 6

## Risk Mitigation

### Backwards Compatibility
- All phases maintain old API alongside new
- Deprecation warnings guide migration
- Old API removed only in Phase 6
- Examples updated incrementally

### Performance
- Benchmark each phase
- Profile before/after changes
- Optimize hot paths (layout, text shaping)
- Pipeline caching for startup performance

### Testing Coverage
- Target >80% coverage for UI/text crates
- Mock-based unit tests (fast, no GPU)
- Integration tests with real GPU
- Visual regression tests for examples

## Summary

This redesign transforms Astrelis from a proof-of-concept architecture to a production-ready engine:

- **Proper lifetime management** - No leaks, idiomatic Rust
- **Type-safe widget system** - Compile-time guarantees
- **Testable GPU code** - Fast unit tests without hardware
- **Simpler text/UI integration** - Reduced complexity
- **Better RAII** - Compile-time safety for resources

The staged migration ensures existing code continues working while new APIs are introduced incrementally. By Week 10, Astrelis will have a modern, production-ready architecture suitable for both game development and as a reference implementation.
