//! Cross-platform settings window and Milestone 10 interaction gallery, built
//! through the `astrelis-ui` facade.
//!
//! This is a faithful port of `astrelis-ui-core`'s `settings_window.rs`: the
//! GPU, windowing, and event-loop plumbing is unchanged, but the tree is
//! constructed with the facade instead of the raw `add_*` + `set_*` + `map_err`
//! sequence. Compare `Settings::new` here with the original to see the
//! construction path collapse into infallible chains. The `ui-core` original is
//! retained as the low-level reference.

use std::{io, sync::Arc};

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::geometry::Size;
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::{Brush, CornerRadii, Painter, RoundedRect, StrokeStyle};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};
use astrelis_text::{FontDatabase, FontFamily};
use astrelis_ui::prelude::*;

const NOTO_SANS: &[u8] = include_bytes!("../../astrelis-ui-core/assets/NotoSans.ttf");

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
    overlay: ElementHandle<Overlay>,
}

enum Message {
    Save,
    Edited,
    Notifications(bool),
    Scale(f32),
    ToggleOverlay,
}

/// A titled, framed section that mounts a label and an empty content column.
///
/// `widget_any!` supplies the `as_any`/`as_any_mut` identity casts so only the
/// mounting, painting, and semantics behaviour is written by hand.
struct SettingsSection {
    title: String,
    content: Option<ElementHandle<Column>>,
}

impl SettingsSection {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: None,
        }
    }
    fn content(&self) -> ElementHandle<Column> {
        self.content.expect("settings section is mounted")
    }
}

impl Widget<Message> for SettingsSection {
    widget_any!();

    fn mounted(&mut self, context: &mut MountContext<'_, Message>) -> Result<(), UiError> {
        context.add_label(self.title.clone())?;
        self.content = Some(context.add_column()?);
        Ok(())
    }

    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        theme: &Theme,
    ) -> Result<(), UiError> {
        let rounded = RoundedRect::new(bounds, CornerRadii::uniform(theme.radii.lg))?;
        painter.fill_rounded_rect(rounded, Brush::Solid(theme.field_background))?;
        painter.stroke_rounded_rect(
            rounded,
            StrokeStyle {
                width: theme.border_width,
                ..Default::default()
            },
            Brush::Solid(theme.border),
        )?;
        Ok(())
    }

    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((SemanticRole::Group, self.title.clone(), None))
    }
}

struct Settings {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui<Message>,
    handles: Handles,
    overlay_open: bool,
}

impl Settings {
    fn new() -> io::Result<Self> {
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(NOTO_SANS))
            .map_err(io::Error::other)?;
        let theme = Theme {
            font_families: vec![FontFamily::Named("Noto Sans".into())],
            ..Default::default()
        };
        let mut ui = Ui::<Message>::new(fonts, theme);
        let root = ui.root();

        // padding -> scroll view -> content column, in one chain.
        let column = ui
            .padding(root, Insets::all(28.0))
            .grow(1.0)
            .scroll_view()
            .grow(1.0)
            .column()
            .finish();
        ui.label(column, "Astrelis settings").finish();
        ui.label(column, "Milestone 10 interaction gallery")
            .finish();

        // A wrapping row of percent-sized cards.
        let cards = ui
            .row(column)
            .width(percent(1.0))
            .flex(FlexStyle {
                wrap: FlexWrap::Wrap,
                column_gap: 8.0,
                row_gap: 8.0,
                ..Default::default()
            })
            .finish();
        for title in [
            "Wrapped layout",
            "Percent sizing",
            "Keyboard focus",
            "Custom cursor",
        ] {
            ui.button(cards, title)
                .layout(
                    layout()
                        .width(percent(0.48))
                        .min_width(px(220.0))
                        .min_height(px(42.0)),
                )
                .finish();
        }

