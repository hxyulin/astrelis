//! Rendering resources and command recording built directly on wgpu.
//!
//! Applications own windows, event loops, redraw scheduling, and drawing order.
//! [`GraphicsContext`] wraps a wgpu instance, adapter, device, and queue. Resource
//! factories live on the context; independent [`MeshRenderer`] and [`TextureRenderer`]
//! values record into the same scoped [`RenderPass`]. No windowing dependency is
//! required by the library, and no scene graph or display list is imposed.
//!
//! # Context, targets, and frames
//!
//! Select an adapter for your first window with [`GraphicsContext::with_surface`].
//! Add compatible windows through [`GraphicsContext::create_surface`]. A cloned
//! context shares GPU handles, upload storage, and managed submission ordering.
//! [`GraphicsContext::headless`] initializes without a window. Use
//! [`GraphicsContext::from_wgpu`] to adopt application-configured features/limits.
//!
//! [`RenderTarget`] owns a surface or [`Framebuffer`]. Target options select size,
//! format where applicable, MSAA, and optional depth/stencil storage. Allocation
//! and capability checks happen at creation or reconfiguration, not on prepared
//! draws. Resize and sample-count changes replace storage only when values change.
//! Zero dimensions suspend the target without allocating attachments.
//!
//! A [`Frame`] exclusively borrows its default target. [`Frame::render_to`] adds
//! framebuffer passes to the same recording. Passes borrow the encoder until they
//! end; renderer lifetimes remain independent. [`Frame::finish`] submits once and
//! presents a surface, returning a submission index without waiting for completion.
//! Dropping a frame discards commands and parameter uploads. A surface frame must
//! initialize its output before presentation, including a resolve when MSAA is used.
//!
//! ```no_run
//! use std::sync::Arc;
//! use astrelis::{FrameError, GraphicsContext, MeshRenderer, SurfaceOptions, Vertex, wgpu};
//! use winit::window::Window;
//! async fn draw(window: Arc<Window>) -> Result<(), Box<dyn std::error::Error>> {
//!     let size = window.inner_size();
//!     let (graphics, mut target) = GraphicsContext::with_surface(
//!         window, SurfaceOptions::new(size.width, size.height).sample_count(4),
//!     ).await?;
//!     let mesh = graphics.create_mesh(&[
//!         Vertex::new([0., 0.7, 0.], [1., 0., 0., 1.]),
//!         Vertex::new([-0.7, -0.7, 0.], [0., 1., 0., 1.]),
//!         Vertex::new([0.7, -0.7, 0.], [0., 0., 1., 1.]),
//!     ], &[0, 1, 2])?;
//!     let mut renderer = MeshRenderer::new(&graphics);
//!     renderer.prepare_for_target(&target)?;
//!     // Keep resources in application state and acquire on each redraw.
//!     let mut frame = match target.begin_frame() {
//!         Ok(frame) => frame,
//!         Err(FrameError::Retry | FrameError::Suspended) => return Ok(()),
//!         Err(error) => return Err(error.into()),
//!     };
//!     {
//!         let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
//!         renderer.draw(&mut pass, &mesh)?;
//!     }
//!     frame.finish()?;
//!     Ok(())
//! }
//! ```
//!
//! Handle [`FrameError::Retry`] by scheduling another redraw, [`FrameError::Suspended`]
//! by waiting for resize/visibility, and [`FrameError::SurfaceLost`] by recreating
//! the surface from the application-owned window. An `Arc<Window>` gives a surface
//! a `'static` window lifetime; a borrowed window ties it to that borrow.
//!
//! # Attachment operations and MSAA
//!
//! `frame.render_pass().begin()?` clears transparent color, depth to 1, and stencil
//! to 0, and stores available aspects. Every pass has the same defaults.
//! [`RenderPassBuilder::load_color`] preserves only color;
//! [`RenderPassBuilder::load_all`] preserves each available aspect. Individual
//! clear/load selectors and raw operations provide independent aspect control.
//! Read-only depth/stencil use `depth_ops(None)` / `stencil_ops(None)`; materials
//! must disable writes to those aspects. [`RenderPassBuilder::without_depth_stencil`]
//! omits both aspects entirely, which also changes compatible pipeline formats.
//!
//! Targets own MSAA allocation; passes choose when to resolve and whether to store
//! samples. Resolving is enabled by default. Skip intermediate resolves with
//! `resolve(false)`, keep samples with `Store`, and resolve the final pass. `Discard`
//! can release sample contents while retaining the resolved output. A later sample
//! load then requires another clear. Depth/stencil storage is never resolved.
//!
//! ```no_run
//! use astrelis::{Error, Frame, wgpu};
//! fn passes(frame: &mut Frame<'_, '_>) -> Result<(), Error> {
//!     // The target was created with MSAA. Record scene draws in this first pass.
//!     drop(frame.render_pass().resolve(false).begin()?);
//!     // Preserve all aspects, resolve, and discard unneeded multisampled color.
//!     drop(frame.render_pass().load_all().color_ops(wgpu::Operations {
//!         load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Discard,
//!     }).begin()?);
//!     Ok(())
//! }
//! ```
//!
//! Framebuffer contents persist across submitted recordings. Managed loads require
//! initialized contents; clears/discards are tracked per aspect and storage
//! generation. Effects commit in managed submission order. Loads depending on an
//! earlier submission are checked again at finish, so an intervening discard can
//! cause finish to return an initialization error without submitting the recording.
//! Abandoned/rejected recordings do not commit effects. Shared attachment ordering
//! remains explicit: this API does not reorder recordings or manage dependencies.
//!
//! # Images and changing draw parameters
//!
//! [`TextureBinding`] owns reusable image/sampler resources. [`TextureDraw`] carries
//! per-draw destination, UVs, tint, and affine transform, independently of bindings
//! and materials. [`TextureDraw::new`] uses physical pixels relative to the active
//! viewport's top-left; [`TextureDraw::normalized`] uses viewport fractions.
//! [`UvRect`] always uses normalized source coordinates. RGB tint is linear and
//! opacity is straight alpha. Built-in shading produces premultiplied output.
//!
//! ```no_run
//! use astrelis::{GraphicsContext, RenderTarget, TextureOptions, TextureRenderer,
//!     TextureBindingOptions, TextureDraw, Rect, UvRect};
//! fn image(graphics: &GraphicsContext, target: &mut RenderTarget<'_>)
//!     -> Result<(), Box<dyn std::error::Error>> {
//!     let texture = graphics.create_texture(TextureOptions::new(1, 1))?;
//!     texture.write(&[255, 255, 255, 255])?;
//!     let mut renderer = TextureRenderer::new(graphics);
//!     let image = renderer.create_binding(texture.view(), TextureBindingOptions::new())?;
//!     renderer.prepare(&image, &target.render_format())?;
//!     let mut frame = target.begin_frame()?;
//!     {
//!         let mut pass = frame.render_pass().begin()?;
//!         renderer.draw(&mut pass, &image, TextureDraw::new(Rect::new(10., 20., 80., 60.))
//!             .uv(UvRect::new(0., 0., 1., 1.)).tint([1., 1., 1., 0.5]))?;
//!         renderer.draw_many(&mut pass, &image, &[
//!             TextureDraw::new(Rect::new(100., 20., 80., 60.)),
//!             TextureDraw::new(Rect::new(190., 20., 80., 60.)),
//!         ])?;
//!     }
//!     frame.finish()?;
//!     Ok(())
//! }
//! ```
//!
//! Draw parameters use dense instance storage. Pages are leased exclusively to a
//! recording until submit/drop and reused through the shared context. Finish queues
//! one upload per used page before that recording's commands. Overlapping recordings
//! therefore retain different parameter data. First use or increased concurrent
//! capacity can allocate pages; ordinary draws reuse them. [`TextureRenderer::draw_many`]
//! instances consecutive compatible draws in input order, splitting large batches
//! by page capacity. Individual draws remain available, with no automatic sorting.
//!
//! Static content can use [`TextureRenderer::prepare_draws`] and
//! [`TextureRenderer::draw_prepared`] for immutable instance data with no recurring
//! parameter uploads. Bindings and material settings remain independently selectable.
//! Pixel draws retain the viewport size used during preparation; normalized draws
//! can adapt to any viewport size without rebuilding.
//!
//! Snapshot bindings retain their view until explicit [`TextureRenderer::rebind`].
//! [`Framebuffer::sampled_color`] instead returns a live [`SampledColor`] handle;
//! [`TextureRenderer::create_sampled_binding`] follows replacement after resize/MSAA,
//! rebuilding only the source bind group when the view changes. Suspended live sources
//! reject drawing. Live sources default to premultiplied alpha, suitable for built-in
//! framebuffer output; custom shaders must declare their actual source alpha through
//! [`TextureRenderer::create_sampled_binding_with_options`]. Snapshot bindings use
//! [`TextureBindingOptions`] to select alpha/filtering. Custom samplers support
//! addressing and mip filtering through [`TextureRenderer::create_binding_with_sampler`].
//!
//! [`TextureMaterial`] holds reusable shader, blend, and depth/stencil settings.
//! Create it through [`GraphicsContext::create_texture_material`]. Texture stencil
//! clipping uses the same pass reference and attachment rules as mesh materials.
//! Custom texture shaders receive instance locations 0–3: origin/X axis, Y axis,
//! source UV rectangle, and tint (four vec4 values). Additional groups follow the
//! image/sampler group at slot zero. Prepare custom pipelines with
//! [`TextureRenderer::try_prepare_material`] to receive validation diagnostics.
//!
//! # Geometry and custom shading
//!
//! [`GraphicsContext::create_mesh`] uploads standard position/color triangle geometry
//! with Uint32 indices. Standard vertices use clip space in the built-in shader and
//! linear, straight-alpha color. [`GraphicsContext::create_mesh_with_options`] accepts
//! custom vertex/instance streams, Uint16/Uint32 indices or nonindexed geometry,
//! primitive topology, and explicit update usages. [`GraphicsContext::create_mesh_from_buffers`]
//! retains application-owned buffer ranges. [`MeshDraw`] selects geometry ranges,
//! instances, and indexed base vertex. Updates keep layout/capacity fixed.
//!
//! [`MaterialOptions::vertex_layouts`] and topology must match the mesh. An empty
//! layout list selects the standard format. Materials own immutable shader, layout,
//! blend, culling, and depth/stencil settings; application bindings are independent.
//! Use [`RenderPass::set_bind_group`] for resource groups and dynamic offsets.
//! [`MeshRenderer::try_prepare_material`] checks a complete [`RenderFormat`] and
//! caches only successfully validated pipelines. Raw shader-module/layout creation
//! still follows wgpu error scopes. Convenience preparation/drawing can lazily create
//! pipelines and uses wgpu's error reporting for shader validation.
//!
//! Queue writes to textures or dynamic geometry execute before the next submission,
//! not at their source-code position between draws. Use encoder copies or custom
//! commands when updates must occur at a precise point in the command sequence.
//!
//! # Scoped repeated drawing
//!
//! Individual draw calls remain useful for simple and mixed rendering. When a
//! sequence keeps one mesh/material or image/material, bind a scoped session to
//! check immutable compatibility and select the pipeline once. Sessions borrow
//! the pass; dynamic texture scopes also borrow renderer scratch storage. Mesh and
//! prepared texture scopes allow renderer reuse through their pass access. Dropping
//! a session leaves the pass open. Changing
//! geometry ranges or texture parameters still gets bounds/data validation.
//!
//! ```no_run
//! use astrelis::{Error, Mesh, MeshRenderer, RenderPass};
//! fn meshes(pass: &mut RenderPass<'_>, renderer: &mut MeshRenderer, mesh: &Mesh)
//!     -> Result<(), Error> {
//!     let mut draws = renderer.bind(pass, mesh)?;
//!     draws.draw();
//!     let [width, height] = draws.pass().size();
//!     draws.pass().set_scissor_rect(0, 0, width / 2, height)?;
//!     draws.draw();
//!     Ok(())
//! }
//! ```
//!
//! [`TextureRenderer::bind`] scopes changing rectangles, while
//! [`TextureRenderer::bind_prepared`] fixes immutable instance data as well:
//!
//! ```no_run
//! use astrelis::{Error, PreparedTextureDraw, RenderPass, TextureBinding, TextureRenderer};
//! fn images(pass: &mut RenderPass<'_>, renderer: &mut TextureRenderer,
//!     image: &TextureBinding, data: &PreparedTextureDraw) -> Result<(), Error> {
//!     let mut draws = renderer.bind_prepared(pass, image, data)?;
//!     draws.draw()?;
//!     Ok(())
//! }
//! ```
//!
//! A session's `pass()` permits clipping, application bindings, other renderers,
//! and raw access. The next scoped draw restores renderer-owned state. Prepared
//! pixel-space data is rechecked after pass access; changing to an incompatible
//! viewport returns an error before drawing. Application-owned groups remain
//! caller-controlled. Texture sessions snapshot a live framebuffer image for the
//! scope's duration; begin a new scope to follow later storage replacement.
//!
//! # Custom passes and renderers
//!
//! [`Frame::begin_render_pass`] accepts [`RenderPassDescriptor`] with depth-only
//! passes, multiple color outputs, imported mip/layer views, timestamp writes, and
//! occlusion queries. Default passes and custom passes return the same [`RenderPass`].
//! Managed attachment snapshots participate in initialization tracking. Imported
//! views use wgpu initialization/validation; declare their view format and selected
//! mip dimensions accurately. Built-in renderers require one color output at slot
//! zero; user renderers can use any compatible configuration through raw access.
//!
//! ```no_run
//! use astrelis::{Error, Frame, RenderPassDescriptor, wgpu};
//! fn custom_surface_pass(frame: &mut Frame<'_, '_>) -> Result<(), Error> {
//!     let mut color = frame.color_attachment()?;
//!     color.ops.load = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
//!     let colors = [Some(color)];
//!     let depth = frame.depth_stencil_attachment()?;
//!     let mut pass = frame.begin_render_pass(&RenderPassDescriptor {
//!         colors: &colors, depth_stencil: depth.as_ref(), ..Default::default()
//!     })?;
//!     pass.as_wgpu().insert_debug_marker("application renderer");
//!     Ok(())
//! }
//! ```
//!
//! Wrapped viewport/scissor/stencil changes affect subsequent draws. Built-in
//! renderers track pipeline, geometry, and image state to avoid redundant setters.
//! [`RenderPass::as_wgpu`] invalidates those caches; the next wrapped draw restores
//! its required state. Custom renderers must establish their own state. Raster
//! settings changed only through raw access do not change the wrapper's choices.
//!
//! [`Frame::encoder`] permits copies, compute, and custom commands between passes.
//! Raw writes do not automatically update managed initialization. Use custom managed
//! attachment passes, or [`Frame::write_framebuffer_color`] for explicit full-output
//! copy/compute writes. The latter callback must initialize every output texel and
//! does not initialize separate MSAA samples. Independently submitted raw commands
//! do not participate in managed tracking. Resources must belong to the same device.
//!
//! All standalone examples own their windows and event handling. The library
//! reexports its exact [`wgpu`] version for application-controlled GPU work.

