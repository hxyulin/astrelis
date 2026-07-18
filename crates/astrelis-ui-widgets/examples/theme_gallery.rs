//! Native and WebGPU gallery for the Astrelis theme token system.
//!
//! Toggling the theme calls `set_theme`, which restyles every already-created
//! control live — the staleness fix from Milestone 19 — and swaps the whole
//! palette between the dark and light themes. Custom `Swatch` widgets resolve
//! their color from the active theme at paint time, so the status colors track
//! the toggle without any per-widget bookkeeping.

use std::{any::Any, io, sync::Arc};

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::{
    color::Color,
    geometry::{LogicalRect, Size},
};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::{Brush, CornerRadii, Painter, RoundedRect};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};
use astrelis_text::{FontDatabase, FontFamily};
use astrelis_ui_core::{
    Column, ElementHandle, EventFilter, EventPhase, Insets, Label, LayoutStyle, Length, Theme, Ui,
    UiError, Widget, WidgetStyle,
};
use astrelis_ui_widgets::Tooltip;

const NOTO_SANS: &[u8] = include_bytes!("../../astrelis-ui-core/assets/NotoSans.ttf");
const FONT_NAME: &str = "Noto Sans";

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    renderer: Renderer,
}

#[derive(Clone)]
enum Message {
    ToggleTheme,
}

/// A theme-driven color chip that resolves its fill at paint time.
struct Swatch {
    pick: fn(&Theme) -> Color,
}

impl Widget<Message> for Swatch {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        theme: &Theme,
    ) -> Result<(), UiError> {
        let rounded = RoundedRect::new(bounds, CornerRadii::uniform(theme.radii.md))
            .map_err(|error| UiError::from_message(error.to_string()))?;
        painter
            .fill_rounded_rect(rounded, Brush::Solid((self.pick)(theme)))
            .map_err(|error| UiError::from_message(error.to_string()))
    }
}

struct Gallery {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    ui: Ui<Message>,
    dark: bool,
    theme_status: ElementHandle<Label>,
}

impl Gallery {
    fn new() -> io::Result<Self> {
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(NOTO_SANS))
            .map_err(io::Error::other)?;
        let mut ui = Ui::<Message>::new(fonts, themed(true));
        let theme_status = build(&mut ui).map_err(io::Error::other)?;

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
            dark: true,
            theme_status,
        })
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

    fn consume_messages(&mut self) -> io::Result<()> {
        let mut toggled = false;
        for message in self.ui.drain_messages().collect::<Vec<_>>() {
            match message {
                Message::ToggleTheme => toggled = true,
            }
        }
        if toggled {
            self.dark = !self.dark;
            self.ui.set_theme(themed(self.dark));
            self.ui
                .set_label_text(
                    self.theme_status,
                    format!(
                        "Active theme: {} — every control above restyled live.",
                        if self.dark { "dark" } else { "light" }
                    ),
                )
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

/// Builds a theme with the gallery's registered font family.
fn themed(dark: bool) -> Theme {
    let mut theme = if dark { Theme::dark() } else { Theme::light() };
    theme.font_families = vec![FontFamily::Named(FONT_NAME.into())];
    theme
}

/// Populates the tree and returns the handle of the theme-status label.
fn build(ui: &mut Ui<Message>) -> Result<ElementHandle<Label>, UiError> {
    let type_scale = ui.theme().type_scale;

    let root = ui.root();
    let padding = ui.add_padding(root, Insets::all(28.0))?;
    ui.set_layout(
        padding,
        LayoutStyle {
            grow: 1.0,
            ..Default::default()
        },
    )?;
    let scroll = ui.add_scroll_view(padding)?;
    ui.set_layout(
        scroll,
        LayoutStyle {
            grow: 1.0,
            ..Default::default()
        },
    )?;
    let column = ui.add_column(scroll)?;

    let toggle = ui.add_button(column, "Toggle light / dark")?;
    ui.listen(
        toggle,
        Some(EventPhase::Target),
        EventFilter::Activate,
        |context, _| context.emit(Message::ToggleTheme),
    )?;
    let theme_status = ui.add_label(
        column,
        "Active theme: dark — every control above restyled live.",
    )?;

    // Typography: the three type-scale steps, sized through per-element
    // font-size overrides.
    ui.add_label(column, "Type scale")?;
    let headings = ui.add_row(column)?;
    for (size, text) in [
        (type_scale.heading, "Heading"),
        (type_scale.body, "Body"),
        (type_scale.caption, "Caption"),
    ] {
        let label = ui.add_label(headings, text)?;
        ui.set_widget_style(
            label,
            WidgetStyle {
                font_size: Some(size),
                ..Default::default()
            },
        )?;
    }

    // Buttons: enabled reacts to hover and press; disabled dims its label
    // through the disabled-foreground token.
    ui.add_label(column, "Buttons (hover and press to see states)")?;
    let buttons = ui.add_row(column)?;
    ui.add_button(buttons, "Enabled")?;
    let disabled = ui.add_button(buttons, "Disabled")?;
    ui.set_enabled(disabled, false)?;

    // Selection controls.
    ui.add_label(column, "Selection controls")?;
    let controls = ui.add_row(column)?;
    ui.add_checkbox(controls, true)?;
    ui.add_checkbox(controls, false)?;
    let disabled_checkbox = ui.add_checkbox(controls, true)?;
    ui.set_enabled(disabled_checkbox, false)?;
    let slider = ui.add_slider(controls, 0.0, 1.0, 0.05, 0.5)?;
    ui.set_layout(
        slider,
        LayoutStyle {
            width: Length::Px(200.0),
            ..Default::default()
        },
    )?;

    // Text input.
    ui.add_label(column, "Text field")?;
    ui.add_text_field(column, "Editable value")?;

    // Status colors, each a theme-driven swatch that tracks the toggle.
    ui.add_label(column, "Status colors")?;
    let swatches = ui.add_row(column)?;
    for (name, pick) in [
        (
            "Danger",
            (|theme: &Theme| theme.danger) as fn(&Theme) -> Color,
        ),
        ("Success", |theme: &Theme| theme.success),
        ("Warning", |theme: &Theme| theme.warning),
    ] {
        let cell: ElementHandle<Column> = ui.add_column(swatches)?;
        let swatch = ui.add_widget(cell, Swatch { pick })?;
        ui.set_layout(
            swatch,
            LayoutStyle {
                width: Length::Px(140.0),
                height: Length::Px(44.0),
                ..Default::default()
            },
        )?;
        ui.add_label(cell, name)?;
    }

    // Elevation: an overlay surface (the tooltip) casts the shadow token.
    ui.add_label(column, "Elevation")?;
    let tooltip_owner = ui.add_button(column, "Hover for an elevated tooltip")?;
    Tooltip::new(
        ui,
        tooltip_owner,
        "Overlays paint the shadow token behind their surface.",
    )?;

    Ok(theme_status)
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
                title: "Astrelis theme gallery".into(),
                #[cfg(not(target_arch = "wasm32"))]
                inner_size: Some(Size::new(720.0, 720.0)),
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
/// Starts the theme gallery in the host page canvas.
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
