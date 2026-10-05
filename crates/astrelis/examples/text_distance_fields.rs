//! Standalone distance-field fill comparisons; Space toggles MSAA, Z toggles 2x/4x zoom.
//! Copy this file and the three bundled fonts (retaining their licenses).
//! Coverage references and MTSDF preparation are explicit; drawing does not generate fields.
use astrelis::{
    FrameError, GraphicsContext, MtsdfOptions, Painter, PreparedText, Rect, RenderPass,
    RenderTarget, SurfaceOptions, TextBuffer, TextDraw, TextPreparation, TextRasterOptions,
    TextStyle, TextSystem, Transform2D, wgpu,
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
    style: TextStyle,
    preparation: impl Into<TextPreparation>,
) -> Result<PreparedText, Box<dyn Error>> {
    let mut buffer = TextBuffer::new();
    buffer.set_text(content, style)?;
    Ok(painter.prepare_text(buffer.layout(fonts)?.as_ref(), preparation)?)
}
fn sample(
    painter: &mut Painter,
    fonts: &mut TextSystem,
    caption: &str,
    content: &str,
    style: TextStyle,
    settings: (TextPreparation, Transform2D),
    dpi: f32,
) -> Result<Sample, Box<dyn Error>> {
    Ok(Sample {
        caption: prepared(
            painter,
            fonts,
            caption,
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(12.)
                .line_height(18.),
            TextRasterOptions::new().scale_factor(dpi),
        )?,
        text: prepared(painter, fonts, content, style, settings.0)?,
        transform: settings.1,
    })
}
fn prepare_scene(
    painter: &mut Painter,
    fonts: &mut TextSystem,
    dpi: f32,
    zoom: f32,
) -> Result<Scene, Box<dyn Error>> {
    let style = |size| {
        TextStyle::new()
            .family("Source Sans 3")
            .font_size(size)
            .line_height(size * 1.25)
    };
    let coverage = |scale| TextRasterOptions::new().scale_factor(scale).into();
    let field = |density, range| {
        MtsdfOptions::new()
            .pixels_per_em(density)
            .range_em(range)
            .scale_factor(dpi)
            .into()
    };
    let identity = Transform2D::IDENTITY;
    let mut rows = Vec::new();
    for size in [9., 12., 16.] {
        rows.push([
            sample(
                painter,
                fonts,
                &format!("{size} units: hinted native coverage"),
                "AV office e\u{301} 123",
                style(size),
                (coverage(dpi), identity),
                dpi,
            )?,
            sample(
                painter,
                fonts,
                "64 texels/EM MTSDF: fractional +0.5px origin",
                "AV office e\u{301} 123",
                style(size),
                (field(64, 0.25), Transform2D::translation(0.5, 0.5)),
                dpi,
            )?,
        ]);
    }
    let magnify = Transform2D::scale(zoom, zoom);
    rows.push([
        sample(
            painter,
            fonts,
            &format!("Final-size coverage reference ({zoom}x)"),
            "AV office",
            style(13.),
            (coverage(dpi * zoom), identity),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "64 texels/EM field magnified, no regeneration",
            "AV office",
            style(13.),
            (field(64, 0.25), magnify),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "32 texels/EM, full range 0.25 EM",
            "AV office",
            style(13.),
            (field(32, 0.25), magnify),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "96 texels/EM, full range 0.25 EM",
            "AV office",
            style(13.),
            (field(96, 0.25), magnify),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "64 texels/EM, full range 0.125 EM",
            "AV office",
            style(13.),
            (field(64, 0.125), magnify),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "64 texels/EM, full range 0.5 EM",
            "AV office",
            style(13.),
            (field(64, 0.5), magnify),
            dpi,
        )?,
    ]);
    let rotation = Transform2D::rotation(12_f32.to_radians());
    let nonuniform = Transform2D::scale(1.6, 0.8).then(Transform2D::translation(0.5, 0.5));
    rows.push([
        sample(
            painter,
            fonts,
            "Native coverage: rotate 12 degrees",
            "AV office العربية",
            style(24.),
            (coverage(dpi), rotation),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "MTSDF: rotate 12 degrees",
            "AV office العربية",
            style(24.),
            (field(64, 0.25), rotation),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "Native coverage: scale X 1.6 / Y 0.8 + 0.5px",
            "office e\u{301} العربية",
            style(24.),
            (coverage(dpi), nonuniform),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "MTSDF: scale X 1.6 / Y 0.8 + 0.5px",
            "office e\u{301} العربية",
            style(24.),
            (field(64, 0.25), nonuniform),
            dpi,
        )?,
    ]);
    rows.push([
        sample(
            painter,
            fonts,
            "Coverage + COLR + PNG intrinsic artwork",
            "M😀M😁M",
            style(20.),
            (coverage(dpi), identity),
            dpi,
        )?,
        sample(
            painter,
            fonts,
            "MTSDF + same size-dependent color artwork",
            "M😀M😁M",
            style(20.),
            (field(64, 0.25), identity),
            dpi,
        )?,
    ]);
    Ok(Scene {
        heading: prepared(
            painter,
            fonts,
            "Distance-field fill: Space = MSAA, Z = zoom",
            style(22.),
            coverage(dpi),
        )?,
        footer: prepared(
            painter,
            fonts,
            "Explicit preparation; coverage for small hinted text, MTSDF for scalable outlines. Fill only; color artwork retains its image path.",
            style(11.),
            coverage(dpi),
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
    paint.draw_text(&scene.heading, TextDraw::new([24. * dpi, 12. * dpi]))?;
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
                TextDraw::new([x + 8. * dpi, y + 4. * dpi]).color([0.4, 0.65, 0.9, 1.]),
            )?;
            paint.draw_text(
                &sample.text,
                TextDraw::default().color([0.9, 0.9, 0.9, 1.]).transform_2d(
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
        TextDraw::new([24. * dpi, 922. * dpi]).color([0.6, 0.6, 0.6, 1.]),
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
                    .with_title("Astrelis distance fields - Space: MSAA, Z: zoom")
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
