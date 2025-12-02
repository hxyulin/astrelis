use std::ops::Deref;

use astrelis_core::profiling::profile_function;
use astrelis_winit::{
    event::PhysicalSize,
    window::{Window, WindowBackend},
};

/// A globally shared graphics context.
pub struct GraphicsContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GraphicsContext {
    /// Creates a new graphics context synchronously.
    ///
    /// See [`GraphicsContext::new`] for the asynchronous version.
    pub fn new_sync() -> &'static Self {
        pollster::block_on(Self::new())
    }

    /// Creates a new graphics context asynchronously.
    ///
    /// This returns a static reference to simplify the public API and lifecycle
    pub async fn new() -> &'static Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
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
            .expect("Failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        Box::leak(Box::new(Self {
            instance,
            adapter,
            device,
            queue,
        }))
    }
}

struct PendingReconfigure {
    pub resize: Option<PhysicalSize<u32>>,
}

impl PendingReconfigure {
    const fn new() -> Self {
        Self { resize: None }
    }
}

/// Descriptor for configuring a window's rendering context.
pub struct WindowContextDescriptor {
    /// The surface texture format. If None, uses the default format for the surface.
    pub format: Option<wgpu::TextureFormat>,
}

impl Default for WindowContextDescriptor {
    fn default() -> Self {
        Self { format: None }
    }
}

pub struct WindowContext {
    pub(crate) context: &'static GraphicsContext,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) reconfigure: PendingReconfigure,
}

impl WindowContext {
    pub fn new(
        context: &'static GraphicsContext,
        window: &Window,
        descriptor: WindowContextDescriptor,
    ) -> Self {
        let window = window.window.clone();
        let PhysicalSize { width, height } = window.inner_size();
        let surface = context
            .instance
            .create_surface(window)
            .expect("Failed to create surface");

        let mut config = surface
            .get_default_config(&context.adapter, width, height)
            .expect("Failed to get default surface configuration");

        if let Some(format) = descriptor.format {
            config.format = format;
        }

        surface.configure(&context.device, &config);

        Self {
            surface,
            config,
            reconfigure: PendingReconfigure::new(),
            context,
        }
    }

    pub fn context(&self) -> &GraphicsContext {
        self.context
    }

    pub fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }
}

pub struct RenderableWindow {
    pub(crate) window: Window,
    pub(crate) context: WindowContext,
}

impl RenderableWindow {
    pub fn new(window: Window, context: &'static GraphicsContext) -> Self {
        Self::new_with_descriptor(window, context, WindowContextDescriptor::default())
    }

    pub fn new_with_descriptor(
        window: Window,
        context: &'static GraphicsContext,
        descriptor: WindowContextDescriptor,
    ) -> Self {
        let context = WindowContext::new(context, &window, descriptor);
        Self { window, context }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn context(&self) -> &WindowContext {
        &self.context
    }
}

impl Deref for RenderableWindow {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}

pub struct FrameStats {
    pub passes: usize,
    pub draw_calls: usize,
}

impl FrameStats {
    fn new() -> Self {
        Self {
            passes: 0,
            draw_calls: 0,
        }
    }
}

pub struct FrameContext {
    pub(crate) stats: FrameStats,
    pub(crate) surface: Option<Surface>,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) context: &'static GraphicsContext,
}

impl WindowBackend for RenderableWindow {
    type FrameContext = FrameContext;

    fn begin_drawing(&mut self) -> Self::FrameContext {
        profile_function!();

        let mut configure_needed = false;
        if let Some(new_size) = self.context.reconfigure.resize {
            self.context.config.width = new_size.width;
            self.context.config.height = new_size.height;
            configure_needed = true;
        }

        if configure_needed {
            self.context
                .surface
                .configure(&self.context.context.device, &self.context.config);
        }

        let frame = self.context.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder =
            self.context
                .context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Frame Encoder"),
                });

        FrameContext {
            surface: Some(Surface {
                texture: frame,
                view,
            }),
            encoder: Some(encoder),
            context: self.context.context,
            stats: FrameStats::new(),
        }
    }
}

impl FrameContext {
    pub fn surface(&self) -> &Surface {
        self.surface.as_ref().unwrap()
    }

