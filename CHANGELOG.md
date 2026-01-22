# Changelog

All notable changes to the Astrelis Game Engine will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-01-22

### Added

#### Style API Constraint Integration (`astrelis-ui`)
- **Style methods now accept `Constraint`** - All dimension-related Style methods (width, height, min_width, etc.) now accept `Constraint` values in addition to raw `f32` values
  - Enables advanced responsive layouts with calc expressions
  - Backward compatible - existing `f32` usage continues to work
  - Supports all constraint types: Px, Percent, Auto, viewport units (Vw, Vh, Vmin, Vmax), calc, min, max, clamp

- **New constraint exports** - Added top-level exports for constraint system:
  - `Constraint`, `CalcExpr` - Core constraint types
  - `ConstraintResolver`, `ResolveContext` - Resolution utilities
  - Builder helpers: `px`, `calc`, `min2`, `min_of`, `max2`, `max_of`, `clamp`

#### Frame Context Improvements (`astrelis-render`)
- **Fallible access methods** - Added try_ variants to FrameContext and RenderPass:
  - `try_surface()`, `has_surface()` - Check surface availability
  - `try_encoder()`, `has_encoder()` - Check encoder availability
  - `try_encoder_and_surface()` - Get both if available
  - `try_descriptor()`, `is_valid()` on RenderPass

#### Asset System Improvements (`astrelis-assets`)
- **Thread-safety documentation** - Comprehensive documentation for AssetServer thread-safety requirements and recommended patterns

### Changed

#### Breaking: Widget System Consolidation (`astrelis-ui`)
- **Deprecated capability-based widget system** - The experimental `widget/` module is now marked as `#[doc(hidden)]` and internal
  - The `widgets` module (widgets.rs) is the stable, actively-used widget system
  - Removed confusing `Cap`-prefixed exports (`CapButton`, `CapContainer`, `CapText`)
  - Types like `WidgetHandle`, `WidgetStorage`, `TextWidget`, `ParentWidget`, `ColorWidget` are no longer re-exported
  - **Migration**: Use `widgets::Button`, `widgets::Container`, `widgets::Text` instead

#### Breaking: Style Methods Accept Constraint
- Style dimension methods now accept `impl Into<Constraint>` instead of `impl Into<Length>`
  - This is backward compatible for simple values (f32, Length)
  - **Note**: Viewport units (Vw, Vh, Vmin, Vmax) and complex constraints (calc, min, max, clamp) must be resolved before passing to Taffy. The UI layout system handles this automatically.

### Upgrade Guide

#### For Users of Cap-prefixed Widget Types

```rust
// Before (0.1.1):
use astrelis_ui::{CapButton, CapContainer, CapText};

// After (0.1.2):
// These types were experimental and not integrated.
// Use the widgets module instead:
use astrelis_ui::widgets::{Button, Container, Text};
```

#### Using Constraint in Style

```rust
use astrelis_ui::{Style, Constraint, calc, percent, px, min2};

// Simple usage (unchanged)
let style = Style::new().width(400.0);

// Using Constraint directly
let style = Style::new()
    .width(Constraint::Percent(50.0))
    .height(Constraint::Auto);

// Using constraint builders
let style = Style::new()
    .width(min2(percent(50.0), px(400.0)));  // min(50%, 400px)
```

---

## [0.1.1] - 2026-01-21

### Added

#### Constraint System (`astrelis-ui`)
- **CSS-like constraint expressions** - Responsive dimension values for layouts
  - `Constraint::Px(f32)` - Fixed pixel values
  - `Constraint::Percent(f32)` - Percentage of parent dimension
  - `Constraint::Auto` - Automatic sizing based on content
  - `Constraint::Vw(f32)` - Percentage of viewport width
  - `Constraint::Vh(f32)` - Percentage of viewport height
  - `Constraint::Vmin(f32)` - Percentage of minimum viewport dimension
  - `Constraint::Vmax(f32)` - Percentage of maximum viewport dimension
  - `Constraint::Calc(CalcExpr)` - Arithmetic expressions like `calc(100% - 40px)`
  - `Constraint::Min(Vec)` - Minimum value: `min(50%, 400px)`
  - `Constraint::Max(Vec)` - Maximum value: `max(200px, 30%)`
  - `Constraint::Clamp { min, val, max }` - Bounded value: `clamp(100px, 50%, 800px)`

