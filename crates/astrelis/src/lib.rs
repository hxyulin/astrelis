//! Indexed triangle-mesh rendering with custom materials, built directly on wgpu.
//!
//! Astrelis handles GPU initialization, mesh uploads, surfaces, offscreen framebuffers, and
//! frame submission. The application owns its windows, event loop, redraw schedule,
//! and rendering order. The library has no windowing-framework dependency.
//!
//! # Ownership and initialization
//!
//! A [`GraphicsContext`] owns a wgpu instance, one adapter, and a shared device and
//! queue. Initialize it with [`GraphicsContext::with_surface`] so adapter selection
//! accounts for the first window. Add other windows with
//! [`GraphicsContext::create_surface`]; each surface must support the same adapter.
//! [`SurfaceOptions`] selects the initial physical size and sample count; its
//! constructor defaults to one sample per pixel. Creation validates options and
//! caches the usable sample counts for the selected surface format and device.
//! Cloning shares existing GPU handles. [`GraphicsContext::headless`] initializes
//! without a window for custom GPU work.
//!
//! Each [`RenderTarget`] owns a surface or a [`Framebuffer`]. A [`Frame`]
//! exclusively borrows its target and configures scoped [`RenderPass`] values
//! through [`RenderPassBuilder`]. A pass
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
//! use astrelis::{FrameError, GraphicsContext, MeshRenderer, SurfaceOptions, Vertex, wgpu};
//! use winit::window::Window;
//!
//! async fn draw(window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
//!     let size = window.inner_size();
//!     let (graphics, mut target) = GraphicsContext::with_surface(
//!         window, SurfaceOptions::new(size.width, size.height),
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
//!         let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
//!         renderer.draw(&mut pass, &triangle)?;
//!     } // Ends the pass without submitting.
//!     let _submission = frame.finish()?;
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
//!         let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
//!         background_renderer.draw(&mut pass, background)?;
//!         overlay_renderer.draw(&mut pass, overlay)?;
//!     }
//!     frame.finish()?;
//!     Ok(())
//! }
//! ```
//!
//! For additional windows, pass the window and [`SurfaceOptions`] to
//! `graphics.create_surface` and keep each target in application state. All
//! compatible targets can reuse the same renderers and meshes.
//!
//! # Passes and presentation
//!
//! `frame.render_pass().begin()?` clears transparent black, stores the results,
//! and uses the full viewport and scissor rectangle. Configure the builder before
//! recording with `label`, `clear_color`, `load`, `viewport`, or `scissor_rect`.
//! The last clear/load selection wins. Every pass has the same defaults; use
//! [`RenderPassBuilder::load`] explicitly to preserve earlier drawing.
//! Dropping a builder records nothing. Rejected configuration leaves the frame
//! unchanged, so it can be corrected and retried.
//!
//! ```no_run
//! use astrelis::{Error, Frame, Mesh, MeshRenderer, wgpu};
//! fn draw_clipped(
//!     frame: &mut Frame<'_, '_>,
//!     renderer: &mut MeshRenderer,
//!     mesh: &Mesh,
//! ) -> Result<(), Error> {
//!     let mut pass = frame.render_pass()
//!         .label("clipped meshes")
//!         .clear_color(wgpu::Color::BLACK)
//!         .scissor_rect(0, 0, 100, 100)
//!         .begin()?;
//!     renderer.draw(&mut pass, mesh)?;
//!     // Change clipping for subsequent draws, independently of the renderer.
//!     pass.set_scissor_rect(10, 10, 80, 80)?;
//!     renderer.draw(&mut pass, mesh)?;
//!     Ok(())
//! }
//! ```
//!
//! Viewports use physical pixels and a depth range; scissor rectangles must fit
//! the attachment, and zero dimensions clip all drawing. The active pass exposes
//! [`RenderPass::set_viewport`] and [`RenderPass::set_scissor_rect`]. Mesh renderers
//! respect these choices across draws. An empty clear pass is valid.
//! [`Frame::finish`] consumes the frame and submits its commands once. Surface
//! frames also request presentation; framebuffer frames submit without presenting.
//! It returns a submission index without waiting for GPU completion. Dropping a
//! frame discards recorded work without submission or presentation. A surface
//! frame with no clear pass for its default destination cannot be finished.
//!
//! On resize, call [`RenderTarget::resize`] with physical pixel dimensions when no
//! frame is active. Acquisition returns `Result<Frame, FrameError>` directly.
//! [`FrameError::Retry`] requires a later redraw;
//! [`FrameError::Suspended`] waits for resize or visibility. Zero dimensions
//! suspend acquisition. Outdated surfaces are reconfigured once before retrying.
//! For [`FrameError::SurfaceLost`], replace the target through the context's
//! `create_surface` method with the same window and current size.
//!
//! # Offscreen framebuffers
//!
//! [`Framebuffer`] is a persistent GPU resource created with
//! [`GraphicsContext::create_framebuffer`]. [`FramebufferOptions`] selects its
//! physical size, color format, output usages, and optional MSAA. Defaults are
//! linear RGBA8, one sample, and rendering/sampling usages. Add `COPY_SRC` for
//! texture copies or readback. Creation validates the entire configuration before
//! allocation and caches usable sample counts, including for zero-sized resources.
//!
//! ```no_run
//! use astrelis::{FramebufferOptions, GraphicsContext, MeshRenderer, Vertex, wgpu};
//! async fn offscreen() -> Result<(), Box<dyn std::error::Error>> {
//!     let graphics = GraphicsContext::headless().await?;
//!     let mut framebuffer = graphics.create_framebuffer(
//!         FramebufferOptions::new(256, 256).sample_count(4),
//!     )?;
//!     let mesh = graphics.create_mesh(&[
//!         Vertex::new([0.0, 0.7, 0.0], [1.0; 4]),
//!         Vertex::new([-0.7, -0.7, 0.0], [1.0; 4]),
//!         Vertex::new([0.7, -0.7, 0.0], [1.0; 4]),
//!     ], &[0, 1, 2])?;
//!     let mut renderer = MeshRenderer::new(&graphics);
//!     let mut frame = framebuffer.begin_frame()?;
//!     {
//!         let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
//!         renderer.draw(&mut pass, &mesh)?;
//!     }
//!     frame.finish()?; // Submit without a surface or presentation.
//!     let _resolved_view = framebuffer.color_view()?;
//!     Ok(())
//! }
//! ```
//!
//! [`Frame::render_to`] writes an additional framebuffer in the same encoder:
//!
//! ```no_run
//! use astrelis::{Error, Frame, Framebuffer, Mesh, MeshRenderer};
//! fn draw_layer(frame: &mut Frame<'_, '_>, layer: &mut Framebuffer,
//!     renderer: &mut MeshRenderer, mesh: &Mesh) -> Result<(), Error> {
//!     {
//!         let mut pass = frame.render_to(layer).begin()?;
//!         renderer.draw(&mut pass, mesh)?;
//!     }
//!     // An application-defined renderer can now sample layer.color_view()?
//!     // in another pass. All commands share the caller's eventual finish().
//!     Ok(())
//! }
//! ```
//!
//! The output stays single-sampled; MSAA resolves at each pass end. Framebuffer
//! load passes can preserve contents across submitted recordings. A first load
//! requires a submitted clear or an earlier clear in the same frame. Abandoning
//! recording does not initialize the resource or change submitted contents.
//! Clearing an additional framebuffer never initializes a surface frame's default
//! destination. [`Frame::finish`] rejects that surface until its own clear is recorded.
//!
//! Resize or MSAA changes replace framebuffer attachments and discard contents.
//! Update application bind groups with the new view, and clear before loading
//! again. Unchanged settings reuse attachments. Zero dimensions release resources:
//! acquisition returns `Suspended`, while view/texture access and `render_to(...).begin()`
//! return [`Error::TargetSuspended`]. View and texture access return `Result` because
//! no attachment exists while suspended. The default target cannot resize while
//! borrowed by a frame; additional targets cannot resize during their active passes.
//! Make all attachment changes between recordings. Pending commands and cloned raw
//! handles continue to reference previous attachment generations.
//!
//! A framebuffer can also be owned by [`RenderTarget::Framebuffer`] for generic
//! destination handling. Integer formats support custom shaders; the default
//! [`MeshRenderer::draw`] rejects formats incompatible with floating-point output
//! and alpha blending. [`MeshRenderer::draw_with_material`] can use an integer
//! fragment output with a matching format and blending disabled.
//! Framebuffer contents require sampling usages to be bound as a shader texture;
//! avoid reading from and writing to the same framebuffer in one pass.
//!
//! # Multisample antialiasing
//!
//! Select MSAA during creation with [`SurfaceOptions::sample_count`]. Unsupported
//! requests return [`Error::UnsupportedSampleCount`] without choosing a fallback.
//! One sample disables MSAA; zero-sized targets validate the count but defer allocation.
//!
//! ```no_run
//! use std::sync::Arc;
//! use astrelis::{GraphicsContext, SurfaceOptions};
//! use winit::window::Window;
//! async fn draw_with_msaa(window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
//!     let size = window.inner_size();
//!     let options = SurfaceOptions::new(size.width, size.height).sample_count(4);
//!     let (_graphics, mut target) = GraphicsContext::with_surface(window, options).await?;
//!     // Usable counts are cached during creation; this borrows them without allocation.
//!     let supported = target.supported_sample_counts();
//!     assert!(supported.contains(&4));
//!     // An in-game settings change, between frames, without replacing the surface.
//!     target.set_sample_count(1)?;
//!     Ok(())
//! }
//! ```
//!
//! The surface image remains single-sampled. The target reuses a multisampled
//! color attachment and recreates it when the size or sample count changes. Each
//! pass resolves into the surface image and stores the multisampled contents, so
//! later `load()` passes preserve individual samples and edge coverage. Every
//! frame still starts with a clear pass. Zero dimensions release the attachment
//! and suspend acquisition; restoring the size recreates it with the selected count.
//! [`RenderTarget::supported_sample_counts`] returns a borrowed slice of cached
//! capabilities. [`RenderTarget::set_sample_count`] validates against that cache
//! and recreates the attachment only if the count changes. Neither capability
//! lookup nor attachment allocation occurs during ordinary frame/pass recording.
//!
//! [`MeshRenderer`] automatically caches pipelines by format and sample count.
//! Different targets can use different counts with the same renderer. Custom
//! renderers must match [`RenderPass::sample_count`] in their pipeline's
//! [`wgpu::MultisampleState`]. MSAA cannot change within a frame.
//!
//! Support includes render-attachment and resolve capabilities and accounts for
//! enabled device features. Default initialization requires no optional features.
//! Native hardware may offer extra counts through
//! [`wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`]; enable that feature
//! on an application-created device and adopt it with [`GraphicsContext::from_wgpu`]
//! to use them. A recreated surface is a new target: pass the desired count in its
//! creation options and handle [`Error::UnsupportedSampleCount`] if unsupported.
//!
//! # Coordinates and colors
//!
//! Default shading interprets positions as clip-space X/Y in `[-1, 1]`, Z in
//! `[0, 1]`, and colors as linear, straight-alpha RGBA. Its fragment shader
//! interpolates vertex color and premultiplies it for alpha blending. Indices are
//! `u32` triangle lists. Draws execute in recording order without depth testing;
//! default shading does not cull faces. A custom material can transform positions,
//! define color conventions, and select face culling. The vertex layout remains
//! fixed; this API has no scene or display-list layer.
//!
//! # Materials and custom mesh shaders
//!
//! A [`Material`] retains an application-created [`wgpu::ShaderModule`], explicit
//! binding layouts, entry-point names, blending, write masks, and face culling.
//! Create one with [`GraphicsContext::create_material`] and [`MaterialOptions`].
//! Resources stay on the context's device; materials and meshes can be reused
//! with independent renderers and compatible targets. Shader modules use wgpu
//! directly so source loading and compilation diagnostics remain application-owned.
//!
//! ```no_run
//! use astrelis::{GraphicsContext, Material, MaterialOptions, wgpu};
//! fn material(graphics: &GraphicsContext) -> Material {
//!     let shader = graphics.device().create_shader_module(wgpu::ShaderModuleDescriptor {
//!         label: Some("application mesh shader"),
//!         source: wgpu::ShaderSource::Wgsl(r#"
//!             @vertex fn vertex_main(@location(0) position: vec3<f32>)
//!                 -> @builtin(position) vec4<f32> { return vec4(position, 1.0); }
//!             @fragment fn fragment_main() -> @location(0) vec4<f32> {
//!                 return vec4(0.2, 0.6, 1.0, 1.0);
//!             }
//!         "#.into()),
//!     });
//!     graphics.create_material(MaterialOptions::new(&shader).blend(None))
//! }
//! ```
//!
//! `renderer.draw(pass, mesh)` uses the default vertex-color material.
//! `renderer.draw_with_material(pass, mesh, material)` selects custom shading;
//! both use the same fixed vertex layout and pass viewport/scissor settings.
//! Shader inputs are location 0 `vec3<f32>` position and location 1 `vec4<f32>`
//! color; unused inputs can be omitted. The vertex shader may transform positions.
//! Fragment output at location 0 must match the attachment format. Premultiplied
//! alpha blending is the material default; custom shaders must premultiply their
//! output or select a different blend state. No depth attachment is used.
//!
//! Supply explicit bind-group layouts with [`MaterialOptions::bind_group_layouts`].
//! The application owns buffers, textures, groups, and their updates, and binds
//! groups through [`RenderPass::as_wgpu`] before drawing. Dynamic offsets and
//! per-draw binding changes work as in wgpu. They do not create new materials or
//! pipelines. The standalone `materials` example updates a tint uniform on Space.
//!
//! Pipelines are cached per renderer by material identity, format, and sample count.
//! Cloned materials reuse entries; new materials have independent entries, even
//! when their settings are identical. Cached pipelines live until the renderer is
//! dropped. Prepare before rendering with [`MeshRenderer::prepare`] for default
//! shading or [`MeshRenderer::prepare_material`] for custom shading, passing the
//! target's format and count. Cache hits perform no capability queries, allocation,
//! or pipeline creation. Preparation does not submit or wait for GPU completion
//! and cannot guarantee that a driver defers no work until the first draw.
//!
//! Astrelis returns device/format/sample-count errors directly. Raw shader,
//! layout, interface, and bind-group validation follows wgpu's error scopes and
//! uncaptured error handler; a successful preparation `Result` alone does not
//! certify shader validity. A validation scope can check preparation before use:
//!
//! ```no_run
//! use astrelis::{Error, Material, MeshRenderer, wgpu};
//! async fn prepare_checked(device: &wgpu::Device, renderer: &mut MeshRenderer,
//!     material: &Material, format: wgpu::TextureFormat, samples: u32,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
//!     let prepared: Result<(), Error> = renderer.prepare_material(material, format, samples);
//!     let validation_error = scope.pop().await;
//!     prepared?;
//!     if let Some(error) = validation_error { return Err(error.into()); }
//!     Ok(())
//! }
//! ```
//!
//! # wgpu interoperability
//!
//! [`wgpu`] reexports the exact wgpu version used by Astrelis. The context exposes
//! raw handles, and meshes expose vertex and index buffers. [`RenderPass::as_wgpu`]
//! allows custom renderers to record into the same pass. Each renderer must establish
//! the GPU state it needs; the built-in renderer restores its pipeline, geometry
//! bindings, and the pass's selected viewport and scissor on each draw. Raw viewport
//! and scissor changes do not update the wrapper's state; use the wrapped setters
//! to control mesh rendering. [`Frame::encoder`] provides raw command recording
//! before, between, and after passes. It is unavailable while a pass is in use.
//!
//! ```no_run
//! use astrelis::{Error, Frame, wgpu};
//! fn copy_around_drawing(
//!     mut frame: Frame<'_, '_>,
//!     source: &wgpu::Buffer,
//!     destination: &wgpu::Buffer,
//! ) -> Result<wgpu::SubmissionIndex, Error> {
//!     // Buffers must have the matching copy usages and belong to the frame's device.
//!     frame.encoder().copy_buffer_to_buffer(source, 0, destination, 0, 4);
//!     {
//!         let _pass = frame.render_pass().begin()?;
//!     }
//!     frame.encoder().copy_buffer_to_buffer(source, 0, destination, 4, 4);
//!     frame.finish()
//! }
//! ```
//!
//! Wrapped passes currently have one color attachment and no depth/stencil,
//! multiple render targets, or custom mip/layer selection. Raw encoder access
//! supports those passes with application-owned attachments. It does not expose
//! the acquired surface image or managed MSAA view; render more elaborate scenes
//! offscreen and composite into a wrapped surface pass. Raw framebuffer writes
//! do not establish the initialization tracked by wrapped `load()` passes. Stay
//! with raw passes for those writes, or initialize through a wrapped clear first.
//! Surface frame submission and presentation are owned together by `finish()`;
//! externally batching multiple frames' command buffers is outside this API.
//!
//! [`GraphicsContext::request`] accepts a custom instance;
//! [`GraphicsContext::from_wgpu`] adopts an application-configured device and queue.
//! Default initialization requires no optional features. Normal rendering does
//! not wait for GPU completion; surface reconfiguration may synchronize with GPU work.

mod context;
mod error;
mod frame;
mod framebuffer;
mod material;
mod mesh;
mod mesh_renderer;
mod pass;
mod target;

pub use context::GraphicsContext;
pub use error::Error;
pub use frame::{Frame, FrameError};
pub use framebuffer::{Framebuffer, FramebufferOptions};
pub use material::{Material, MaterialOptions};
pub use mesh::{Mesh, Vertex};
pub use mesh_renderer::MeshRenderer;
pub use pass::{RenderPass, RenderPassBuilder};
pub use target::{RenderTarget, SurfaceOptions, SurfaceTarget};

/// The exact wgpu version used by Astrelis, available for GPU interoperability.
pub use wgpu;
