//! Standalone Painter with batched multilingual text preparation, scoped transforms, and ordered layers.
//! Space toggles MSAA. Copy this file and its licensed fonts into your application.
use astrelis::{
    FrameError, GraphicsContext, LineDraw, Mesh, MeshRenderer, Painter, PreparedText, Rect,
    RenderTarget, Stroke, SurfaceOptions, TextBuffer, TextDraw, TextRasterOptions, TextStyle,
    TextSystem, Transform2D, Vertex, wgpu,
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

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    fonts: TextSystem,
    paragraph: TextBuffer,
    caption: TextBuffer,
    painter: Painter,
    custom: MeshRenderer,
    marker: Mesh,
    prepared: Vec<PreparedText>,
}
impl State {
    fn new(events: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let window = Arc::new(
            events.create_window(
                Window::default_attributes()
                    .with_title("Astrelis Painter text - Space: MSAA")
                    .with_inner_size(LogicalSize::new(800., 480.)),
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
        let mut paragraph = TextBuffer::new();
        paragraph.set_text("Painter: retained coverage and color text\n\nHello, office! AV kerning and combining marks: e\u{301}.\nالعربية — مرحبا بالعالم 123\nMixed fallback: Hello العربية 😀 😁\n\nResize to reflow. Preparation stays outside painting.\nSpace toggles MSAA. Shapes and text preserve call order.\n\nThe two geometric color marks come from the original test font: one uses COLR layers, the other a PNG bitmap.",
            TextStyle::new().family("Source Sans 3").font_size(22.).line_height(32.))?;
        let mut caption = TextBuffer::new();
        caption.set_text(
            "Two layouts share geometry; each draws with its own placement and color.",
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(12.)
                .line_height(18.),
        )?;
        let painter = Painter::new(&graphics);
        let custom = MeshRenderer::new(&graphics);
        let marker = graphics.create_mesh(
            &[
                Vertex::new([0.7, -0.75, 0.], [0.2, 0.7, 1., 0.8]),
                Vertex::new([0.9, -0.75, 0.], [0.2, 0.7, 1., 0.8]),
                Vertex::new([0.8, -0.55, 0.], [0.2, 0.7, 1., 0.8]),
            ],
            &[0, 1, 2],
        )?;
        let mut state = Self {
            window,
            graphics,
            target,
            fonts,
            paragraph,
            caption,
            painter,
            custom,
            marker,
            prepared: Vec::new(),
        };
        state.prepare()?;
        state.window.request_redraw();
        Ok(state)
    }
    fn prepare(&mut self) -> Result<(), Box<dyn Error>> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.prepared.clear();
            return Ok(());
        }
        let scale = self.window.scale_factor() as f32;
        self.paragraph
            .set_width(Some((size.width as f32 / scale - 48.).max(0.)))?;
        let layout = self.paragraph.layout(&mut self.fonts)?;
        // Drop the obsolete resource before preparing a replacement. Glyph cache
        // entries remain reusable; in-flight recordings retain completion leases.
        self.prepared.clear();
        self.caption
            .set_width(Some((size.width as f32 / scale - 96.).max(0.)))?;
        let caption = self.caption.layout(&mut self.fonts)?;
        // Returned resources share geometry storage, but can be drawn independently.
        self.prepared = self.painter.prepare_texts(
            [layout, caption],
            TextRasterOptions::new().scale_factor(scale),
        )?;
        self.painter.prepare_for_target(&self.target)?;
        self.custom.prepare_for_target(&self.target)?;
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
            self.custom.prepare_for_target(&self.target)?;
        }
        Ok(())
    }
    fn draw(&mut self) -> Result<(), Box<dyn Error>> {
        let scale = self.window.scale_factor() as f32;
        let mut frame = self.target.begin_frame()?;
        {
            let [w, h] = frame.size();
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.015,
                    g: 0.025,
                    b: 0.04,
                    a: 1.,
                })
                .begin()?;
            let margin = (16. * scale) as u32;
            if w > margin * 2 && h > margin * 2 {
                pass.set_scissor_rect(margin, margin, w - margin * 2, h - margin * 2)?;
                if let Some(text) = self.prepared.first() {
                    let mut paint = self.painter.begin(&mut pass)?;
                    let panel = Rect::new(
                        20. * scale,
                        20. * scale,
                        w as f32 - 40. * scale,
                        h as f32 - 40. * scale,
                    );
                    paint.fill_rounded_rect(panel, 12. * scale, [0.025, 0.055, 0.09, 1.])?;
                    {
                        // Prepared glyphs are already physical pixels. Translate by
                        // a physical margin; do not apply the DPI scale again here.
                        let mut local = paint
                            .transformed(Transform2D::translation(24. * scale, 24. * scale))?;
                        local
                            .draw_text(text, TextDraw::new([0., 0.]).color([0.82, 0.9, 1., 1.]))?;
                        // A later primitive covers part of the text, with no flush.
                        local.draw_line(
                            LineDraw::new(
                                [0., 28. * scale],
                                [250. * scale, 28. * scale],
                                [0.2, 0.7, 1., 0.8],
                            )
                            .width(2. * scale),
                        )?;
                    }
                    if let Some(caption) = self.prepared.get(1) {
                        paint.draw_text(
                            caption,
                            TextDraw::new([44. * scale, h as f32 - 48. * scale])
                                .color([0.45, 0.7, 0.85, 1.]),
                        )?;
                    }
                    paint.stroke_rounded_rect(
                        panel,
                        12. * scale,
                        Stroke::new(scale).inside(),
                        [0.15, 0.4, 0.65, 1.],
                    )?;
                    // This custom mesh uses its own clip-space geometry; Painter
                    // transforms do not affect explicit pass access.
                    self.custom.draw(paint.pass(), &self.marker)?;
                    paint.fill_ellipse(
                        Rect::new(28. * scale, h as f32 - 40. * scale, 8. * scale, 8. * scale),
                        [0.2, 0.7, 1., 1.],
                    )?;
                }
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
