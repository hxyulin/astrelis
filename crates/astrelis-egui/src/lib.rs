//! Egui integration for Astrelis.
//!
//! Provides immediate mode GUI rendering using egui on top of the astrelis-render wrapper.

mod state;

use astrelis_core::profiling::profile_function;
use astrelis_render::{Frame as RenderFrame, RenderWindow};
use astrelis_winit::event::EventBatch;
use state::State;

// Re-export egui types
pub use egui::{
    self, Align, Align2, Color32, Context as EguiContext, CornerRadius, FontFamily, FontId, Frame,
    Id, Key, Label, Layout, Margin, Modifiers, Pos2, Rect, Response, RichText, Sense, Slider,
    Stroke, Style, TextEdit, TextStyle, Ui, Vec2, Visuals, Widget,
};
pub use state::EventResponse;

pub struct Egui {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    state: State,
    full_output: Option<egui::FullOutput>,
}

impl Egui {
    pub fn new(window: &RenderWindow, graphics_ctx: &astrelis_render::GraphicsContext) -> Self {
        let context = egui::Context::default();
        let id = context.viewport_id();

        let visuals = egui::Visuals::dark();
        context.set_visuals(visuals);

        let state = State::new(context.clone(), id, None, None);

        let renderer = egui_wgpu::Renderer::new(
            graphics_ctx.device(),
            window.context().surface_config().format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                ..Default::default()
            },
        );

        Self {
            context,
            renderer,
            state,
            full_output: None,
        }
    }

    /// Begin UI frame and run the GUI closure.
    pub fn ui(&mut self, window: &RenderWindow, gui: impl FnMut(&egui::Context)) {
        profile_function!();
        let raw_input = self.state.take_input(window);
        self.full_output.replace(self.context.run(raw_input, gui));
    }

    /// Render egui to the current frame.
    ///
    /// This method uses frame information directly without needing the window.
    pub fn render(&mut self, frame: &RenderFrame<'_>) {
        profile_function!();

        if self.full_output.is_none() {
            return;
        }

        let full_output = self.full_output.take().unwrap();
        // Note: platform_output handling (cursor changes, clipboard, etc.) is a TODO
        let _ = full_output.platform_output;

        let device = frame.device();
        let queue = frame.queue();

        let tris = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        let (width, height) = frame.size();
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: full_output.pixels_per_point,
        };

        // Create encoder for buffer updates
        let mut encoder = frame.create_encoder(Some("Egui Buffer Update"));
        self.renderer
            .update_buffers(device, queue, &mut encoder, &tris, &screen_descriptor);

        // Create render pass
        {
            let surface_view = frame.surface_view();
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    label: Some("Egui Render Pass"),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();

            self.renderer.render(&mut rpass, &tris, &screen_descriptor);
        }

        // Add command buffer to frame
        frame.add_command_buffer(encoder.finish());

        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }
    }

    /// Process events from the event batch.
    pub fn handle_events(&mut self, window: &RenderWindow, events: &mut EventBatch) -> bool {
        profile_function!();
        let mut any_consumed = false;

        events.dispatch(|event| {
            let response = self.state.on_event(window, event);
            if response.consumed {
                any_consumed = true;
            }
            let mut status = astrelis_winit::event::HandleStatus::empty();
            if response.repaint || response.consumed {
                status |= astrelis_winit::event::HandleStatus::HANDLED;
            }
            if response.consumed {
                status |= astrelis_winit::event::HandleStatus::CONSUMED;
            }
            status
        });

        any_consumed
    }

    /// Get the egui context for direct access.
    pub fn context(&self) -> &egui::Context {
        &self.context
    }

    /// Register a wgpu texture with egui for rendering.
    /// Returns a texture ID that can be used in egui image widgets.
    pub fn register_wgpu_texture(
        &mut self,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        filter: wgpu::FilterMode,
    ) -> egui::TextureId {
        self.renderer
            .register_native_texture(device, texture, filter)
    }

    /// Unregister a texture from egui.
    pub fn unregister_texture(&mut self, id: egui::TextureId) {
        self.renderer.free_texture(&id);
    }
}
