//! Native end-to-end docking workspace exercise.

use std::{io, sync::Arc};

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::geometry::Size;
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};
use astrelis_text::{FontDatabase, FontFamily};
use astrelis_ui_core::{
    ElementHandle, EventFilter, Label, LayoutStyle, Length, RoutedEventKind, Theme, Ui,
};
use astrelis_ui_docking::{
    DockAction, DockAxis, DockLayout, DockNode, DockPlacement, DockSide, DockStyle, DockTabs,
    DockWorkspace, FloatingRect, PanelDescriptor, PanelId, PreferredPlacement,
};

const NOTO_SANS: &[u8] = include_bytes!("../../astrelis-ui-core/assets/NotoSans.ttf");

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    renderer: Renderer,
}

#[derive(Clone, Debug)]
enum Message {
    Dock(DockAction),
    Save,
    Load,
    Reset,
    LoadStale,
    Open(PanelId),
    Float(PanelId),
}

struct DockingExample {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui<Message>,
    workspace: DockWorkspace<Message>,
    default_layout: DockLayout,
    saved_json: Option<String>,
    status: ElementHandle<Label>,
}

impl DockingExample {
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
        ui.set_layout(
            root,
            LayoutStyle {
                width: Length::Percent(1.0),
                height: Length::Percent(1.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;

        let toolbar = ui.add_row(root).map_err(io::Error::other)?;
        ui.set_layout(
            toolbar,
            LayoutStyle {
                height: Length::Px(42.0),
                min_height: Length::Px(42.0),
                shrink: 0.0,
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        add_message_button(&mut ui, toolbar, "Save", Message::Save)?;
        add_message_button(&mut ui, toolbar, "Load", Message::Load)?;
        add_message_button(&mut ui, toolbar, "Reset", Message::Reset)?;
        add_message_button(&mut ui, toolbar, "Load stale", Message::LoadStale)?;

        let scene_id = panel("scene");
        let inspector_id = panel("inspector");
        let assets_id = panel("assets");
        let console_id = panel("console");
        add_message_button(
            &mut ui,
            toolbar,
            "Open Inspector",
            Message::Open(inspector_id.clone()),
        )?;
        add_message_button(
            &mut ui,
            toolbar,
            "Open Console",
            Message::Open(console_id.clone()),
        )?;
        add_message_button(
            &mut ui,
            toolbar,
            "Float Assets",
            Message::Float(assets_id.clone()),
        )?;

        let status = ui
            .add_label(
                root,
                "Ready — drag tabs, resize splits, or use the toolbar.",
            )
            .map_err(io::Error::other)?;
        let dock_host = ui.add_column(root).map_err(io::Error::other)?;
        ui.set_layout(
            dock_host,
            LayoutStyle {
                grow: 1.0,
                min_height: Length::Px(240.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;

        let scene = build_scene_panel(&mut ui, root)?;
        let inspector = build_inspector_panel(&mut ui, root)?;
        let assets = build_assets_panel(&mut ui, root)?;
        let console = build_console_panel(&mut ui, root)?;

        let mut workspace =
            DockWorkspace::new(&mut ui, dock_host, DockStyle::default(), Message::Dock)
                .map_err(io::Error::other)?;

        let scene_descriptor = PanelDescriptor::new(scene_id.clone(), "Scene")
            .closable(false)
            .minimum_size(Size::new(280.0, 220.0));
        workspace
            .register_panel(&mut ui, scene_descriptor, scene)
            .map_err(io::Error::other)?;

        let inspector_descriptor = PanelDescriptor::new(inspector_id.clone(), "Inspector")
            .minimum_size(Size::new(210.0, 180.0))
            .preferred(PreferredPlacement::Split {
                anchor: scene_id.clone(),
                side: DockSide::Right,
            });
        workspace
            .register_panel(&mut ui, inspector_descriptor, inspector)
            .map_err(io::Error::other)?;

        let assets_descriptor = PanelDescriptor::new(assets_id.clone(), "Assets")
            .preferred(PreferredPlacement::Tab(inspector_id.clone()));
        workspace
            .register_panel(&mut ui, assets_descriptor, assets)
            .map_err(io::Error::other)?;

        let console_descriptor = PanelDescriptor::new(console_id.clone(), "Console")
            .minimum_size(Size::new(260.0, 120.0))
            .preferred(PreferredPlacement::Split {
                anchor: scene_id.clone(),
                side: DockSide::Bottom,
            });
        workspace
            .register_panel(&mut ui, console_descriptor, console)
            .map_err(io::Error::other)?;

        let default_layout = default_layout(&scene_id, &inspector_id, &assets_id, &console_id);
        workspace
            .restore(&mut ui, default_layout.clone(), default_layout.clone())
            .map_err(io::Error::other)?;

        Ok(Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
            ui,
            workspace,
            default_layout,
            saved_json: None,
            status,
        })
    }

    fn set_status(&mut self, text: impl Into<String>) -> io::Result<()> {
        self.ui
            .set_label_text(self.status, text)
            .map_err(io::Error::other)
    }

    fn consume_messages(&mut self) -> io::Result<()> {
        for message in self.ui.drain_messages().collect::<Vec<_>>() {
            match message {
                Message::Dock(action) => {
                    let outcome = self
                        .workspace
                        .apply(&mut self.ui, action)
                        .map_err(io::Error::other)?;
                    if outcome.layout_changed {
                        self.set_status("Dock layout changed; use Save to capture it.")?;
                    }
                }
                Message::Save => {
                    let json = serde_json::to_string_pretty(self.workspace.layout())
                        .map_err(io::Error::other)?;
                    let bytes = json.len();
                    self.saved_json = Some(json);
                    self.set_status(format!(
                        "Saved the current layout in memory ({bytes} bytes)."
                    ))?;
                }
                Message::Load => {
                    if let Some(json) = &self.saved_json {
                        let layout = serde_json::from_str(json).map_err(io::Error::other)?;
                        let report = self
                            .workspace
                            .restore(&mut self.ui, layout, self.default_layout.clone())
                            .map_err(io::Error::other)?;
                        self.set_status(format!("Loaded saved layout; recovery: {report:?}"))?;
                    } else {
                        self.set_status("Nothing saved yet.")?;
                    }
                }
                Message::Reset => {
                    self.workspace
                        .restore(
                            &mut self.ui,
                            self.default_layout.clone(),
                            self.default_layout.clone(),
                        )
                        .map_err(io::Error::other)?;
                    self.set_status("Restored the default layout.")?;
                }
                Message::LoadStale => {
                    let stale = serde_json::from_str::<DockLayout>(STALE_LAYOUT)
                        .map_err(io::Error::other)?;
                    let report = self
                        .workspace
                        .restore(&mut self.ui, stale, self.default_layout.clone())
                        .map_err(io::Error::other)?;
                    self.set_status(format!("Recovered intentionally stale layout: {report:?}"))?;
                }
                Message::Open(panel) => {
                    self.workspace
                        .open(&mut self.ui, &panel)
                        .map_err(io::Error::other)?;
                    self.set_status(format!("Opened {panel} at its preferred placement."))?;
                }
                Message::Float(panel) => {
                    self.workspace
                        .apply(
                            &mut self.ui,
                            DockAction::Place {
                                panel: panel.clone(),
                                placement: DockPlacement::Floating(FloatingRect::new(
                                    80.0, 72.0, 380.0, 280.0,
                                )),
                            },
                        )
                        .map_err(io::Error::other)?;
                    self.set_status(format!("Floated {panel}."))?;
                }
            }
        }
        Ok(())
    }

    fn install_gpu(&mut self, gpu: GpuState) -> io::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| io::Error::other("window closed during GPU initialization"))?;
        let scale = window.scale_factor() as f32;
        self.ui.set_viewport(
            Size::new(
                gpu.configuration.width as f32 / scale,
                gpu.configuration.height as f32 / scale,
            ),
            scale,
        );
        self.gpu = Some(gpu);
        Ok(())
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
            let viewport = Size::new(width as f32 / scale, height as f32 / scale);
            self.ui.set_viewport(viewport, scale);
            self.workspace
                .clamp_floating(&mut self.ui, viewport)
                .map_err(io::Error::other)?;
        }
        Ok(())
    }

    fn render(&mut self) -> io::Result<()> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let list = self.ui.display_list().map_err(io::Error::other)?;
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
}

fn panel(value: &str) -> PanelId {
    PanelId::new(value).expect("example panel identity is valid")
}

fn add_message_button<T>(
    ui: &mut Ui<Message>,
    parent: ElementHandle<T>,
    label: &str,
    message: Message,
) -> io::Result<()> {
    let button = ui.add_button(parent, label).map_err(io::Error::other)?;
    ui.listen(
        button,
        None,
        EventFilter::Activate,
        move |context, event| {
            if matches!(event.kind, RoutedEventKind::Activate) {
                context.emit(message.clone());
            }
        },
    )
    .map_err(io::Error::other)?;
    Ok(())
}

fn build_scene_panel(
    ui: &mut Ui<Message>,
    root: ElementHandle<astrelis_ui_core::Column>,
) -> io::Result<ElementHandle<astrelis_ui_core::Column>> {
    let panel = ui.add_column(root).map_err(io::Error::other)?;
    ui.add_label(
        panel,
        "Scene panel — this panel is required and cannot close.",
    )
    .map_err(io::Error::other)?;
    ui.add_text_field(panel, "Untitled level")
        .map_err(io::Error::other)?;
    ui.add_label(panel, "Grid enabled")
        .map_err(io::Error::other)?;
    ui.add_checkbox(panel, true).map_err(io::Error::other)?;
    ui.add_label(panel, "Zoom").map_err(io::Error::other)?;
    ui.add_slider(panel, 0.25, 2.0, 0.05, 1.0)
        .map_err(io::Error::other)?;
    Ok(panel)
}

fn build_inspector_panel(
    ui: &mut Ui<Message>,
    root: ElementHandle<astrelis_ui_core::Column>,
) -> io::Result<ElementHandle<astrelis_ui_core::Column>> {
    let panel = ui.add_column(root).map_err(io::Error::other)?;
    ui.add_label(panel, "Inspector — edit these values, then move the panel.")
        .map_err(io::Error::other)?;
    ui.add_text_field(panel, "Player")
        .map_err(io::Error::other)?;
    ui.add_label(panel, "Visible").map_err(io::Error::other)?;
    ui.add_checkbox(panel, true).map_err(io::Error::other)?;
    ui.add_label(panel, "Opacity").map_err(io::Error::other)?;
    ui.add_slider(panel, 0.0, 100.0, 1.0, 75.0)
        .map_err(io::Error::other)?;
    Ok(panel)
}

fn build_assets_panel(
    ui: &mut Ui<Message>,
    root: ElementHandle<astrelis_ui_core::Column>,
) -> io::Result<ElementHandle<astrelis_ui_core::Column>> {
    let panel = ui.add_column(root).map_err(io::Error::other)?;
    ui.add_label(panel, "Assets").map_err(io::Error::other)?;
    for asset in ["player.mesh", "level.scene", "ui.theme", "music.ogg"] {
        ui.add_button(panel, asset).map_err(io::Error::other)?;
    }
    Ok(panel)
}

fn build_console_panel(
    ui: &mut Ui<Message>,
    root: ElementHandle<astrelis_ui_core::Column>,
) -> io::Result<ElementHandle<astrelis_ui_core::Column>> {
    let panel = ui.add_column(root).map_err(io::Error::other)?;
    ui.add_label(panel, "Console output")
        .map_err(io::Error::other)?;
    ui.add_label(panel, "[info] Docking workspace initialized")
        .map_err(io::Error::other)?;
    ui.add_label(panel, "[hint] Ctrl+W closes the focused optional tab")
        .map_err(io::Error::other)?;
    Ok(panel)
}

fn default_layout(
    scene: &PanelId,
    inspector: &PanelId,
    assets: &PanelId,
    console: &PanelId,
) -> DockLayout {
    DockLayout {
        root: Some(DockNode::Split {
            axis: DockAxis::Vertical,
            ratio: 0.72,
            first: Box::new(DockNode::Split {
                axis: DockAxis::Horizontal,
                ratio: 0.7,
                first: Box::new(DockNode::Tabs(
                    DockTabs::new(vec![scene.clone()]).expect("scene group is non-empty"),
                )),
                second: Box::new(DockNode::Tabs(
                    DockTabs::new(vec![inspector.clone(), assets.clone()])
                        .expect("inspector group is non-empty"),
                )),
            }),
            second: Box::new(DockNode::Tabs(
                DockTabs::new(vec![console.clone()]).expect("console group is non-empty"),
            )),
        }),
        floating: Vec::new(),
    }
}

const STALE_LAYOUT: &str = r#"{
  "root": {
    "kind": "tabs",
    "panels": ["removed-plugin", "scene", "scene"],
    "active": "removed-plugin"
  },
  "floating": [{
    "tabs": {"panels": ["inspector"], "active": "missing"},
    "bounds": {"x": 5000.0, "y": 5000.0, "width": 40.0, "height": 20.0}
  }]
}"#;

async fn initialize_gpu(
    instance: astrelis_gpu::Instance,
    surface: astrelis_gpu::Surface,
    window: Window,
) -> io::Result<GpuState> {
    let adapter = instance
        .request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        })
        .await
        .map_err(io::Error::other)?;
    let (device, queue) = adapter
        .request_device(DeviceDescriptor::default())
        .await
        .map_err(io::Error::other)?;
    let capabilities = surface.capabilities(&adapter).map_err(io::Error::other)?;
    let format = capabilities
        .formats
        .first()
        .copied()
        .ok_or_else(|| io::Error::other("surface reported no formats"))?;
    let size = window.inner_size().map_err(io::Error::other)?;
    let configuration = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format,
        view_formats: vec![],
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
    Ok(GpuState {
        surface,
        device,
        queue,
        configuration,
        renderer,
    })
}

impl App for DockingExample {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis docking workspace E2E".into(),
                inner_size: Some(Size::new(1180.0, 760.0)),
                ..Default::default()
            })
            .map_err(io::Error::other)?;
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .map_err(io::Error::other)?;
        self.window = Some(window.clone());
        self.install_gpu(pollster::block_on(initialize_gpu(
            self.instance.clone(),
            surface,
            window,
        ))?)?;
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
            self.consume_messages()?;
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
        DockingExample::new().map_err(astrelis_app::RuntimeError::Application)?,
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}
