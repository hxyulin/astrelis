//! Standalone text quality comparisons; Space toggles MSAA, Z toggles 2x/4x zoom.
//! Copy this file and the three bundled fonts (retaining their licenses).
//! Glyph preparation is explicit. This comparison does not implement distance fields.
use astrelis::{
    FrameError, GraphicsContext, Painter, PreparedText, Rect, RenderPass, RenderTarget,
    SurfaceOptions, TextBuffer, TextDraw, TextRasterOptions, TextStyle, TextSystem, Transform2D,
    wgpu,
};
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct Sample {
    caption: PreparedText,
    text: PreparedText,
    transform: Transform2D,
}
struct Scene {
    heading: PreparedText,
    footer: PreparedText,
    rows: Vec<[Sample; 2]>,
}
fn prepared(
    painter: &mut Painter,
    fonts: &mut TextSystem,
    content: &str,
    size: f32,
    dpi: f32,
    hinting: bool,
) -> Result<PreparedText, Box<dyn Error>> {
    let mut buffer = TextBuffer::new();
    buffer.set_text(
        content,
        TextStyle::new()
            .family("Source Sans 3")
            .font_size(size)
            .line_height(size * 1.5),
    )?;
    let layout = buffer.layout(fonts)?;
    Ok(painter.prepare_text(
        &layout,
        TextRasterOptions::new().raster_scale(dpi).hinting(hinting),
    )?)
}
fn sample(
    painter: &mut Painter,
    fonts: &mut TextSystem,
    caption: &str,
    content: &str,
    size: f32,
    raster_transform: (TextRasterOptions, Transform2D),
    dpi: f32,
) -> Result<Sample, Box<dyn Error>> {
    let (raster, transform) = raster_transform;
    let caption = prepared(painter, fonts, caption, 12., dpi, true)?;
    let text = prepared(
        painter,
        fonts,
        content,
        size,
        raster.raster_scale,
        raster.hinting,
    )?;
    Ok(Sample {
        caption,
        text,
        transform: Transform2D::scale(raster.raster_scale, raster.raster_scale).then(transform),
    })
}
fn prepare_scene(
    painter: &mut Painter,
    fonts: &mut TextSystem,
    dpi: f32,
    zoom: f32,
) -> Result<Scene, Box<dyn Error>> {
    let mut rows = Vec::new();
    for size in [9., 12., 16.] {
        rows.push([
            sample(
                painter,
                fonts,
                &format!("{size} logical units: whole pixel origin"),
                "AV office e\u{301} 123",
                size,
                (
                    TextRasterOptions::new().raster_scale(dpi),
                    Transform2D::IDENTITY,
                ),
                dpi,
            )?,
            sample(
                painter,
                fonts,
                "Same raster: +0.5 physical pixel origin",
                "AV office e\u{301} 123",
                size,
                (
                    TextRasterOptions::new().raster_scale(dpi),
                    Transform2D::translation(0.5, 0.5),
                ),
                dpi,
            )?,
        ]);
    }
    rows.push([
        sample(
            painter,
            fonts,
            "12 units: hinting enabled",
            "AV office e\u{301} 123",
            12.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::IDENTITY,
            ),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "12 units: hinting disabled",
            "AV office e\u{301} 123",
            12.,
            (
                TextRasterOptions::new().raster_scale(dpi).hinting(false),
                Transform2D::IDENTITY,
            ),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "1x raster scaled to window DPI",
            "AV office e\u{301}",
            16.,
            (TextRasterOptions::new(), Transform2D::scale(dpi, dpi)),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            &format!("Prepared at window DPI ({dpi:.2}x)"),
            "AV office e\u{301}",
            16.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::IDENTITY,
            ),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            &format!("Native raster magnified {zoom:.0}x"),
            "AV e\u{301}",
            14.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::scale(zoom, zoom),
            ),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "Prepared at the final physical size",
            "AV e\u{301}",
            14.,
            (
                TextRasterOptions::new().raster_scale(dpi * zoom),
                Transform2D::IDENTITY,
            ),
            dpi,
        )?,
    ]);
    let rotation = Transform2D::rotation(12_f32.to_radians());
    rows.push([
        sample(
            painter,
            fonts,
            "Native raster, rotated 12 degrees",
            "AV office",
            20.,
            (TextRasterOptions::new().raster_scale(dpi), rotation),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "2x denser raster, downscaled and rotated",
            "AV office",
            20.,
            (
                TextRasterOptions::new().raster_scale(dpi * 2.),
                Transform2D::scale(0.5, 0.5).then(rotation),
            ),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "Advanced shaping and fallback",
            "office e\u{301} العربية 123",
            20.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::IDENTITY,
            ),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "Same text: fractional physical placement",
            "office e\u{301} العربية 123",
            20.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::translation(0.5, 0.5),
            ),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "Coverage + COLR layers + PNG bitmap",
            "M😀M😁M",
            20.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::IDENTITY,
            ),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "Same prepared color images magnified 3x",
            "M😀M😁M",
            20.,
            (
                TextRasterOptions::new().raster_scale(dpi),
                Transform2D::scale(3., 3.),
            ),
            dpi,
        )?,
    ]);
    Ok(Scene {
        heading: prepared(
            painter,
            fonts,
            "Coverage text quality: Space = MSAA, Z = zoom",
            22.,
            dpi,
            true,
        )?,
        footer: prepared(
            painter,
            fonts,
            "DPI/raster changes prepare explicitly. Fractional movement filters existing images. Each cell clips its own text.",
            11.,
            dpi,
            true,
        )?,
        rows,
    })
}
fn draw_scene(
    painter: &mut Painter,
    scene: &Scene,
    pass: &mut RenderPass<'_>,
    size: [u32; 2],
    dpi: f32,
) -> Result<(), Box<dyn Error>> {
    let mut paint = painter.begin(pass)?;
    paint.draw_text(
        &scene.heading,
        TextDraw::new([24., 12.]).transform(Transform2D::scale(dpi, dpi)),
    )?;
    for (row, samples) in scene.rows.iter().enumerate() {
        for (column, sample) in samples.iter().enumerate() {
            let x = (24. + column as f32 * 548.) * dpi;
            let y = (64. + row as f32 * 94.) * dpi;
            if x >= size[0] as f32 || y >= size[1] as f32 {
                continue;
            }
            let width = (524. * dpi).min(size[0] as f32 - x);
            let height = (88. * dpi).min(size[1] as f32 - y);
            paint
                .pass()
                .set_scissor_rect(x as u32, y as u32, width as u32, height as u32)?;
            paint.fill_rect(Rect::new(x, y, width, height), [0.035, 0.035, 0.035, 1.])?;
            paint.draw_text(
                &sample.caption,
                TextDraw::new([x / dpi + 8., y / dpi + 4.])
                    .transform(Transform2D::scale(dpi, dpi))
                    .color([0.4, 0.65, 0.9, 1.]),
            )?;
            paint.draw_text(
                &sample.text,
                TextDraw::default().color([0.9, 0.9, 0.9, 1.]).transform(
                    sample
                        .transform
                        .then(Transform2D::translation(x + 8. * dpi, y + 20. * dpi)),
                ),
            )?;
        }
    }
    paint.pass().set_scissor_rect(0, 0, size[0], size[1])?;
    paint.draw_text(
        &scene.footer,
        TextDraw::new([24., 922.])
            .transform(Transform2D::scale(dpi, dpi))
            .color([0.6, 0.6, 0.6, 1.]),
    )?;
    Ok(())
}
struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    fonts: TextSystem,
    painter: Painter,
    scene: Option<Scene>,
    zoom: f32,
}
impl State {
    fn new(events: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let window = Arc::new(
            events.create_window(
                Window::default_attributes()
                    .with_title("Astrelis text quality - Space: MSAA, Z: zoom")
                    .with_inner_size(LogicalSize::new(1120., 960.)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut fonts = TextSystem::new();
        fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
        fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
        fonts.load_font(include_bytes!("../tests/fonts/TestColor.ttf"))?;
        let painter = Painter::new(&graphics);
        let mut state = Self {
            window,
            graphics,
            target,
            fonts,
            painter,
            scene: None,
            zoom: 4.,
        };
        state.prepare()?;
        state.window.request_redraw();
        Ok(state)
    }
    fn prepare(&mut self) -> Result<(), Box<dyn Error>> {
        self.scene = None;
        if self.window.inner_size().width == 0 || self.window.inner_size().height == 0 {
            return Ok(());
        }
        self.scene = Some(prepare_scene(
            &mut self.painter,
            &mut self.fonts,
            self.window.scale_factor() as f32,
            self.zoom,
        )?);
        self.painter.prepare_for_target(&self.target)?;
        Ok(())
    }
    fn resize(&mut self) -> Result<(), Box<dyn Error>> {
        let size = self.window.inner_size();
        self.target.resize(size.width, size.height)?;
        self.prepare()
    }
    fn recreate_surface(&mut self) -> Result<(), Box<dyn Error>> {
        let size = self.window.inner_size();
        let samples = self.target.sample_count();
        self.target = self.graphics.create_surface(
            self.window.clone(),
            SurfaceOptions::new(size.width, size.height).sample_count(samples),
        )?;
        self.prepare()?;
        self.window.request_redraw();
        Ok(())
    }
    fn toggle_msaa(&mut self) -> Result<(), Box<dyn Error>> {
        if self.target.supported_sample_counts().contains(&4) {
            self.target
                .set_sample_count(if self.target.sample_count() == 1 {
                    4
                } else {
                    1
                })?;
            self.painter.prepare_for_target(&self.target)?;
        }
        Ok(())
    }
    fn draw(&mut self) -> Result<(), Box<dyn Error>> {
        let mut frame = self.target.begin_frame()?;
        let size = frame.size();
        {
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.015,
                    g: 0.015,
                    b: 0.015,
                    a: 1.,
                })
                .begin()?;
            if let Some(scene) = &self.scene {
                draw_scene(
                    &mut self.painter,
                    scene,
                    &mut pass,
                    size,
                    self.window.scale_factor() as f32,
                )?;
            }
        }
        frame.finish()?;
        Ok(())
    }
}
#[derive(Default)]
struct App {
    state: Option<State>,
    failure: Option<Box<dyn Error>>,
    retry_at: Option<Instant>,
    occluded: bool,
}
impl App {
    fn fail(&mut self, events: &ActiveEventLoop, error: Box<dyn Error>) {
        self.failure = Some(error);
        events.exit();
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.state.is_none() {
            match State::new(events) {
                Ok(s) => {
                    self.state = Some(s);
                    self.occluded = false;
                }
                Err(e) => self.fail(events, e),
            }
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.state = None;
        self.retry_at = None;
    }
    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if events.exiting() {
            return;
        }
        let Some(state) = &mut self.state else {
            return;
        };
        if id != state.window.id() {
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested => {
                events.exit();
                Ok(())
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                let result = state.resize();
                state.window.request_redraw();
                result
            }
            WindowEvent::Occluded(value) => {
                self.occluded = value;
                self.retry_at = None;
                if !value {
                    state.window.request_redraw();
                }
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Space) =>
            {
                let result = state.toggle_msaa();
                state.window.request_redraw();
                result
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && !event.repeat
                    && matches!(&event.logical_key, Key::Character(key) if key.eq_ignore_ascii_case("z")) =>
            {
                state.zoom = if state.zoom == 4. { 2. } else { 4. };
                let result = state.prepare();
                state.window.request_redraw();
                result
            }
            WindowEvent::RedrawRequested if !self.occluded => match state.draw() {
                Err(e) if e.downcast_ref::<FrameError>() == Some(&FrameError::Retry) => {
                    self.retry_at = Some(Instant::now() + Duration::from_millis(16));
                    Ok(())
                }
                Err(e) if e.downcast_ref::<FrameError>() == Some(&FrameError::Suspended) => {
                    self.retry_at = None;
                    Ok(())
                }
                Err(e) if e.downcast_ref::<FrameError>() == Some(&FrameError::SurfaceLost) => {
                    state.recreate_surface()
                }
                result => result,
            },
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.fail(events, e);
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        if self
            .retry_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.retry_at = None;
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
        }
        events.set_control_flow(
            self.retry_at
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let events = EventLoop::new()?;
    let mut app = App::default();
    events.run_app(&mut app)?;
    if let Some(e) = app.failure {
        Err(e)
    } else {
        Ok(())
    }
}
