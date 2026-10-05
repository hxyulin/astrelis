//! Standalone coverage/color text with DPI-aware preparation, reflow, clipping, and MSAA.
//! Space toggles MSAA. Copy this file and its licensed fonts into your application.
use astrelis::{
    FrameError, GraphicsContext, PreparedText, RenderTarget, SurfaceOptions, TextBuffer, TextDraw,
    TextRasterOptions, TextRenderer, TextStyle, TextSystem, wgpu,
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
    renderer: TextRenderer,
    prepared: Option<PreparedText>,
}
impl State {
    fn new(events: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let window = Arc::new(
            events.create_window(
                Window::default_attributes()
                    .with_title("Astrelis text - Space: MSAA")
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
        paragraph.set_text("Astrelis coverage and color text\n\nHello, office! AV kerning and combining marks: e\u{301}.\nالعربية — مرحبا بالعالم 123\nMixed fallback: Hello العربية 😀 😁\n\nResize to reflow. DPI is applied explicitly during preparation.\nSpace toggles MSAA; glyph edge coverage also works at 1x.\n\nThe two geometric color marks come from the original test font: one uses COLR layers, the other a PNG bitmap.",
            TextStyle::new().family("Source Sans 3").font_size(22.).line_height(32.))?;
        let renderer = TextRenderer::new(&graphics);
        let mut state = Self {
            window,
            graphics,
            target,
            fonts,
            paragraph,
            renderer,
            prepared: None,
        };
        state.prepare()?;
        state.window.request_redraw();
        Ok(state)
    }
    fn prepare(&mut self) -> Result<(), Box<dyn Error>> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.prepared = None;
            return Ok(());
        }
        let scale = self.window.scale_factor() as f32;
        self.paragraph
            .set_width(Some((size.width as f32 / scale - 48.).max(0.)))?;
        let layout = self.paragraph.layout(&mut self.fonts)?;
        // Drop the obsolete resource before preparing a replacement. Glyph cache
        // entries remain reusable; in-flight recordings retain completion leases.
        self.prepared = None;
        self.prepared = Some(
            self.renderer
                .prepare_text(&layout, TextRasterOptions::new().raster_scale(scale))?,
        );
        self.renderer.prepare_for_target(&self.target)?;
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
            self.renderer.prepare_for_target(&self.target)?;
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
                if let Some(text) = &self.prepared {
                    self.renderer.draw(
                        &mut pass,
                        text,
                        TextDraw::new([24., 24.])
                            .color([0.82, 0.9, 1., 1.])
                            .transform(astrelis::Transform2D::scale(scale, scale)),
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