        // An absolutely-positioned overlapping stage with a transformed child.
        let stage = ui
            .stack(column)
            .width(percent(1.0))
            .height(px(92.0))
            .overflow(Overflow::Clip)
            .finish();
        ui.button(stage, "Overlapping target")
            .layout(
                layout()
                    .width(percent(0.58))
                    .height(px(58.0))
                    .positioning(Positioning::Absolute)
                    .inset(Edges {
                        left: px(12.0),
                        top: px(12.0),
                        ..Default::default()
                    }),
            )
            .cursor_icon(Some(CursorIcon::Crosshair))
            .finish();
        ui.button(stage, "Transformed + z-index")
            .layout(
                layout()
                    .width(percent(0.48))
                    .height(px(52.0))
                    .positioning(Positioning::Absolute)
                    .inset(Edges {
                        left: percent(0.36),
                        top: px(28.0),
                        ..Default::default()
                    }),
            )
            .z_index(2)
            .transform(Affine2::from_angle(-0.04), Point::new(80.0, 30.0))
            .finish();

        // An owner button that toggles a viewport-hosted overlay.
        let overlay_owner = ui
            .button(column, "Anchored overlay owner")
            .cursor_icon(Some(CursorIcon::Pointer))
            .finish();
        ui.on_click(overlay_owner, |context| {
            context.emit(Message::ToggleOverlay)
        });
        let overlay = ui
            .overlay(
                overlay_owner,
                OverlayOptions {
                    offset: Point::new(0.0, 6.0),
                    z_index: 20,
                    focus: FocusScopeOptions {
                        trapped: true,
                        autofocus: false,
                        restore_focus: true,
                    },
                    ..Default::default()
                },
            )
            .width(px(220.0))
            .visibility(Visibility::Hidden)
            .style(WidgetStyle {
                background: Some(Color::new(0.12, 0.16, 0.26, 1.0)),
                ..Default::default()
            })
            .finish();
        ui.label(overlay, "Viewport-hosted portal").finish();
        ui.button(overlay, "Tab stays in this overlay").finish();

        // Two text fields, one a password.
        ui.label(column, "Display name").finish();
        let name = ui
            .text_field(column, "Explorer")
            .placeholder("Your display name")
            .min_width(px(340.0))
            .finish();
        ui.label(column, "Access token").finish();
        let secret = ui
            .text_field(column, "")
            .placeholder("Paste a token")
            .password(true)
            .min_width(px(340.0))
            .finish();

        // A custom framed section, its content column read back after mounting.
        let preferences = ui
            .mount(column, SettingsSection::new("Preferences"))
            .finish();
        let preferences_content = ui
            .widget(preferences)
            .expect("preferences section is mounted")
            .content();
        ui.label(preferences_content, "Enable notifications")
            .finish();
        let notifications = ui.checkbox(preferences_content, true).finish();
        ui.label(preferences_content, "Interface scale").finish();
        let scale = ui
            .slider(preferences_content, 0.75, 1.5, 0.05, 1.0)
            .finish();

        let save = ui.button(column, "Save settings").finish();
        ui.on_click(save, |context| context.emit(Message::Save));
        ui.on_checked(notifications, |context, value| {
            context.emit(Message::Notifications(value))
        });
        ui.on_slider(scale, |context, value| context.emit(Message::Scale(value)));
        for field in [name, secret] {
            ui.on_text_changed(field, |context, _| context.emit(Message::Edited));
        }

