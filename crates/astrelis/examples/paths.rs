//! Retained vector fills and connected strokes through Painter.
//! Top: nonzero fill, even-odd hole, cubic heart. Middle: miter, bevel, round joins.
//! Bottom: a self-intersecting fill, curved stroke, and a closed star outline.
//! A toggles shader coverage; Space toggles target MSAA when 4x is supported.
//! Resize changes placement only: paths are tessellated/uploaded once.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    EdgeAntialiasing, Error, FillRule, FrameError, GraphicsContext, LineCap, LineJoin, Painter,
    Path, PathDraw, PathOptions, PathStroke, PreparedPath, Rect, RenderTarget, SurfaceOptions,
    Transform2D, wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    painter: Painter,
    paths: Vec<PreparedPath>,
    coverage: bool,
}

fn prepare_paths(painter: &mut Painter) -> Result<Vec<PreparedPath>, Error> {
    let mut paths = Vec::new();
    let mut builder = Path::builder();
    // Same-winding contours show the difference between the fill rules.
    for (low, high) in [(5., 95.), (30., 70.)] {
        builder
            .move_to([low, low])
            .line_to([high, low])
            .line_to([high, high])
            .line_to([low, high])
            .close();
    }
    let nested = builder.build()?;
    paths.push(painter.prepare_path(&nested, PathOptions::new())?);
    paths.push(painter.prepare_path(&nested, PathOptions::new().fill_rule(FillRule::EvenOdd))?);
    let mut builder = Path::builder();
    builder
        .move_to([50., 90.])
        .cubic_to([30., 75.], [0., 55.], [5., 30.])
        .cubic_to([10., 0.], [40., 5.], [50., 25.])
        .cubic_to([60., 5.], [90., 0.], [95., 30.])
        .cubic_to([100., 55.], [70., 75.], [50., 90.])
        .close();
    paths.push(painter.prepare_path(&builder.build()?, PathOptions::new())?);
    let mut builder = Path::builder();
    builder
        .move_to([5., 75.])
        .line_to([25., 15.])
        .line_to([50., 75.])
        .line_to([75., 15.])
        .line_to([95., 75.]);
    let polyline = builder.build()?;
    for join in [LineJoin::Miter, LineJoin::Bevel, LineJoin::Round] {
        paths.push(painter.prepare_path(
            &polyline,
            PathOptions::new().stroke(PathStroke::new(10.).cap(LineCap::Round).join(join)),
        )?);
    }
    let mut builder = Path::builder();
    builder
        .move_to([5., 5.])
        .line_to([95., 95.])
        .line_to([5., 95.])
        .line_to([95., 5.])
        .close();
    paths.push(painter.prepare_path(
        &builder.build()?,
        PathOptions::new().fill_rule(FillRule::EvenOdd),
    )?);
    let mut builder = Path::builder();
    builder
        .move_to([5., 75.])
        .quadratic_to([25., 0.], [50., 50.])
        .cubic_to([70., 95.], [90., 0.], [95., 20.]);
    paths.push(painter.prepare_path(
        &builder.build()?,
        PathOptions::new().stroke(PathStroke::new(6.).cap(LineCap::Round)),
    )?);
    let mut builder = Path::builder();
    for i in 0..10 {
        let angle = i as f32 * std::f32::consts::PI / 5. - std::f32::consts::FRAC_PI_2;
        let radius = if i % 2 == 0 { 43. } else { 19. };
        let p = [50. + angle.cos() * radius, 50. + angle.sin() * radius];
        if i == 0 {
            builder.move_to(p);
        } else {
            builder.line_to(p);
        }
    }
    builder.close();
    paths.push(painter.prepare_path(
        &builder.build()?,
        PathOptions::new().stroke(PathStroke::new(4.).join(LineJoin::Round)),
    )?);
    Ok(paths)
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — paths (A: coverage, Space: MSAA)")
                    .with_inner_size(PhysicalSize::new(840, 840)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut painter = Painter::new(&graphics);
        let paths = prepare_paths(&mut painter)?;
        painter.prepare_for_target(&target)?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            painter,
            paths,
            coverage: true,
        })
    }
    fn toggle_msaa(&mut self) -> Result<(), Error> {
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
}