    pub fn increment_passes(&mut self) {
        self.stats.passes += 1;
    }

    pub fn increment_draw_calls(&mut self) {
        self.stats.draw_calls += 1;
    }

    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder.as_mut().unwrap()
    }

    pub fn encoder_and_surface(&mut self) -> (&mut wgpu::CommandEncoder, &Surface) {
        (
            self.encoder.as_mut().unwrap(),
            self.surface.as_ref().unwrap(),
        )
    }

    pub fn finish(self) {
        drop(self);
    }
}

impl Drop for FrameContext {
    fn drop(&mut self) {
        profile_function!();

        if self.stats.passes == 0 {
            tracing::error!("No render passes were executed for this frame!");
            return;
        }

        if let Some(encoder) = self.encoder.take() {
            self.context.queue.submit(std::iter::once(encoder.finish()));
        }

        if let Some(surface) = self.surface.take() {
            surface.texture.present();
        }
    }
}

pub struct Surface {
    pub(crate) texture: wgpu::SurfaceTexture,
    pub(crate) view: wgpu::TextureView,
}

impl Surface {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub struct RenderPassBuilder<'a> {
    label: Option<&'a str>,
    color_attachments: Vec<Option<wgpu::RenderPassColorAttachment<'a>>>,
    surface_attachment_ops: Option<(wgpu::Operations<wgpu::Color>, Option<&'a wgpu::TextureView>)>,
    depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
}

impl<'a> RenderPassBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            color_attachments: Vec::new(),
            surface_attachment_ops: None,
            depth_stencil_attachment: None,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn color_attachment(
        mut self,
        view: Option<&'a wgpu::TextureView>,
        resolve_target: Option<&'a wgpu::TextureView>,
        ops: wgpu::Operations<wgpu::Color>,
    ) -> Self {
        if let Some(view) = view {
            self.color_attachments
                .push(Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops,
                    depth_slice: None,
                }));
        } else {
            // Store ops for later - will be filled with surface view in build()
            self.surface_attachment_ops = Some((ops, resolve_target));
        }
        self
    }

    pub fn depth_stencil_attachment(
        mut self,
        view: &'a wgpu::TextureView,
        depth_ops: Option<wgpu::Operations<f32>>,
        stencil_ops: Option<wgpu::Operations<u32>>,
    ) -> Self {
        self.depth_stencil_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops,
            stencil_ops,
        });
        self
    }

    /// Builds the render pass and begins it on the provided frame context.
    ///
    /// This takes ownership of the CommandEncoder from the FrameContext, and releases it
    /// back to the FrameContext when the RenderPass is dropped or [`finish`](RenderPass::finish)
    /// is called.
    pub fn build(self, frame_context: &'a mut FrameContext) -> RenderPass<'a> {
        let mut encoder = frame_context.encoder.take().unwrap();

        // Fill in surface attachment if requested
        let surface_attachment = self.surface_attachment_ops.map(|(ops, resolve_target)| {
            let surface_view = frame_context.surface().view();
            wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target,
                ops,
                depth_slice: None,
            }
        });

        // Build combined attachments - surface first if present, then others
        let mut all_attachments = Vec::new();
        if let Some(attachment) = surface_attachment {
            all_attachments.push(Some(attachment));
        }
        all_attachments.extend(self.color_attachments);

        let descriptor = wgpu::RenderPassDescriptor {
            label: self.label,
            color_attachments: &all_attachments,
            depth_stencil_attachment: self.depth_stencil_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        };

        let render_pass = encoder.begin_render_pass(&descriptor).forget_lifetime();

        frame_context.increment_passes();

        RenderPass {
            context: frame_context,
            encoder: Some(encoder),
            descriptor: Some(render_pass),
        }
    }
}

pub struct RenderPass<'a> {
    pub context: &'a mut FrameContext,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) descriptor: Option<wgpu::RenderPass<'static>>,
}

impl<'a> RenderPass<'a> {
    pub fn descriptor(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.descriptor.as_mut().unwrap()
    }

    pub fn finish(self) {
        drop(self);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        profile_function!();

        drop(self.descriptor.take());

        // Return the encoder to the frame context
        self.context.encoder = self.encoder.take();
    }
}

impl<'a> Default for RenderPassBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
