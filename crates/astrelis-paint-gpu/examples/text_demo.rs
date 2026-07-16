//! Multilingual retained-text viewer and IME-aware single-line field.

use astrelis_core::{
    color::Color,
    geometry::{Point, Rect, Size},
};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::{Brush, Painter};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{
    Application, ElementState, ImeEvent, Key, Modifiers, NamedKey, PlatformContext, Window,
    WindowAttributes, WindowEvent, WindowId,
};
use astrelis_text::{
    CaretMovement, FontDatabase, ParagraphStyle, TextLayout, TextLayoutContext, TextLayoutRequest,
    TextPosition, TextSpan, TextStylePatch, TextWrap,
};

struct GpuState {
    window: Window,
    surface: astrelis_gpu::Surface,
    configuration: SurfaceConfiguration,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    renderer: Renderer,
}

struct Demo {
    instance: astrelis_gpu::Instance,
    gpu: Option<GpuState>,
    fonts: FontDatabase,
    layouts: TextLayoutContext,
    field: String,
    preedit: String,
    caret: TextPosition,
    field_layout: Option<TextLayout>,
    modifiers: Modifiers,
}

impl Default for Demo {
    fn default() -> Self {
        let field = "Edit me: مرحبا 👋".to_owned();
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            gpu: None,
            fonts: FontDatabase::default(),
            layouts: TextLayoutContext::new(),
            caret: TextPosition {
                byte_index: field.len(),
                ..Default::default()
            },
            field,
            preedit: String::new(),
            field_layout: None,
            modifiers: Modifiers::default(),
        }
    }
}

impl Demo {
    fn request_redraw(&self) {
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn insert(&mut self, value: &str) {
        self.field.insert_str(self.caret.byte_index, value);
        self.caret.byte_index += value.len();
    }

    fn backspace(&mut self) {
        if self.caret.byte_index == 0 {
            return;
        }
        let mut start = self.caret.byte_index - 1;
        while !self.field.is_char_boundary(start) {
            start -= 1;
        }
        self.field.replace_range(start..self.caret.byte_index, "");
        self.caret.byte_index = start;
    }

    fn make_list(&mut self, width: f32, height: f32) -> astrelis_paint::DisplayList {
        let title = "Astrelis text — English · العربية · עברית · हिन्दी · 中文 · 日本語 · 🌍✨";
        let mut viewer = TextLayoutRequest::new(format!(
            "{title}\n\nRetained layouts provide the same glyph positions for measurement, \
             painting, bidi caret movement, selection geometry, and IME placement."
        ));
        viewer.style.size = 22.0;
        viewer.style.color = Color::new(0.88, 0.92, 1.0, 1.0);
        viewer.paragraph = ParagraphStyle {
            max_width: Some((width - 64.0).max(0.0)),
            ..Default::default()
        };
        viewer.spans.push(TextSpan {
            range: 0..title.len(),
            style: TextStylePatch {
                size: Some(28.0),
                weight: Some(700.0),
                color: Some(Color::new(0.25, 0.9, 1.0, 1.0)),
                ..Default::default()
            },
        });
        let viewer = self
            .layouts
            .layout(&mut self.fonts, viewer)
            .expect("viewer layout");

        let mut shown = self.field.clone();
        shown.insert_str(self.caret.byte_index, &self.preedit);
        let shown_caret = self.caret.byte_index + self.preedit.len();
        let mut field = TextLayoutRequest::new(shown);
        field.style.size = 22.0;
        field.style.color = Color::WHITE;
        field.paragraph.wrap = TextWrap::NoWrap;
        if !self.preedit.is_empty() {
            field.spans.push(TextSpan {
                range: self.caret.byte_index..shown_caret,
                style: TextStylePatch {
                    underline: Some(true),
                    color: Some(Color::new(1.0, 0.82, 0.25, 1.0)),
                    ..Default::default()
                },
            });
        }
        let field = self
            .layouts
            .layout(&mut self.fonts, field)
            .expect("field layout");
        let caret = field.caret_rect(
            TextPosition {
                byte_index: shown_caret,
                ..Default::default()
            },
            1.0,
        );

        let field_origin = Point::new(32.0, 245.0);
        if let Some(gpu) = &self.gpu {
            gpu.window.set_ime_cursor_area(Rect::from_xywh(
                (field_origin.x + caret.origin.x) as f64,
                (field_origin.y + caret.origin.y) as f64,
                caret.size.width.max(1.0) as f64,
                caret.size.height as f64,
            ));
        }
        let mut painter = Painter::new();
        painter
            .fill_rect(
                Rect::from_xywh(0.0, 0.0, width, height),
                Brush::Solid(Color::new(0.025, 0.035, 0.065, 1.0)),
            )
            .unwrap();
        painter
            .draw_text(&viewer, Point::new(32.0, 32.0), 1.0)
            .unwrap();
        painter
            .fill_rect(
                Rect::from_xywh(22.0, 235.0, (width - 44.0).max(0.0), 48.0),
                Brush::Solid(Color::new(0.09, 0.12, 0.2, 1.0)),
            )
            .unwrap();
        painter.draw_text(&field, field_origin, 1.0).unwrap();
        painter
            .fill_rect(
                Rect::from_xywh(
                    field_origin.x + caret.origin.x,
                    field_origin.y + caret.origin.y,
                    caret.size.width.max(1.0),
                    caret.size.height,
                ),
                Brush::Solid(Color::WHITE),
            )
            .unwrap();
        self.field_layout = Some(field);
        painter.finish().unwrap()
    }

    fn redraw(&mut self) {
        let Some(gpu) = self.gpu.take() else { return };
        let frame = match gpu.surface.acquire().expect("acquire frame") {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .expect("recover surface");
                self.gpu = Some(gpu);
                return;
            }
            _ => {
                self.gpu = Some(gpu);
                return;
            }
        };
        let scale = gpu.window.scale_factor() as f32;
        let width = gpu.configuration.width as f32 / scale;
        let height = gpu.configuration.height as f32 / scale;
        self.gpu = Some(gpu);
        let list = self.make_list(width, height);
        let gpu = self.gpu.as_mut().expect("GPU state");
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
                    scale_factor: scale,
                    clear_color: Color::BLACK,
                },
            )
            .expect("render text");
        gpu.queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");
        frame.present().expect("present");
    }
}