- **Constraint builder helpers** - Ergonomic constructors for constraint expressions
  - `px()`, `percent()`, `vw()`, `vh()`, `vmin()`, `vmax()` - Simple value constructors
  - `calc()` - Build calc expressions with operator overloading (`+`, `-`, `*`, `/`)
  - `min2()`, `min3()`, `max2()`, `max3()` - Min/max with 2-3 values
  - `clamp()` - Bounded value constructor

- **Constraint resolver** - Resolves constraints to absolute pixel values given parent size and viewport context

#### Viewport Context (`astrelis-ui`)
- **ViewportContext** - Context for resolving viewport-relative units (vw, vh, vmin, vmax)
  - `ViewportContext::new(viewport_size)` - Create from viewport dimensions
  - `width()`, `height()`, `vmin()`, `vmax()` - Accessors for viewport metrics

#### Overflow Clipping (`astrelis-ui`)
- **Overflow style property** - Control content overflow behavior
  - `Overflow::Visible` - Content renders beyond bounds (default)
  - `Overflow::Hidden` - Content clipped at container bounds
  - `Overflow::Scroll` - Scrollable with scrollbars (planned)
  - `overflow_x()`, `overflow_y()`, `overflow_xy()` - Style builder methods

- **ClipRect** - GPU scissor-based clipping rectangles
  - `ClipRect::infinite()` - No clipping
  - `ClipRect::from_bounds()` - Create from position and size
  - `intersect()` - Combine nested clip regions
  - `contains()` - Point containment testing
  - Automatic conversion to physical pixel coordinates for GPU scissor tests

#### Examples
- **constraint_showcase.rs** - Demonstrates constraint expressions and viewport units
- **overflow_demo.rs** - Demonstrates overflow clipping with scrollable lists

### Fixed

- **Overflow clipping on dirty updates** - Fixed clipping not persisting through incremental updates
  - Root cause: Dirty update path bypassed clip rect computation, always using infinite clip
  - Added `compute_inherited_clip()` method to walk up tree and compute proper inherited clips
  - Clipping now persists correctly through window resizes and incremental updates

## [0.1.0] - 2025-01-21

### Added

#### Error Handling Infrastructure
- **TextError enum** (`astrelis-text`) - Comprehensive error types for text rendering failures
  - `FontLoadError` - Font loading failures
  - `FontFileNotFound` - Missing font files
  - `InvalidFontData` - Corrupted font data
  - `LockPoisoned` - RwLock poisoning recovery
  - `ShapingError` - Text shaping failures
  - `BufferAllocationFailed` - GPU buffer allocation errors
  - `AtlasFull` - Texture atlas capacity exceeded
  - `GpuResourceError` - GPU resource creation failures
  - `InvalidRange` - Invalid text range operations
  - `IoError` - File I/O errors

- **GraphicsError extensions** (`astrelis-render`) - Surface and rendering error handling
  - `SurfaceCreationFailed` - Window surface initialization errors
  - `SurfaceConfigurationFailed` - Surface configuration errors
  - `SurfaceTextureAcquisitionFailed` - Frame acquisition errors

- **Graceful error recovery** - Lock poisoning recovery in multi-threaded text rendering
  - `lock_or_recover!` macro for consistent RwLock error handling
  - Automatic lock clearing and logging on poisoned lock detection

#### Text Rendering Features
- **Line style decorations** (`astrelis-text`) - Complete text decoration support
  - `LineStyle::Solid` - Standard solid lines
  - `LineStyle::Dashed` - Dashed underlines and strikethrough
  - `LineStyle::Dotted` - Dotted decorations
  - `LineStyle::Wavy` - Wavy underlines (spell-check style)
  - CPU tessellation with proper spacing and sine-wave patterns

#### UI System Features
- **Virtual scrolling** (`astrelis-ui`) - Memory-efficient rendering for large lists
  - `UiTree::remove_node()` - Proper node lifecycle management
  - `UiTree::remove_child()` - Child removal with Taffy cleanup
  - `UiTree::set_position_offset()` - Position offsetting for scroll transforms
  - Completed node removal and transform updates implementation

### Changed

#### Breaking API Changes
- **Result-based APIs** - All fallible operations now return `Result<T, Error>`
  - `WindowContext::new()` returns `Result<Self, GraphicsError>`
  - `RenderableWindow::new()` returns `Result<Self, GraphicsError>`
  - `WindowManager::create_window()` returns `Result<WindowId, GraphicsError>`
  - Text rendering methods handle lock poisoning gracefully

