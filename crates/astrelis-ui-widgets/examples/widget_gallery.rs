//! Native and WebGPU gallery for reusable Astrelis widgets.

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
    DragPayload, DropOperation, ElementHandle, Label, LayoutStyle, Length, Theme, Ui,
};
use astrelis_ui_widgets::{
    DropZone, Form, List, ListItem, Menu, MenuItem, Popover, SplitAxis, SplitPane,
    SplitPaneOptions, Tabs, Tooltip, VirtualList, VirtualListOptions, install_drag_source,
    move_drag_options,
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
struct Card(&'static str);

#[derive(Clone)]
enum Message {
    Dropped(String),
    Status(String),
}

struct Gallery {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui<Message>,
    status: ElementHandle<Label>,
    virtual_list: VirtualList,
    virtual_status: ElementHandle<Label>,
}

impl Gallery {
    fn new() -> io::Result<Self> {
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(NOTO_SANS))
            .map_err(io::Error::other)?;
        let mut ui = Ui::<Message>::new(
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
        let scroll = ui.add_scroll_view(padding).map_err(io::Error::other)?;
        ui.set_layout(
            scroll,
            LayoutStyle {
                grow: 1.0,
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        let column = ui.add_column(scroll).map_err(io::Error::other)?;
        ui.add_label(column, "Milestone 11 widget gallery")
            .map_err(io::Error::other)?;
        ui.add_label(
            column,
            "Drag a card into the accepting zone. Click without moving to test the threshold.",
        )
        .map_err(io::Error::other)?;

        let sources = ui.add_row(column).map_err(io::Error::other)?;
        for name in ["Orion", "Lyra", "Draco"] {
            let source = ui.add_button(sources, name).map_err(io::Error::other)?;
            ui.set_layout(
                source,
                LayoutStyle {
                    width: Length::Px(150.0),
                    min_height: Length::Px(52.0),
                    ..Default::default()
                },
            )
            .map_err(io::Error::other)?;
            install_drag_source(&mut ui, source, move_drag_options(), move || {
                DragPayload::new(Card(name))
            })
            .map_err(io::Error::other)?;
        }

        let zones = ui.add_row(column).map_err(io::Error::other)?;
        let accepting = ui
            .add_widget(
                zones,
                DropZone::new(
                    "Accepts Card payloads",
                    DropOperation::Move,
                    |payload| payload.downcast_ref::<Card>().is_some(),
                    |payload, operation| {
                        let card = payload
                            .downcast_ref::<Card>()
                            .expect("accepted payload remains a Card");
                        Message::Dropped(format!("Dropped {} using {operation:?}.", card.0))
                    },
                ),
            )
            .map_err(io::Error::other)?;
        ui.set_layout(
            accepting,
            LayoutStyle {
                grow: 1.0,
                min_height: Length::Px(110.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        let rejected = ui
            .add_widget(
                zones,
                DropZone::new(
                    "Rejects Card payloads",
                    DropOperation::Copy,
                    |_payload| false,
                    |_payload, _operation| unreachable!("rejecting zone cannot receive a drop"),
                ),
            )
            .map_err(io::Error::other)?;
        ui.set_layout(
            rejected,
            LayoutStyle {
                grow: 1.0,
                min_height: Length::Px(110.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        let status = ui
            .add_label(column, "No drop yet. Escape cancels an active drag.")
            .map_err(io::Error::other)?;

        ui.add_label(column, "Horizontal split pane")
            .map_err(io::Error::other)?;
        let horizontal = SplitPane::new(
            &mut ui,
            column,
            SplitPaneOptions {
                ratio: 0.35,
                first_min: 120.0,
                second_min: 140.0,
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        horizontal
            .set_container_layout(
                &mut ui,
                LayoutStyle {
                    width: Length::Percent(1.0),
                    height: Length::Px(150.0),
                    ..Default::default()
                },
            )
            .map_err(io::Error::other)?;
        ui.add_label(horizontal.first(), "Navigation\nMinimum 120 px")
            .map_err(io::Error::other)?;
        ui.add_button(horizontal.first(), "First pane")
            .map_err(io::Error::other)?;
        ui.add_label(horizontal.second(), "Inspector\nMinimum 140 px")
            .map_err(io::Error::other)?;
        ui.add_button(horizontal.second(), "Second pane")
            .map_err(io::Error::other)?;

        ui.add_label(column, "Vertical split pane")
            .map_err(io::Error::other)?;
        let vertical = SplitPane::new(
            &mut ui,
            column,
            SplitPaneOptions {
                axis: SplitAxis::Vertical,
                ratio: 0.55,
                first_min: 60.0,
                second_min: 60.0,
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        vertical
            .set_container_layout(
                &mut ui,
                LayoutStyle {
                    width: Length::Percent(1.0),
                    height: Length::Px(210.0),
                    ..Default::default()
                },
            )
            .map_err(io::Error::other)?;
        ui.add_label(vertical.first(), "Editor region")
            .map_err(io::Error::other)?;
        ui.add_button(vertical.first(), "Upper pane")
            .map_err(io::Error::other)?;
        ui.add_label(vertical.second(), "Output region")
            .map_err(io::Error::other)?;
        ui.add_button(vertical.second(), "Lower pane")
            .map_err(io::Error::other)?;

        ui.add_label(column, "Overlay and navigation widgets")
            .map_err(io::Error::other)?;
        let controls = ui.add_row(column).map_err(io::Error::other)?;
        let tooltip_owner = ui
            .add_button(controls, "Hover or focus me")
            .map_err(io::Error::other)?;
        Tooltip::new(
            &mut ui,
            tooltip_owner,
            "Immediate tooltip shared by pointer hover and keyboard focus.",
        )
        .map_err(io::Error::other)?;

        let popover_owner = ui
            .add_button(controls, "Toggle popover")
            .map_err(io::Error::other)?;
        let popover = Popover::new(
            &mut ui,
            popover_owner,
            astrelis_ui_core::OverlayOptions {
                offset: astrelis_core::geometry::Point::new(0.0, 6.0),
                z_index: 70,
                focus: astrelis_ui_core::FocusScopeOptions {
                    trapped: true,
                    autofocus: true,
                    restore_focus: true,
                },
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;
        ui.add_label(popover.content(), "Arbitrary popover content")
            .map_err(io::Error::other)?;
        ui.add_button(popover.content(), "Focusable popover action")
            .map_err(io::Error::other)?;

        let menu_owner = ui
            .add_button(controls, "Open menu")
            .map_err(io::Error::other)?;
        Menu::new(
            &mut ui,
            menu_owner,
            vec![
                MenuItem {
                    label: "New document".into(),
                    message: Message::Status("Menu selected: New document.".into()),
                    enabled: true,
                },
                MenuItem {
                    label: "Disabled action".into(),
                    message: Message::Status("Disabled item activated unexpectedly.".into()),
                    enabled: false,
                },
                MenuItem {
                    label: "Close document".into(),
                    message: Message::Status("Menu selected: Close document.".into()),
                    enabled: true,
                },
            ],
        )
        .map_err(io::Error::other)?;

        let tabs = Tabs::new(&mut ui, column, ["General", "Display", "Advanced"])
            .map_err(io::Error::other)?;
        ui.add_label(tabs.panels()[0], "General tab content")
            .map_err(io::Error::other)?;
        ui.add_label(tabs.panels()[1], "Display tab content")
            .map_err(io::Error::other)?;
        ui.add_label(tabs.panels()[2], "Advanced tab content")
            .map_err(io::Error::other)?;

        ui.add_label(column, "Selectable list")
            .map_err(io::Error::other)?;
        List::new(
            &mut ui,
            column,
            vec![
                ListItem {
                    label: "Mercury".into(),
                    message: Message::Status("Selected Mercury.".into()),
                    enabled: true,
                },
                ListItem {
                    label: "Venus (disabled)".into(),
                    message: Message::Status("Disabled list item activated.".into()),
                    enabled: false,
                },
                ListItem {
                    label: "Earth".into(),
                    message: Message::Status("Selected Earth.".into()),
                    enabled: true,
                },
            ],
        )
        .map_err(io::Error::other)?;

        ui.add_label(column, "Form compositions")
            .map_err(io::Error::other)?;
        let form = Form::new(&mut ui, column).map_err(io::Error::other)?;
        form.add_text_field(
            &mut ui,
            "Workspace name",
            "Astrelis",
            Some("Names are stored locally for this gallery."),
        )
        .map_err(io::Error::other)?;
        form.add_checkbox(&mut ui, "Enable previews", true)
            .map_err(io::Error::other)?;
        form.add_slider(&mut ui, "Preview scale", 0.5..=2.0, 0.1, 1.0)
            .map_err(io::Error::other)?;
        form.add_status(&mut ui, "Validation: ready")
            .map_err(io::Error::other)?;

        ui.add_label(column, "Virtual list: 10,000 retained-data rows")
            .map_err(io::Error::other)?;
        let virtual_status = ui
            .add_label(column, "Realized range will appear after layout.")
            .map_err(io::Error::other)?;
        let virtual_list = VirtualList::new(
            &mut ui,
            column,
            VirtualListOptions {
                item_extent: 40.0,
                overscan: 4,
            },
        )
        .map_err(io::Error::other)?;
        ui.set_layout(
            virtual_list.scroll_view(),
            LayoutStyle {
                width: Length::Percent(1.0),
                height: Length::Px(360.0),
                ..Default::default()
            },
        )
        .map_err(io::Error::other)?;

        #[cfg(target_arch = "wasm32")]
        let descriptor = astrelis_gpu_wgpu::InstanceDescriptor {
            use_environment: false,
            ..Default::default()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let descriptor = astrelis_gpu_wgpu::InstanceDescriptor::default();
        Ok(Self {
            instance: astrelis_gpu_wgpu::create_instance(descriptor),
            window: None,
            gpu: None,
            ui,
            status,
            virtual_list,
            virtual_status,
        })
    }

    fn sync_virtual_list(&mut self) -> io::Result<()> {
        self.virtual_list
            .sync(&mut self.ui, 10_000, |ui, item, index| {
                ui.add_label(item, format!("Row {index:05} — retained only while nearby"))?;
                Ok(())
            })
            .map_err(io::Error::other)?;
        let range = self.virtual_list.realized_range();
        self.ui
            .set_label_text(
                self.virtual_status,
                format!(
                    "Realized {} rows: {}..{}",
                    self.virtual_list.realized_count(),
                    range.start,
                    range.end
                ),
            )
            .map_err(io::Error::other)
    }

    fn resize_virtual_list(&mut self, viewport_height: f32) -> io::Result<()> {
        self.ui
            .set_layout(
                self.virtual_list.scroll_view(),
                LayoutStyle {
                    width: Length::Percent(1.0),
                    height: Length::Px((viewport_height * 0.5).clamp(220.0, 520.0)),
                    ..Default::default()
                },
            )
            .map_err(io::Error::other)
    }

    fn install_gpu(&mut self, gpu: GpuState) -> io::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| io::Error::other("window closed during GPU initialization"))?;
        let scale = window.scale_factor() as f32;
        let viewport = Size::new(
            gpu.configuration.width as f32 / scale,
            gpu.configuration.height as f32 / scale,
        );
        self.ui.set_viewport(viewport, scale);
        self.gpu = Some(gpu);
        self.resize_virtual_list(viewport.height)?;
        self.sync_virtual_list()?;
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
            self.ui.set_viewport(
                Size::new(width as f32 / scale, height as f32 / scale),
                scale,
            );
            self.resize_virtual_list(height as f32 / scale)?;
            self.sync_virtual_list()?;
        }
        Ok(())
    }

    fn consume_messages(&mut self) -> io::Result<()> {
        for message in self.ui.drain_messages().collect::<Vec<_>>() {
            let status = match message {
                Message::Dropped(status) | Message::Status(status) => status,
            };
            self.ui
                .set_label_text(self.status, status)
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

impl App for Gallery {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis widget gallery".into(),
                #[cfg(not(target_arch = "wasm32"))]
                inner_size: Some(Size::new(820.0, 720.0)),
                ..Default::default()
            })
            .map_err(io::Error::other)?;
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .map_err(io::Error::other)?;
        self.window = Some(window.clone());

        #[cfg(not(target_arch = "wasm32"))]
        self.install_gpu(pollster::block_on(initialize_gpu(
            self.instance.clone(),
            surface,
            window,
        ))?)?;

        #[cfg(target_arch = "wasm32")]
        {
            let instance = self.instance.clone();
            let proxy = context.proxy();
            wasm_bindgen_futures::spawn_local(async move {
                let result = initialize_gpu(instance, surface, window).await;
                let _ = proxy.run_on_main_thread(move |app, context| {
                    if let Ok(gpu) = result {
                        app.install_gpu(gpu)?;
                        if let Some(window) = &app.window {
                            context.invalidate_window(window.id());
                        }
                    }
                    Ok(())
                });
            });
        }
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
            self.sync_virtual_list()?;
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

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), astrelis_app::RuntimeError<io::Error>> {
    Runtime::finish(astrelis_platform_winit::run_return(Runtime::new(
        Gallery::new().map_err(astrelis_app::RuntimeError::Application)?,
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// Starts the widget gallery in the host page canvas.
pub fn start() -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("browser document unavailable"))?;
    let canvas = document
        .get_element_by_id("astrelis-canvas")
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("#astrelis-canvas not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| wasm_bindgen::JsValue::from_str("#astrelis-canvas is not a canvas"))?;
    astrelis_platform_winit::web::spawn_on_canvas(
        Runtime::new(
            Gallery::new().map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?,
            RuntimeConfig::default(),
        ),
        canvas,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn main() {}