impl Application for Demo {
    type UserEvent = ();

    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis multilingual text".into(),
                ..Default::default()
            })
            .expect("create window");
        window.set_ime_allowed(true);
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .expect("create surface");
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        }))
        .expect("request adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .expect("request device");
        let capabilities = surface.capabilities(&adapter).expect("capabilities");
        let size = window.inner_size().expect("window size");
        let configuration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
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
            .expect("configure");
        let renderer = Renderer::new(device.clone(), queue.clone(), RendererOptions::default())
            .expect("renderer");
        self.gpu = Some(GpuState {
            window,
            surface,
            configuration,
            device,
            queue,
            renderer,
        });
        self.request_redraw();
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => context.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::ModifiersChanged(value) => self.modifiers = value,
            WindowEvent::KeyboardInput(input) if input.state == ElementState::Pressed => {
                match &input.logical_key {
                    Key::Named(NamedKey::Backspace) if self.preedit.is_empty() => self.backspace(),
                    Key::Named(NamedKey::Other(name))
                        if self.preedit.is_empty()
                            && (name == "ArrowLeft" || name == "ArrowRight") =>
                    {
                        if let Some(layout) = &self.field_layout {
                            self.caret = layout.move_caret(
                                self.caret,
                                if name == "ArrowLeft" {
                                    CaretMovement::VisualLeft
                                } else {
                                    CaretMovement::VisualRight
                                },
                            );
                        }
                    }
                    _ if self.preedit.is_empty()
                        && !self.modifiers.control
                        && !self.modifiers.super_key =>
                    {
                        if let Some(text) = input.text.as_deref()
                            && !text.chars().any(char::is_control)
                        {
                            self.insert(text);
                        }
                    }
                    _ => {}
                }
                self.request_redraw();
            }
            WindowEvent::Ime(ImeEvent::Preedit(value, _)) => {
                self.preedit = value;
                self.request_redraw();
            }
            WindowEvent::Ime(ImeEvent::Commit(value)) => {
                self.preedit.clear();
                self.insert(&value);
                self.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu
                    && size.width > 0
                    && size.height > 0
                {
                    gpu.configuration.width = size.width;
                    gpu.configuration.height = size.height;
                    gpu.surface
                        .configure(&gpu.device, gpu.configuration.clone())
                        .expect("resize");
                    gpu.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.configuration.width = inner_size.width.max(1);
                    gpu.configuration.height = inner_size.height.max(1);
                    gpu.surface
                        .configure(&gpu.device, gpu.configuration.clone())
                        .expect("DPI resize");
                    gpu.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    astrelis_platform_winit::run(Demo::default()).expect("run text demo");
}