        let clipboard_note = if cfg!(target_arch = "wasm32") {
            "Tab through controls; browser clipboard is not enabled in this slice."
        } else {
            "Tab through controls; clipboard and IME are enabled."
        };
        let status = ui.label(column, clipboard_note).finish();

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
            handles: Handles {
                name,
                secret,
                status,
                overlay,
            },
            overlay_open: false,
        })
    }

    fn install_gpu(&mut self, gpu: GpuState) -> io::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| io::Error::other("window was closed during GPU initialization"))?;
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
            self.ui.set_viewport(
                Size::new(width as f32 / scale, height as f32 / scale),
                scale,
            );
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
                    clear_color: Color::BLACK,
                },
            )
            .map_err(io::Error::other)?;
        gpu.queue
            .submit([encoder.finish().map_err(io::Error::other)?])
            .map_err(io::Error::other)?;
        frame.present().map_err(io::Error::other)?;
        #[cfg(target_arch = "wasm32")]
        set_web_status(None);
        Ok(())
    }

    fn consume_events(&mut self) -> io::Result<()> {
        let messages = self.ui.drain_messages().collect::<Vec<_>>();
        for message in messages {
            match message {
                Message::Save => {
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
                Message::Edited => {
                    self.ui
                        .set_label_text(
                            self.handles.status,
                            "Press Save settings to apply changes.",
                        )
                        .map_err(io::Error::other)?;
                }
                Message::Notifications(value) => self
                    .ui
                    .set_label_text(
                        self.handles.status,
                        format!("Notifications enabled: {value}."),
                    )
                    .map_err(io::Error::other)?,
                Message::Scale(value) => self
                    .ui
                    .set_label_text(
                        self.handles.status,
                        format!("Interface scale: {value:.2}×."),
                    )
                    .map_err(io::Error::other)?,
                Message::ToggleOverlay => {
                    self.overlay_open = !self.overlay_open;
                    self.ui
                        .set_visibility(
                            self.handles.overlay,
                            if self.overlay_open {
                                Visibility::Visible
                            } else {
                                Visibility::Hidden
                            },
                        )
                        .map_err(io::Error::other)?;
                }
            }
        }
        Ok(())
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
        .ok_or_else(|| io::Error::other("surface reported no supported formats"))?;
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
    Ok(GpuState {
        surface,
        device,
        queue,
        configuration,
        renderer,
    })
}

impl App for Settings {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let attributes = WindowAttributes {
            title: "Astrelis settings".into(),
            #[cfg(not(target_arch = "wasm32"))]
            inner_size: Some(Size::new(760.0, 520.0)),
            ..Default::default()
        };
        let window = context
            .create_window(attributes)
            .map_err(io::Error::other)?;
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .map_err(io::Error::other)?;
        self.window = Some(window.clone());

        #[cfg(not(target_arch = "wasm32"))]
        {
            let gpu = pollster::block_on(initialize_gpu(self.instance.clone(), surface, window))?;
            self.install_gpu(gpu)?;
        }

        #[cfg(target_arch = "wasm32")]
        {
            let instance = self.instance.clone();
            let proxy = context.proxy();
            wasm_bindgen_futures::spawn_local(async move {
                let result = initialize_gpu(instance, surface, window).await;
                let _ = proxy.run_on_main_thread(move |app, context| {
                    match result {
                        Ok(gpu) => {
                            app.install_gpu(gpu)?;
                            if let Some(window) = &app.window {
                                context.invalidate_window(window.id());
                            }
                        }
                        Err(error) => {
                            set_web_status(Some(&format!("WebGPU initialization failed: {error}")));
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

#[cfg(not(target_arch = "wasm32"))]
/// Runs the native interaction gallery.
pub fn main() -> Result<(), astrelis_app::RuntimeError<io::Error>> {
    Runtime::finish(astrelis_platform_winit::run_return(Runtime::new(
        Settings::new().map_err(astrelis_app::RuntimeError::Application)?,
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// Starts the settings example in its host page canvas.
pub fn start() -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("browser document is unavailable"))?;
    let canvas = document
        .get_element_by_id("astrelis-canvas")
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("#astrelis-canvas was not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| wasm_bindgen::JsValue::from_str("#astrelis-canvas is not a canvas"))?;
    let app =
        Settings::new().map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    astrelis_platform_winit::web::spawn_on_canvas(
        Runtime::new(app, RuntimeConfig::default()),
        canvas,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn set_web_status(message: Option<&str>) {
    let Some(status) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("astrelis-status"))
    else {
        return;
    };
    if let Some(message) = message {
        status.set_text_content(Some(message));
        let _ = status.remove_attribute("hidden");
    } else {
        let _ = status.set_attribute("hidden", "");
    }
}

#[cfg(target_arch = "wasm32")]
/// WASM startup is driven by the exported async entry point above.
pub fn main() {}
