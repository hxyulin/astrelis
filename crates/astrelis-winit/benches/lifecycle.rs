//! Matched native prepared-frame measurement, run each variant in a fresh process.
//! cargo bench -p astrelis-winit --bench lifecycle -- --runner
//! cargo bench -p astrelis-winit --bench lifecycle -- --direct
//! CSV stdout; a real 128x128-physical window is shown. No GPU completion waits.
use astrelis_winit::{
    AppContext, Handler, PrepareAction, RedrawMode, Runner, SurfaceSettings, WindowContext,
    WindowInfo,
    astrelis::{Frame, FrameError, Mesh, MeshRenderer, Vertex},
    winit::{
        application::ApplicationHandler,
        dpi::PhysicalSize,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    },
};
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
const WARMUP: usize = 32;
const SAMPLES: usize = 320;
struct Bench {
    window: Option<WindowId>,
    native: Option<Arc<Window>>,
    context: Option<WindowContext>,
    mesh: Option<Mesh>,
    renderer: Option<MeshRenderer>,
    dispatch: Instant,
    prepared: Instant,
    record: Instant,
    recorded: Instant,
    frames: usize,
    samples: Vec<[Duration; 5]>,
    failure: Option<Box<dyn Error>>,
    retry: Option<Instant>,
}
impl Bench {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            window: None,
            native: None,
            context: None,
            mesh: None,
            renderer: None,
            dispatch: now,
            prepared: now,
            record: now,
            recorded: now,
            frames: 0,
            samples: Vec::with_capacity(SAMPLES),
            failure: None,
            retry: None,
        }
    }
    fn attributes() -> astrelis_winit::winit::window::WindowAttributes {
        Window::default_attributes()
            .with_window_level(astrelis_winit::winit::window::WindowLevel::AlwaysOnTop)
            .with_title("Astrelis lifecycle benchmark")
            .with_inner_size(PhysicalSize::new(128, 128))
    }
    fn resources(
        &mut self,
        graphics: &astrelis_winit::astrelis::GraphicsContext,
    ) -> Result<(), Box<dyn Error>> {
        eprintln!("adapter={:?}", graphics.adapter().get_info());
        self.renderer = Some(MeshRenderer::new(graphics));
        self.mesh = Some(graphics.create_mesh(
            &[
                Vertex::new([0., 0.7, 0.], [1., 0., 0., 1.]),
                Vertex::new([-0.7, -0.7, 0.], [0., 1., 0., 1.]),
                Vertex::new([0.7, -0.7, 0.], [0., 0., 1., 1.]),
            ],
            &[0, 1, 2],
        )?);
        Ok(())
    }
    fn recording(
        renderer: &mut MeshRenderer,
        mesh: &Mesh,
        frame: &mut Frame<'_, 'static>,
    ) -> Result<(Instant, Instant), Box<dyn Error>> {
        let record = Instant::now();
        {
            let mut pass = frame.render_pass().begin()?;
            let mut draws = renderer.bind(&mut pass, mesh)?;
            for _ in 0..100 {
                draws.draw();
            }
        }
        Ok((record, Instant::now()))
    }
    fn complete(&mut self) -> bool {
        let submitted = Instant::now();
        if self.frames >= WARMUP {
            self.samples.push([
                self.prepared.duration_since(self.dispatch),
                self.record.duration_since(self.prepared),
                self.recorded.duration_since(self.record),
                submitted.duration_since(self.recorded),
                submitted.duration_since(self.dispatch),
            ]);
        }
        self.frames += 1;
        self.samples.len() == SAMPLES
    }
    fn report(&self, variant: &str) {
        println!("variant,stage,samples,median_us,p95_us");
        for (i, name) in [
            "dispatch_prepare",
            "acquire",
            "record",
            "finish_notify",
            "total",
        ]
        .into_iter()
        .enumerate()
        {
            let mut values: Vec<_> = self
                .samples
                .iter()
                .map(|s| s[i].as_secs_f64() * 1e6)
                .collect();
            values.sort_by(f64::total_cmp);
            println!(
                "{variant},{name},{},{:.3},{:.3}",
                values.len(),
                values[values.len() / 2],
                values[(values.len() * 95).div_ceil(100) - 1]
            );
        }
    }
}
impl Handler for Bench {
    type Message = ();
    type Error = Box<dyn Error>;
    fn resumed(&mut self, cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        if self.window.is_none() {
            let id = cx.create_window(Self::attributes(), SurfaceSettings::new())?;
            self.window = Some(id);
            cx.set_redraw_mode(id, RedrawMode::Continuous)?;
        }
        Ok(())
    }
    fn window_created(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        self.resources(cx.window(id).unwrap().graphics())
    }
    fn window_event(
        &mut self,
        _cx: &mut AppContext<'_, ()>,
        _id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::RedrawRequested) {
            self.dispatch = Instant::now();
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<PrepareAction, Self::Error> {
        self.renderer
            .as_mut()
            .unwrap()
            .prepare(cx.window(id).unwrap().render_format().unwrap())?;
        self.prepared = Instant::now();
        Ok(PrepareAction::Render)
    }
    fn render(
        &mut self,
        _window: WindowInfo<'_>,
        frame: &mut Frame<'_, 'static>,
    ) -> Result<(), Self::Error> {
        (self.record, self.recorded) = Self::recording(
            self.renderer.as_mut().unwrap(),
            self.mesh.as_ref().unwrap(),
            frame,
        )?;
        Ok(())
    }
    fn submitted(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        _id: WindowId,
        _submission: astrelis_winit::astrelis::wgpu::SubmissionIndex,
    ) -> Result<(), Self::Error> {
        if self.complete() {
            cx.exit();
        }
        Ok(())
    }
}
impl ApplicationHandler for Bench {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let window = Arc::new(event_loop.create_window(Self::attributes())?);
            let context =
                pollster::block_on(WindowContext::new(window.clone(), SurfaceSettings::new()))?;
            self.resources(context.graphics())?;
            window.request_redraw();
            self.native = Some(window);
            self.context = Some(context);
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(error);
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn Error>> {
            let Some(context) = &mut self.context else {
                return Ok(());
            };
            let update = context.process_event(&event)?;
            if update.needs_redraw() && context.can_present() {
                self.native.as_ref().unwrap().request_redraw();
            }
            if matches!(event, WindowEvent::CloseRequested) {
                event_loop.exit();
                return Ok(());
            }
            if !matches!(event, WindowEvent::RedrawRequested) || !context.can_present() {
                return Ok(());
            }
            self.dispatch = Instant::now();
            self.renderer
                .as_mut()
                .unwrap()
                .prepare(context.render_format().unwrap())?;
            self.prepared = Instant::now();
            let acquired = context.begin_frame();
            let rendered = match acquired {
                Ok(mut frame) => {
                    (self.record, self.recorded) = Self::recording(
                        self.renderer.as_mut().unwrap(),
                        self.mesh.as_ref().unwrap(),
                        &mut frame,
                    )?;
                    self.native.as_ref().unwrap().pre_present_notify();
                    frame.finish()?;
                    true
                }
                Err(FrameError::Retry) => {
                    self.retry = Some(Instant::now() + Duration::from_millis(16));
                    false
                }
                Err(FrameError::Suspended) => false,
                Err(error) => return Err(error.into()),
            };
            if rendered {
                if self.complete() {
                    event_loop.exit();
                } else {
                    self.native.as_ref().unwrap().request_redraw();
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(error);
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.retry.is_some_and(|at| at <= Instant::now()) {
            self.retry = None;
            self.native.as_ref().unwrap().request_redraw();
        }
        event_loop.set_control_flow(self.retry.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.context = None;
        self.native = None;
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let runner = match std::env::args()
        .find(|a| a == "--runner" || a == "--direct")
        .as_deref()
    {
        Some("--runner") => true,
        Some("--direct") => false,
        _ => return Err("use --runner or --direct (native desktop window)".into()),
    };
    let mut bench = Bench::new();
    if runner {
        Runner::new()?.run(&mut bench)?;
    } else {
        EventLoop::new()?.run_app(&mut bench)?;
        if let Some(error) = bench.failure.take() {
            return Err(error);
        }
    }
    if bench.samples.len() != SAMPLES {
        return Err("benchmark window closed before all samples".into());
    }
    bench.report(if runner { "runner" } else { "direct" });
    Ok(())
}