#[derive(Default)]
struct App {
    state: Option<State>,
    failure: Option<Box<dyn StdError>>,
    retry_at: Option<Instant>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn StdError>>) {
        self.failure = Some(error.into());
        event_loop.exit();
    }

    fn redraw(&mut self) -> Result<(), Box<dyn StdError>> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let mut frame = match state.target.begin_frame() {
            Ok(frame) => frame,
            Err(FrameError::Retry) => {
                self.retry_at = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            Err(FrameError::Suspended) => {
                self.retry_at = None;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        {
            let [width, height] = frame.size();
            let cell = (width.min(height) as f32 / 3.).max(1.);
            let scale = cell / 125.;
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.015,
                    g: 0.025,
                    b: 0.045,
                    a: 1.,
                })
                .begin()?;
            let mut paint = state.painter.begin(&mut pass)?;
            for (i, path) in state.paths.iter().enumerate() {
                let x = (i % 3) as f32 * cell;
                let y = (i / 3) as f32 * cell;
                let padding = (cell * 0.04).min(4.);
                paint.fill_rounded_rect(
                    Rect::new(
                        x + padding,
                        y + padding,
                        cell - padding * 2.,
                        cell - padding * 2.,
                    ),
                    12.,
                    [0.04, 0.065, 0.10, 1.],
                )?;
                paint.draw_path(
                    path,
                    PathDraw::new(match i % 3 {
                        0 => [0.10, 0.7, 0.9, 0.8],
                        1 => [0.25, 0.85, 0.55, 0.8],
                        _ => [0.95, 0.25, 0.45, 0.8],
                    })
                    .transform(
                        Transform2D::scale(scale, scale)
                            .then(Transform2D::translation(x + 12. * scale, y + 12. * scale)),
                    )
                    .antialiasing(if state.coverage {
                        EdgeAntialiasing::Coverage
                    } else {
                        EdgeAntialiasing::None
                    }),
                )?;
            }
        }
        frame.finish()?;
        self.retry_at = None;
        Ok(())
    }

    fn recreate_surface(&mut self) -> Result<(), Error> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let size = state.window.inner_size();
        let count = state.target.sample_count();
        state.target = state.graphics.create_surface(
            state.window.clone(),
            SurfaceOptions::new(size.width, size.height).sample_count(count),
        )?;
        state.painter.prepare_for_target(&state.target)?;
        state.window.request_redraw();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match State::new(event_loop) {
            Ok(state) => {
                self.state = Some(state);
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
        self.retry_at = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
        let Some(state) = &mut self.state else { return };
        if state.window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && !event.repeat =>
            {
                let result = match event.logical_key {
                    Key::Named(NamedKey::Space) => state.toggle_msaa(),
                    Key::Character(ref key) if key.eq_ignore_ascii_case("a") => {
                        state.coverage = !state.coverage;
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(error) = result {
                    self.fail(event_loop, error);
                    return;
                }
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let Err(error) = state.target.resize(size.width, size.height) {
                    self.fail(event_loop, error);
                    return;
                }
                state.window.request_redraw();
            }
            WindowEvent::Occluded(false) => state.window.request_redraw(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw() {
                    if error.downcast_ref::<FrameError>() == Some(&FrameError::SurfaceLost) {
                        if let Err(error) = self.recreate_surface() {
                            self.fail(event_loop, error)
                        }
                    } else {
                        self.fail(event_loop, error);
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.retry_at.is_some_and(|deadline| now >= deadline) {
            self.retry_at = None;
            if let Some(state) = &self.state {
                state.window.request_redraw()
            }
        }
        let wake_at = self.retry_at;
        event_loop.set_control_flow(wake_at.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

fn main() -> Result<(), Box<dyn StdError>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    match app.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
