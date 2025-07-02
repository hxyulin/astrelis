use egui_wgpu::ScreenDescriptor;
use puffin::profile_function;

use crate::{
    RenderContext, Window,
    event::Event,
    graphics::{
        Framebuffer,
        egui::state::{EventResponse, State},
    },
};

mod state;

pub struct EguiContext {
    pub(crate) context: egui::Context,
    pub(crate) renderer: egui_wgpu::Renderer,
    pub(crate) state: State,

    full_output: Option<egui::FullOutput>,
}

impl EguiContext {
    pub fn new(window: &Window) -> Self {
        let context = egui::Context::default();
        let id = context.viewport_id();

        let visuals = egui::Visuals::dark();
        context.set_visuals(visuals);

        let state = State::new(context.clone(), id, None, None);

        let renderer = egui_wgpu::Renderer::new(
            &window.context.device,
            window.context.config.format,
            None,
            window.context.sample_count,
            false,
        );

        Self {
            context,
            renderer,
            state,
            full_output: None,
        }
    }

    pub fn ui<W: AsRef<Window>>(&mut self, window: W, gui: impl FnMut(&egui::Context)) {
        let raw_input = self.state.take_input(window.as_ref());
        self.full_output.replace(self.context.run(raw_input, gui));
    }

    pub fn render(&mut self, ctx: &mut RenderContext) {
        profile_function!();
        if self.full_output.is_none() {
            return;
        }
        let window = &mut ctx.window;
        let device = &window.context.device;
        let queue = &window.context.queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Egui Encoder"),
        });

        let full_output = self.full_output.take().unwrap();
        self.state
            .handle_platform_output(window, full_output.platform_output);
        let frame = window.context.frame.as_mut().unwrap();
        frame.passes += 1;

        let tris = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [window.context.config.width, window.context.config.height],
            pixels_per_point: window.window.scale_factor() as f32,
        };
        self.renderer
            .update_buffers(device, queue, &mut encoder, &tris, &screen_descriptor);
        let mut rpass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                label: Some("Egui Render Pass"),
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        self.renderer.render(&mut rpass, &tris, &screen_descriptor);
        drop(rpass);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }
        queue.submit(Some(encoder.finish()));
    }

    pub fn on_event(&mut self, window: &Window, event: &Event) -> EventResponse {
        self.state.on_event(window, event)
    }

    pub fn update_texture<T: AsRef<Window>>(
        &mut self,
        window: T,
        fb: &Framebuffer,
    ) -> egui::TextureId {
        self.renderer.register_native_texture(
            &window.as_ref().context.device,
            &fb.color.view,
            wgpu::FilterMode::Linear,
        )
    }
}
