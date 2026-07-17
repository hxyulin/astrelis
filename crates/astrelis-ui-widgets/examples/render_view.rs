//! Animated application-owned GPU texture composited by a retained `RenderView`.

use std::{io, sync::Arc, time::Duration};

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig, TimerId};
use astrelis_core::geometry::Size;
use astrelis_gpu::{
    Color, CompositeAlphaMode, DeviceDescriptor, LoadOp, PresentMode, RenderPassColorAttachment,
    RenderPassDescriptor, RequestAdapterOptions, StoreOp, SurfaceConfiguration, SurfaceFrameStatus,
    SurfaceTarget, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};
use astrelis_paint::ExternalImage;
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{
    ElementState, Key, NamedKey, Window, WindowAttributes, WindowEvent, WindowId,
};
use astrelis_text::{FontDatabase, FontFamily};
use astrelis_ui_core::{ElementHandle, Label, LayoutStyle, Length, Theme, Ui, UiEventKind};
use astrelis_ui_widgets::{
    RenderView, RenderViewContent, RenderViewEvent, RenderViewResizePolicy, render_view_snapshot,
};

const NOTO_SANS: &[u8] = include_bytes!("../../astrelis-ui-core/assets/NotoSans.ttf");

struct SceneTexture {
    _texture: astrelis_gpu::Texture,
    view: astrelis_gpu::TextureView,
    image: ExternalImage,
    allocation: Size<astrelis_core::geometry::Physical, u32>,
}

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    renderer: Renderer,
    scene: Option<SceneTexture>,
}

#[derive(Clone)]
enum Message {
    View(RenderViewEvent),
}

struct Example {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui<Message>,
    view: ElementHandle<RenderView<Message>>,
    toggle: ElementHandle<astrelis_ui_core::Button>,
    status: ElementHandle<Label>,
    timer: Option<TimerId>,
    animated: bool,
    phase: f64,
    pointer_x: f64,
}

