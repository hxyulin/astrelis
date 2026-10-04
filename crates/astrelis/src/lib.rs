//! Indexed, colored triangle-mesh rendering built directly on wgpu.
//!
//! Astrelis handles GPU initialization, mesh uploads, surface configuration, and
//! frame submission. The application owns its windows, event loop, redraw schedule,
//! and rendering order. The library has no windowing-framework dependency.
//!
//! # Ownership and initialization
//!
//! A [`GraphicsContext`] owns a wgpu instance, one adapter, and a shared device and
//! queue. Initialize it with [`GraphicsContext::with_surface`] so adapter selection
//! accounts for the first window. Add other windows with
//! [`GraphicsContext::create_surface`]; each surface must support the same adapter.
//! Cloning shares existing GPU handles. [`GraphicsContext::headless`] initializes
//! without a window for custom GPU work.
//!
//! Each [`RenderTarget`] owns presentation state and acquires frames. A [`Frame`]
//! exclusively borrows its target and creates scoped [`RenderPass`] values. A pass
//! accepts drawing from any number of renderers on that device. A [`MeshRenderer`]
//! caches its mesh pipelines, while a [`Mesh`] owns uploaded geometry. Neither is
//! tied to a particular window; resources from different devices cannot be mixed.
//! Create mesh resources with [`GraphicsContext::create_mesh`]. Construct the
//! built-in renderer with [`MeshRenderer::new`], or use an application-defined
//! rendering component. [`Vertex::new`] constructs a CPU value.
//!
//! # Drawing a mesh
//!
//! This example uses an application-created winit window. An owned `Arc<Window>`
//! gives the target a `'static` lifetime; a borrowed window gives it the lifetime
//! of the borrow. Astrelis accepts any safe [`wgpu::SurfaceTarget`].
//!
//! ```no_run
//! use std::sync::Arc;
//! use astrelis::{FrameError, GraphicsContext, MeshRenderer, Vertex, wgpu};
//! use winit::window::Window;
//!
//! async fn draw(window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
//!     let size = window.inner_size();
//!     let (graphics, mut target) = GraphicsContext::with_surface(
//!         window, size.width, size.height,
//!     ).await?;
//!     let mut renderer = MeshRenderer::new(&graphics);
//!     let triangle = graphics.create_mesh(
//!         &[
//!             Vertex::new([ 0.0,  0.7, 0.0], [1.0, 0.0, 0.0, 1.0]),
//!             Vertex::new([-0.7, -0.7, 0.0], [0.0, 1.0, 0.0, 1.0]),
//!             Vertex::new([ 0.7, -0.7, 0.0], [0.0, 0.0, 1.0, 1.0]),
//!         ],
//!         &[0, 1, 2],
//!     )?;
//!
//!     // Keep these resources in application state. Acquire a frame on redraw.
//!     let mut frame = match target.begin_frame() {
//!         Ok(frame) => frame,
//!         Err(FrameError::Retry) => {
//!             // Schedule a later redraw in the application's event loop.
//!             return Ok(());
//!         }
//!         Err(FrameError::Suspended) => return Ok(()),
//!         Err(error) => return Err(error.into()),
//!     };
//!     {
//!         let mut pass = frame.begin_pass(wgpu::LoadOp::Clear(wgpu::Color::BLACK))?;
//!         renderer.draw(&mut pass, &triangle)?;
//!     } // Ends the pass without submitting.
//!     let _submission = frame.present()?;
//!     Ok(())
//! }
//! ```
//!
//! # Multiple renderers and windows
//!
//! The frame and pass do not borrow a renderer. Several renderers can contribute
//! to one pass, in application-defined order, followed by one presentation.
//!
//! ```no_run
//! use astrelis::{Mesh, RenderTarget, MeshRenderer, wgpu};
//!
//! fn draw_layers(
//!     target: &mut RenderTarget<'_>,
//!     background_renderer: &mut MeshRenderer,
//!     overlay_renderer: &mut MeshRenderer,
//!     background: &Mesh,
//!     overlay: &Mesh,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     // This helper propagates acquisition failures for its caller to handle.
//!     let mut frame = target.begin_frame()?;
//!     {
//!         let mut pass = frame.begin_pass(wgpu::LoadOp::Clear(wgpu::Color::BLACK))?;
//!         background_renderer.draw(&mut pass, background)?;
//!         overlay_renderer.draw(&mut pass, overlay)?;
//!     }
//!     frame.present()?;
//!     Ok(())
//! }
//! ```
//!
//! For additional windows, call `graphics.create_surface(window, width, height)`
//! and keep each target in application state. All compatible targets can reuse
//! the same renderers and meshes.
//!
//! # Passes and presentation
//!
//! The first pass must use [`wgpu::LoadOp::Clear`]. Later passes may use
//! [`wgpu::LoadOp::Load`] to preserve earlier drawing or clear again deliberately.
//! All passes store their results. A clear-only pass with no draws is valid.
//! [`Frame::present`] consumes the frame, submits its commands, and requests
//! presentation once, returning a submission index rather than waiting for GPU
//! completion. Dropping a frame discards recorded work without submission or
//! presentation. A frame with no clear pass cannot be presented.
//!
//! On resize, call [`RenderTarget::resize`] with physical pixel dimensions when no
//! frame is active. Acquisition returns `Result<Frame, FrameError>` directly.
//! [`FrameError::Retry`] requires a later redraw;
//! [`FrameError::Suspended`] waits for resize or visibility. Zero dimensions
//! suspend acquisition. Outdated surfaces are reconfigured once before retrying.
//! For [`FrameError::SurfaceLost`], replace the target through the context's
//! `create_surface` method with the same window and current size.
//!
//! # Coordinates and colors
//!
//! Vertices use clip-space X/Y in `[-1, 1]`, Z in `[0, 1]`, and linear,
//! straight-alpha RGBA colors. Indices are `u32` triangle lists. The fragment shader
//! interpolates vertex color and premultiplies it for alpha blending. Draws execute
//! in recording order without depth testing or face culling. This initial API has
//! a fixed vertex-color pipeline and no material, scene, or display-list layer.
//!
//! # wgpu interoperability
//!
//! [`wgpu`] reexports the exact wgpu version used by Astrelis. The context exposes
//! raw handles, and meshes expose vertex and index buffers. [`RenderPass::as_wgpu`]
//! allows custom renderers to record into the same pass. Each renderer must establish
//! the GPU state it needs; the built-in renderer restores its pipeline, geometry
//! bindings, full viewport, and full scissor rectangle on each draw.
//! [`GraphicsContext::request`] accepts a custom instance;
//! [`GraphicsContext::from_wgpu`] adopts an application-configured device and queue.
//! Default initialization requires no optional features. Normal rendering does
//! not wait for GPU completion; surface reconfiguration may synchronize with GPU work.

mod context;
mod error;
mod frame;
mod mesh;
mod mesh_renderer;
mod pass;
mod target;

pub use context::GraphicsContext;
pub use error::Error;
pub use frame::{Frame, FrameError};
pub use mesh::{Mesh, Vertex};
pub use mesh_renderer::MeshRenderer;
pub use pass::RenderPass;
pub use target::{RenderTarget, SurfaceTarget};

/// The exact wgpu version used by Astrelis, available for GPU interoperability.
pub use wgpu;