- **Error propagation** - Errors propagate from low-level (WGPU) to high-level APIs
  - Applications can now handle GPU device loss, surface loss, and other failures
  - No more unwrap/expect in production code paths

#### Improvements
- **Lock poisoning resilience** - Text rendering continues even if locks are poisoned
  - Automatic recovery with logging
  - Prevents cascade failures in multi-threaded environments

- **Surface texture acquisition** - Improved error messages for surface loss
  - Clear diagnostics for window minimize/restore issues
  - Guidance for handling surface recreation

### Fixed

- **Text system unwrap removal** - Replaced all `.unwrap()` and `.expect()` calls
  - `sdf.rs` - Lock poisoning handling in SDF renderer
  - `hybrid.rs` - Graceful error handling in hybrid renderer
  - `bitmap.rs` - Proper error propagation in bitmap renderer
  - `editor.rs` - Safe selection handling with `map_or`

- **Render system unwrap removal** - Production-ready error handling
  - Surface creation errors properly reported
  - Configuration failures handled gracefully
  - Frame acquisition errors logged with context

- **Example compatibility** - All 29+ examples updated
  - Handle new `Result`-based APIs with `.expect()` calls
  - Fixed `FrameTime` import issues across all examples
  - All examples compile and run successfully

### Documentation

- **Error handling patterns** - Clear examples of error recovery
  - Lock poisoning recovery in text rendering
  - Surface loss handling in window management
  - GPU error propagation to application layer

## Release Notes

### Production Readiness (v0.1.0)

This release marks the completion of **Priority 1** production readiness work for Astrelis v2.0:

✅ **Error Handling Infrastructure** - Production-ready
- Comprehensive error types with proper `std::error::Error` implementation
- Graceful degradation for lock poisoning, surface loss, and GPU errors
- All production code uses `Result` instead of unwrap/expect

✅ **Text Rendering** - Complete
- All line style decorations implemented (dashed, dotted, wavy)
- Lock poisoning recovery for multi-threaded safety
- 267 unit tests passing

✅ **UI System** - Production-ready
- Virtual scrolling with proper node lifecycle management
- Memory-efficient rendering for large lists (10,000+ items)
- Complete dirty flag optimization system

✅ **Build & Test Status**
- `cargo build --workspace` - 0 errors
- `cargo test --workspace --lib` - 267 tests passing
- `cargo build --examples` - All 29+ examples compile

### Upgrade Guide

#### For Applications Using RenderableWindow

```rust
// Before (0.0.1):
let window = RenderableWindow::new(window, context);

// After (0.1.0):
let window = RenderableWindow::new(window, context)
    .expect("Failed to create renderable window");
// Or handle the error:
let window = match RenderableWindow::new(window, context) {
    Ok(w) => w,
    Err(e) => {
        eprintln!("Failed to create window: {}", e);
        // Handle error appropriately
        return;
    }
};
```

#### For Applications Using WindowManager

```rust
// Before (0.0.1):
let window_id = window_manager.create_window(ctx, descriptor);

// After (0.1.0):
let window_id = window_manager.create_window(ctx, descriptor)
    .expect("Failed to create window");
// Or handle the error:
let window_id = match window_manager.create_window(ctx, descriptor) {
    Ok(id) => id,
    Err(e) => {
        eprintln!("Window creation failed: {}", e);
        // Handle error appropriately
        return;
    }
};
```

### Next Steps (Planned for v0.2.0)

**Priority 2 - Widget Expansion:**
- Data table/grid widget for dashboard UIs
- Form validation framework for production forms
- Input masking utilities (phone numbers, dates)

**Priority 3 - Documentation & Examples:**
- Production patterns documentation
- Video integration example (4K HEVC streaming)
- API stability audit for v1.0

---

## [Unreleased]

### Planned for v0.2.0
- Data table/grid widget implementation
- Form validation framework
- Production patterns documentation
- Video integration example

---

[0.1.2]: https://github.com/hxyulin/astrelis/releases/tag/v0.1.2
[0.1.1]: https://github.com/hxyulin/astrelis/releases/tag/v0.1.1
[0.1.0]: https://github.com/hxyulin/astrelis/releases/tag/v0.1.0