mod attachments;
mod context;
mod depth_stencil;
mod error;
mod frame;
mod framebuffer;
mod material;
mod mesh;
mod mesh_renderer;
mod pass;
mod target;
mod texture;
mod texture_renderer;
mod uploads;

pub use attachments::{
    RenderColorAttachment, RenderDepthStencilAttachment, RenderFormat, RenderPassDescriptor,
};
pub use context::GraphicsContext;
pub use error::Error;
pub use frame::{Frame, FrameError};
pub use framebuffer::{Framebuffer, FramebufferOptions};
pub use material::{Material, MaterialOptions};
pub use mesh::{
    Mesh, MeshDraw, MeshIndexBuffer, MeshIndices, MeshOptions, MeshVertexBuffer, Vertex,
    VertexLayout, VertexStream,
};
pub use mesh_renderer::{MeshDrawSession, MeshRenderer};
pub use pass::{RenderPass, RenderPassBuilder};
pub use target::{RenderTarget, SurfaceOptions, SurfaceTarget};
pub use texture::{Texture, TextureOptions};
pub use texture_renderer::{
    PreparedTextureDraw, PreparedTextureDrawSession, Rect, SampledColor, TextureAlpha,
    TextureBinding, TextureBindingOptions, TextureBlend, TextureDraw, TextureDrawSession,
    TextureFilter, TextureMaterial, TextureMaterialOptions, TextureRenderer, UvRect,
};

/// The exact wgpu version used by Astrelis, available for GPU interoperability.
pub use wgpu;

#[cfg(test)]
mod api_tests;
