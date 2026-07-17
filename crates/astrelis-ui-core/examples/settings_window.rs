//! Cross-platform settings window using the retained UI vertical slice.

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
    Column, ElementHandle, Insets, Label, LayoutStyle, TextField, Theme, Ui, UiEventKind,
};

const NOTO_SANS: &[u8] = include_bytes!("../assets/NotoSans.ttf");

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
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(NOTO_SANS))
            .map_err(io::Error::other)?;
        let theme = Theme {
            font_families: vec![FontFamily::Named("Noto Sans".into())],
            ..Default::default()
        };
        let mut ui = Ui::new(fonts, theme);
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
        let clipboard_note = if cfg!(target_arch = "wasm32") {
            "Tab through controls; browser clipboard is not enabled in this slice."
        } else {
            "Tab through controls; clipboard and IME are enabled."
        };
        let status = ui
            .add_label(column, clipboard_note)
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
            handles: Handles {
                name,
                secret,
                status,
            },
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
                    clear_color: astrelis_core::color::Color::BLACK,
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
fn main() -> Result<(), astrelis_app::RuntimeError<io::Error>> {
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
fn main() {}
