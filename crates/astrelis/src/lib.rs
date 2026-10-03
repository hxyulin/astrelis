//! Indexed, colored triangle-mesh rendering built directly on wgpu.
//!
//! Astrelis handles GPU initialization, mesh uploads, surface configuration, and
//! frame submission. The application owns its windows and event loop. The library
//! has no dependency on a particular windowing framework.
//!
//! # Ownership and initialization
//!
//! A [`GraphicsContext`] owns a wgpu instance, one adapter, and a shared device and
//! queue. Initialize it with [`GraphicsContext::with_surface`] so adapter selection
//! accounts for the first window. Add other windows with
//! [`GraphicsContext::create_surface`]; each surface must support the same adapter.
//! Cloning a context shares the existing handles rather than creating another GPU
//! device. [`GraphicsContext::headless`] initializes without a window.
//!
//! Each [`RenderTarget`] owns a surface's presentation configuration and physical
//! size. A [`Renderer`] can draw to multiple targets from the same context, reusing
//! its pipelines. Upload a [`Mesh`] once and borrow it for subsequent frames or for
//! other targets on that device. Resources from different devices cannot be mixed.
//!
//! # Drawing a mesh
//!
//! This example uses an application-created winit window. An owned `Arc<Window>`
//! gives the target a `'static` lifetime; a borrowed window gives it the lifetime
//! of the borrow. Astrelis accepts any safe [`wgpu::SurfaceTarget`].
//!
//! ```no_run
//! use std::sync::Arc;
//! use astrelis::{Error, FrameStatus, GraphicsContext, Mesh, Renderer, Vertex, wgpu};
//! use winit::window::Window;
//!
//! async fn draw(window: Arc<Window>) -> Result<FrameStatus, Error> {
//!     let size = window.inner_size();
//!     let (graphics, mut target) = GraphicsContext::with_surface(
//!         window, size.width, size.height,
//!     ).await?;
//!     let mut renderer = Renderer::new(&graphics);
//!     let triangle = Mesh::new(
//!         &graphics,
//!         &[
//!             Vertex::new([ 0.0,  0.7, 0.0], [1.0, 0.0, 0.0, 1.0]),
//!             Vertex::new([-0.7, -0.7, 0.0], [0.0, 1.0, 0.0, 1.0]),
//!             Vertex::new([ 0.7, -0.7, 0.0], [0.0, 0.0, 1.0, 1.0]),
//!         ],
//!         &[0, 1, 2],
//!     )?;
//!
//!     // Keep these resources in application state. Call render on redraw.
//!     renderer.render(&mut target, wgpu::Color::BLACK, &[&triangle])
//! }
//! ```
//!
//! # Multiple windows
//!
//! Create additional targets through the existing context. This reuses the device
//! and queue; neither meshes nor the renderer need to be recreated per window.
//!
//! ```no_run
//! use std::sync::Arc;
//! use astrelis::{Error, GraphicsContext, Mesh, Renderer, wgpu};
//! use winit::window::Window;
//!
//! fn draw_another_window(
//!     graphics: &GraphicsContext,
//!     renderer: &mut Renderer,
//!     mesh: &Mesh,
//!     window: Arc<Window>,
//! ) -> Result<(), Error> {
//!     let size = window.inner_size();
//!     let mut target = graphics.create_surface(window, size.width, size.height)?;
//!     // Keep the target in application state for future redraws and resizes.
//!     renderer.render(&mut target, wgpu::Color::BLACK, &[mesh])?;
//!     Ok(())
//! }
//! ```
//!
//! # Coordinates, colors, and frame lifecycle
//!
//! Vertices use clip-space X/Y in `[-1, 1]`, Z in `[0, 1]`, and linear,
//! straight-alpha RGBA colors. Indices are `u32` triangle lists. The fragment shader
//! interpolates vertex color and premultiplies it for alpha blending. Meshes draw
//! in submission order without depth testing or face culling. An empty submission
//! clears the surface. This initial API has a fixed pipeline and no scene or
//! display-list layer.
//!
//! On a resize, call [`RenderTarget::resize`] with physical pixel dimensions.
//! [`FrameStatus::Presented`] means GPU work was submitted and presentation was
//! requested, not that GPU execution has completed. [`FrameStatus::Retry`] requires
//! a later redraw; [`FrameStatus::Suspended`] waits for a resize or visibility
//! change. A zero dimension suspends rendering. Outdated surfaces are reconfigured
//! once before retrying acquisition. For [`Error::SurfaceLost`], replace the target
//! using the context's `create_surface` method with the same window and current size.
//!
//! # wgpu interoperability
//!
//! [`wgpu`] reexports the exact wgpu version used by Astrelis. The context exposes
//! its raw handles, and meshes expose their vertex and index buffers.
//! [`GraphicsContext::request`] accepts a custom instance;
//! [`GraphicsContext::from_wgpu`] adopts an application-configured device and queue.
//! Default initialization requires no optional features. Ordinary rendering does
//! not wait for GPU completion; surface reconfiguration may synchronize with GPU work.

mod context;
mod error;
mod mesh;
mod renderer;
mod target;

pub use context::GraphicsContext;
pub use error::Error;
pub use mesh::{Mesh, Vertex};
pub use renderer::{FrameStatus, Renderer};
pub use target::{RenderTarget, SurfaceTarget};

/// The exact wgpu version used by Astrelis, available for GPU interoperability.
pub use wgpu;