impl Example {
    fn new() -> io::Result<Self> {
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(NOTO_SANS))
            .map_err(io::Error::other)?;
        let mut ui = Ui::new(
            fonts,
            Theme {
                font_families: vec![FontFamily::Named("Noto Sans".into())],
                ..Default::default()
            },
        );
        let root = ui.root();
        let padding = ui
            .add_padding(root, astrelis_ui_core::Insets::all(28.0))
            .map_err(io::Error::other)?;
        ui.set_layout(
            padding,
            LayoutStyle {
                grow: 1.0,
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        let column = ui.add_column(padding).map_err(io::Error::other)?;
        ui.add_label(column, "Milestone 12 — texture-backed RenderView")
            .map_err(io::Error::other)?;
        ui.add_label(
            column,
            "Move the pointer over the scene. Click it, then press Space or the arrow keys.",
        )
        .map_err(io::Error::other)?;
        let view = ui
            .add_widget(column, RenderView::new("Animated GPU scene", Message::View))
            .map_err(io::Error::other)?;
        ui.set_layout(
            view,
            LayoutStyle {
                width: Length::Percent(1.0),
                height: Length::Px(380.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        let controls = ui.add_row(column).map_err(io::Error::other)?;
        let toggle = ui
            .add_button(controls, "Pause animation")
            .map_err(io::Error::other)?;
        let status = ui
            .add_label(controls, "Scene is animated; click the view to focus it.")
            .map_err(io::Error::other)?;
        Ok(Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
            ui,
            view,
            toggle,
            status,
            timer: None,
            animated: true,
            phase: 0.0,
            pointer_x: 0.5,
        })
    }

    fn start_timer(&mut self, context: &mut AppContext<'_, '_, Self>) {
        if self.timer.is_some() || !self.animated {
            return;
        }
        self.timer = Some(
            context.set_interval(Duration::from_millis(16), |app, context| {
                let visible = render_view_snapshot(&mut app.ui, app.view)
                    .is_ok_and(|snapshot| snapshot.should_render);
                if !app.animated || !visible {
                    if let Some(timer) = app.timer.take() {
                        context.cancel_timer(timer);
                    }
                    return Ok(());
                }
                app.phase += 0.035;
                if let Some(window) = &app.window {
                    context.invalidate_window(window.id());
                }
                Ok(())
            }),
        );
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
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        self.ui.set_viewport(
            Size::new(width as f32 / scale, height as f32 / scale),
            scale,
        );
        Ok(())
    }

    fn ensure_scene(&mut self) -> io::Result<Option<Size<astrelis_core::geometry::Physical, u32>>> {
        let snapshot = render_view_snapshot(&mut self.ui, self.view).map_err(io::Error::other)?;
        if !snapshot.should_render {
            return Ok(None);
        }
        let Some(gpu) = &mut self.gpu else {
            return Ok(None);
        };
        let policy = RenderViewResizePolicy::default();
        let current = gpu.scene.as_ref().map(|scene| scene.allocation);
        let allocation = policy
            .allocation(current, snapshot.desired_physical_size, true)
            .expect("visible non-empty view allocates");
        if current != Some(allocation) {
            if let Some(old) = gpu.scene.take() {
                gpu.renderer.unregister_external_image(&old.image);
            }
            let texture = gpu.device.create_texture(TextureDescriptor {
                label: Some("render-view example scene".into()),
                size: astrelis_gpu::Extent3d::d2(allocation.width, allocation.height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            });
            let view = texture.create_view(TextureViewDescriptor::default());
            let image = ExternalImage::new(allocation).map_err(io::Error::other)?;
            gpu.renderer
                .register_external_image(&image, view.clone())
                .map_err(io::Error::other)?;
            gpu.scene = Some(SceneTexture {
                _texture: texture,
                view,
                image,
                allocation,
            });
        }
        let image = gpu.scene.as_ref().expect("scene allocated").image.clone();
        self.ui
            .update_widget(self.view, |view| {
                view.set_content(RenderViewContent::Ready {
                    image,
                    source_extent: snapshot.desired_physical_size,
                })
            })
            .map_err(io::Error::other)?;
        Ok(Some(snapshot.desired_physical_size))
    }

    fn consume_input(&mut self) -> io::Result<()> {
        for message in self.ui.drain_messages().collect::<Vec<_>>() {
            match message {
                Message::View(RenderViewEvent::PointerMoved { position, .. }) => {
                    self.pointer_x = position.normalized.x as f64
                }
                Message::View(RenderViewEvent::PointerButton {
                    position,
                    state: ElementState::Pressed,
                    ..
                }) => self.pointer_x = position.normalized.x as f64,
                Message::View(RenderViewEvent::Keyboard(input))
                    if input.state == ElementState::Pressed =>
                {
                    match input.logical_key {
                        Key::Named(NamedKey::Space) => self.phase += std::f64::consts::PI,
                        Key::Named(NamedKey::Other(name)) if name == "ArrowLeft" => {
                            self.phase -= 0.3
                        }
                        Key::Named(NamedKey::Other(name)) if name == "ArrowRight" => {
                            self.phase += 0.3
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn render(&mut self) -> io::Result<()> {
        let desired = self.ensure_scene()?;
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
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        if desired.is_some()
            && let Some(scene) = &gpu.scene
        {
            let wave = (self.phase.sin() * 0.5 + 0.5) * 0.65;
            encoder
                .render_pass(RenderPassDescriptor {
                    label: Some("render-view animated scene".into()),
                    color_attachments: vec![Some(RenderPassColorAttachment {
                        view: scene.view.clone(),
                        resolve_target: None,
                        load: LoadOp::Clear(Color {
                            r: 0.04 + wave,
                            g: 0.08 + self.pointer_x.clamp(0.0, 1.0) * 0.55,
                            b: 0.28 + (1.0 - wave) * 0.55,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                })
                .map_err(io::Error::other)?;
        }
        let surface_view = frame
            .texture()
            .create_view(TextureViewDescriptor::default());
        gpu.renderer
            .render(
                &mut encoder,
                &list,
                RenderTarget {
                    view: surface_view,
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
}

impl App for Example {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis RenderView".into(),
                inner_size: Some(Size::new(900.0, 620.0)),
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
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration: configuration.clone(),
            renderer,
            scene: None,
        });
        self.configure(configuration.width, configuration.height)?;
        self.start_timer(context);
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::CloseRequested) {
            if let Some(timer) = self.timer.take() {
                context.cancel_timer(timer);
            }
            self.gpu = None;
            self.window = None;
            context.unregister_window(id);
            context.exit();
            return Ok(());
        }
        match event {
            WindowEvent::Resized(size) => self.configure(size.width, size.height)?,
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height)?
            }
            _ => {}
        }
        if let Some(window) = &self.window {
            let update = self
                .ui
                .handle_window_event(window, &context.clipboard(), &event)
                .map_err(io::Error::other)?;
            let toggle = self.ui.drain_events().any(|event| {
                event.is_from(self.toggle) && event.kind == UiEventKind::ButtonActivated
            });
            self.consume_input()?;
            if toggle {
                self.animated = !self.animated;
                self.ui
                    .set_button_text(
                        self.toggle,
                        if self.animated {
                            "Pause animation"
                        } else {
                            "Resume animation"
                        },
                    )
                    .map_err(io::Error::other)?;
                self.ui
                    .set_label_text(
                        self.status,
                        if self.animated {
                            "Scene animation resumed."
                        } else {
                            "Scene paused; the runtime returns to Wait."
                        },
                    )
                    .map_err(io::Error::other)?;
                if self.animated {
                    self.start_timer(context);
                } else if let Some(timer) = self.timer.take() {
                    context.cancel_timer(timer);
                }
            }
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
        Example::new().map_err(astrelis_app::RuntimeError::Application)?,
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}
