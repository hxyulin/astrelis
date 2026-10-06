//! Reusable solid, linear, and radial brushes through Painter.
//! Top: linear color, hard stops, a gradient heart. Middle: circle/ellipse radial,
//! and transparency. Bottom: repeat, reflect, and an independent gradient line.
//! A toggles edge coverage; Space toggles supported target MSAA.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Brush, BrushOptions, EdgeAntialiasing, Error, FrameError, GradientSpread, GradientStop,
    GraphicsContext, LineCap, LineDraw, Painter, Path, PathDraw, PathOptions, PreparedPath, Rect,
    RenderTarget, ShapeDraw, SurfaceOptions, Transform2D, wgpu,
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
    heart: PreparedPath,
    brushes: Vec<Brush>,
    coverage: bool,
}

fn prepare_heart(painter: &mut Painter) -> Result<PreparedPath, Error> {
    let mut b = Path::builder();
    b.move_to([50., 90.])
        .cubic_to([30., 75.], [0., 55.], [5., 30.])
        .cubic_to([10., 0.], [40., 5.], [50., 25.])
        .cubic_to([60., 5.], [90., 0.], [95., 30.])
        .cubic_to([100., 55.], [70., 75.], [50., 90.])
        .close();
    painter.prepare_path(&b.build()?, PathOptions::new())
}
fn prepare_brushes(graphics: &GraphicsContext) -> Result<Vec<Brush>, Error> {
    let colors = [
        GradientStop::new(0., [0.1, 0.7, 1., 1.]),
        GradientStop::new(0.45, [0.8, 0.15, 0.7, 1.]),
        GradientStop::new(1., [1., 0.5, 0.05, 1.]),
    ];
    let hard = [
        colors[0],
        GradientStop::new(0.5, colors[0].color),
        GradientStop::new(0.5, colors[2].color),
        colors[2],
    ];
    let fade = [
        GradientStop::new(0., [1., 1., 1., 1.]),
        GradientStop::new(1., [0., 0., 1., 0.]),
    ];
    [
        BrushOptions::linear([5., 5.], [95., 95.], &colors),
        BrushOptions::linear([5., 0.], [95., 0.], &hard),
        BrushOptions::linear([0., 0.], [0., 100.], &colors),
        BrushOptions::radial([50., 50.], 45., &colors),
        BrushOptions::radial([0., 0.], 1., &colors)
            .transform(Transform2D::scale(45., 22.).then(Transform2D::translation(50., 50.))),
        BrushOptions::linear([5., 0.], [95., 0.], &fade),
        BrushOptions::linear([5., 0.], [35., 0.], &colors).spread(GradientSpread::Repeat),
        BrushOptions::linear([5., 0.], [35., 0.], &colors).spread(GradientSpread::Reflect),
        BrushOptions::linear([5., 0.], [95., 0.], &colors),
    ]
    .into_iter()
    .map(|options| graphics.create_brush(options))
    .collect()
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — brushes (A: coverage, Space: MSAA)")
                    .with_inner_size(PhysicalSize::new(840, 840)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut painter = Painter::new(&graphics);
        let heart = prepare_heart(&mut painter)?;
        let brushes = prepare_brushes(&graphics)?;
        painter.prepare_for_target(&target)?;
        painter.prepare_brush(&target.render_format())?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            painter,
            heart,
            brushes,
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
            self.painter.prepare_brush(&self.target.render_format())?;
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
            for (i, brush) in state.brushes.iter().enumerate() {
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
                let transform = Transform2D::scale(scale, scale)
                    .then(Transform2D::translation(x + 12. * scale, y + 12. * scale));
                let aa = if state.coverage {
                    EdgeAntialiasing::Coverage
                } else {
                    EdgeAntialiasing::None
                };
                let mut tile = paint.transformed(transform)?;
                if i == 2 {
                    tile.draw_path_with_brush(
                        &state.heart,
                        brush,
                        PathDraw::default().antialiasing(aa),
                    )?;
                } else if i == 8 {
                    tile.draw_line_with_brush(
                        brush,
                        LineDraw::new([5., 75.], [95., 25.], [1.; 4])
                            .width(14.)
                            .cap(LineCap::Round)
                            .antialiasing(aa),
                    )?;
                } else {
                    // A checkerboard makes transparent stop interpolation visible.
                    if i == 5 {
                        for row in 0..8 {
                            for column in 0..8 {
                                let shade = if (row + column) % 2 == 0 { 0.1 } else { 0.3 };
                                tile.fill_rect(
                                    Rect::new(
                                        5. + column as f32 * 11.25,
                                        5. + row as f32 * 11.25,
                                        11.25,
                                        11.25,
                                    ),
                                    [shade, shade, shade, 1.],
                                )?;
                            }
                        }
                    }
                    let draw = if i == 3 || i == 4 {
                        ShapeDraw::ellipse(Rect::new(5., 5., 90., 90.), [1.; 4])
                    } else {
                        ShapeDraw::rounded_rect(Rect::new(5., 5., 90., 90.), 12., [1.; 4])
                    };
                    tile.draw_shape_with_brush(brush, draw.antialiasing(aa))?;
                }
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
        state.painter.prepare_brush(&state.target.render_format())?;
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
