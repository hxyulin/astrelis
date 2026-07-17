//! Keyboard-accessible settings window using the retained UI vertical slice.

use std::io;

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::geometry::Size;
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};
use astrelis_text::FontDatabase;
use astrelis_ui_core::{
    Column, ElementHandle, Insets, Label, LayoutStyle, TextField, Theme, Ui, UiEventKind,
};

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    renderer: Renderer,
}

struct Handles {
    name: ElementHandle<TextField>,
    secret: ElementHandle<TextField>,
    status: ElementHandle<Label>,
}

struct Settings {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui,
    handles: Handles,
}

impl Settings {
    fn new() -> io::Result<Self> {
        let mut ui = Ui::new(FontDatabase::default(), Theme::default());
        let root = ui.root();
        let padding = ui
            .add_padding(root, Insets::all(28.0))
            .map_err(io::Error::other)?;
        let column: ElementHandle<Column> = ui.add_column(padding).map_err(io::Error::other)?;
        ui.add_label(column, "Astrelis settings")
            .map_err(io::Error::other)?;
        ui.add_label(column, "Display name")
            .map_err(io::Error::other)?;
        let name = ui
            .add_text_field(column, "Explorer")
            .map_err(io::Error::other)?;
        ui.set_placeholder(name, "Your display name")
            .map_err(io::Error::other)?;
        ui.set_layout(
            name,
            LayoutStyle {
                min_width: Some(340.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        ui.add_label(column, "Access token")
            .map_err(io::Error::other)?;
        let secret = ui.add_text_field(column, "").map_err(io::Error::other)?;
        ui.set_placeholder(secret, "Paste a token")
            .map_err(io::Error::other)?;
        ui.set_password(secret, true).map_err(io::Error::other)?;
        ui.set_layout(
            secret,
            LayoutStyle {
                min_width: Some(340.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        ui.add_button(column, "Save settings")
            .map_err(io::Error::other)?;
        let status = ui
            .add_label(
                column,
                "Tab through controls; clipboard and IME are enabled.",
            )
            .map_err(io::Error::other)?;
        Ok(Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
            ui,
            handles: Handles {
                name,
                secret,
                status,
            },
        })
    }

    fn configure(&mut self, width: u32, height: u32) -> io::Result<()> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        if width == 0 || height == 0 {
            return Ok(());
        }
        gpu.configuration.width = width;
        gpu.configuration.height = height;
        gpu.surface
            .configure(&gpu.device, gpu.configuration.clone())
            .map_err(io::Error::other)?;
        if let Some(window) = &self.window {
            let scale = window.scale_factor() as f32;
            self.ui.set_viewport(
                Size::new(width as f32 / scale, height as f32 / scale),
                scale,
            );
        }
        Ok(())
    }

    fn render(&mut self) -> io::Result<()> {
        let list = self.ui.display_list().map_err(io::Error::other)?;
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let frame = match gpu.surface.acquire().map_err(io::Error::other)? {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .map_err(io::Error::other)?;
                return Ok(());
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return Ok(()),
            _ => return Ok(()),
        };
        let view = frame
            .texture()
            .create_view(TextureViewDescriptor::default());
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        gpu.renderer
            .render(
                &mut encoder,
                &list,
                RenderTarget {
                    view,
                    format: gpu.configuration.format,
                    size: Size::new(gpu.configuration.width, gpu.configuration.height),
                    scale_factor: self
                        .window
                        .as_ref()
                        .map_or(1.0, |window| window.scale_factor() as f32),
                    clear_color: astrelis_core::color::Color::BLACK,
                },
            )
            .map_err(io::Error::other)?;
        gpu.queue
            .submit([encoder.finish().map_err(io::Error::other)?])
            .map_err(io::Error::other)?;
        frame.present().map_err(io::Error::other)
    }

    fn consume_events(&mut self) -> io::Result<()> {
        let events = self.ui.drain_events().collect::<Vec<_>>();
        for event in events {
            match event.kind {
                UiEventKind::ButtonActivated => {
                    let name = self.ui.text(self.handles.name).map_err(io::Error::other)?;
                    let token_set = !self
                        .ui
                        .text(self.handles.secret)
                        .map_err(io::Error::other)?
                        .is_empty();
                    self.ui
                        .set_label_text(
                            self.handles.status,
                            format!("Saved settings for {name}; token set: {token_set}."),
                        )
                        .map_err(io::Error::other)?;
                }
                UiEventKind::TextSubmitted(_) => {
                    self.ui
                        .set_label_text(
                            self.handles.status,
                            "Press Save settings to apply changes.",
                        )
                        .map_err(io::Error::other)?;
                }
                UiEventKind::TextChanged(_) | UiEventKind::FocusChanged(_) => {}
            }
        }
        Ok(())
    }
}

impl App for Settings {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis settings".into(),
                inner_size: Some(Size::new(760.0, 520.0)),
                ..Default::default()
            })
            .map_err(io::Error::other)?;
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .map_err(io::Error::other)?;
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        }))
        .map_err(io::Error::other)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .map_err(io::Error::other)?;
        let capabilities = surface.capabilities(&adapter).map_err(io::Error::other)?;
        let format = capabilities.formats[0];
        let size = window.inner_size().map_err(io::Error::other)?;
        let configuration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: PresentMode::Fifo,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(CompositeAlphaMode::Opaque),
            desired_maximum_frame_latency: 2,
        };
        surface
            .configure(&device, configuration.clone())
            .map_err(io::Error::other)?;
        let renderer = Renderer::new(device.clone(), queue.clone(), RendererOptions::default())
            .map_err(io::Error::other)?;
        let scale = window.scale_factor() as f32;
        self.ui.set_viewport(
            Size::new(
                configuration.width as f32 / scale,
                configuration.height as f32 / scale,
            ),
            scale,
        );
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration,
            renderer,
        });
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::CloseRequested) {
            self.gpu = None;
            context.unregister_window(id);
            self.window = None;
            context.exit();
            return Ok(());
        }
        match &event {
            WindowEvent::Resized(size) => self.configure(size.width, size.height)?,
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height)?;
            }
            _ => {}
        }
        if let Some(window) = &self.window {
            let update = self
                .ui
                .handle_window_event(window, &context.clipboard(), &event)
                .map_err(io::Error::other)?;
            self.consume_events()?;
            if update.redraw || self.ui.needs_redraw() {
                context.invalidate_window(id);
            }
        }
        Ok(())
    }

    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
    ) -> Result<(), Self::Error> {
        self.render()
    }
}

fn main() -> Result<(), astrelis_app::RuntimeError<io::Error>> {
    Runtime::finish(astrelis_platform_winit::run_return(Runtime::new(
        Settings::new().map_err(astrelis_app::RuntimeError::Application)?,
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}
