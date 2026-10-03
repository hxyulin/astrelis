use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{Error, FrameStatus, GraphicsContext, Mesh, RenderTarget, Renderer, wgpu};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

type BuildMeshes = fn(&GraphicsContext) -> Result<Vec<Mesh>, Error>;

pub fn run(titles: &[&'static str], build: BuildMeshes) -> Result<(), Box<dyn StdError>> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        titles: titles.to_vec(),
        build,
        state: None,
        failure: None,
        smoke: std::env::args().any(|arg| arg == "--smoke"),
        smoke_deadline: None,
    };
    event_loop.run_app(&mut app)?;
    match app.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

struct WindowState {
    window: Arc<Window>,
    target: RenderTarget<'static>,
    retry_at: Option<Instant>,
    frames: u32,
}

struct State {
    graphics: GraphicsContext,
    renderer: Renderer,
    meshes: Vec<Mesh>,
    windows: Vec<WindowState>,
}

fn check_suspension(renderer: &mut Renderer, target: &mut RenderTarget<'_>) -> Result<(), Error> {
    let [width, height] = target.size();
    target.resize(0, 0)?;
    assert!(matches!(
        renderer.render(target, wgpu::Color::BLACK, &[])?,
        FrameStatus::Suspended,
    ));
    target.resize(width, height)
}

struct App {
    titles: Vec<&'static str>,
    build: BuildMeshes,
    state: Option<State>,
    failure: Option<Box<dyn StdError>>,
    smoke: bool,
    smoke_deadline: Option<Instant>,
}

impl App {
    fn initialize(&self, event_loop: &ActiveEventLoop) -> Result<State, Box<dyn StdError>> {
        let mut graphics = None;
        let mut windows = Vec::new();
        for (index, title) in self.titles.iter().enumerate() {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(*title)
                        .with_inner_size(PhysicalSize::new(800, 600))
                        .with_position(PhysicalPosition::new(80 + index as i32 * 380, 80)),
                )?,
            );
            let size = window.inner_size();
            let target = match &graphics {
                Some(graphics) => GraphicsContext::create_surface(
                    graphics,
                    window.clone(),
                    size.width,
                    size.height,
                )?,
                None => {
                    let (context, target) = pollster::block_on(GraphicsContext::with_surface(
                        window.clone(),
                        size.width,
                        size.height,
                    ))?;
                    graphics = Some(context);
                    target
                }
            };
            window.request_redraw();
            windows.push(WindowState {
                window,
                target,
                retry_at: None,
                frames: 0,
            });
        }
        let graphics = graphics.ok_or_else(|| std::io::Error::other("no example windows"))?;
        let renderer = Renderer::new(&graphics);
        let meshes = (self.build)(&graphics)?;
        eprintln!(
            "Rendering {} window(s) on {} ({:?}) with one device",
            windows.len(),
            graphics.adapter().get_info().name,
            graphics.adapter().get_info().backend,
        );
        Ok(State {
            graphics,
            renderer,
            meshes,
            windows,
        })
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn StdError>>) {
        self.failure = Some(error.into());
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            match self.initialize(event_loop) {
                Ok(state) => {
                    self.state = Some(state);
                    if self.smoke {
                        self.smoke_deadline = Some(Instant::now() + Duration::from_secs(10));
                    }
                }
                Err(error) => self.fail(event_loop, error),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };
        let Some(window) = state
            .windows
            .iter_mut()
            .find(|window| window.window.id() == id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Err(error) = window.target.resize(size.width, size.height) {
                    self.fail(event_loop, error);
                    return;
                }
                window.window.request_redraw();
            }
            WindowEvent::Occluded(false) => window.window.request_redraw(),
            WindowEvent::RedrawRequested => {
                let meshes: Vec<_> = state.meshes.iter().collect();
                match state.renderer.render(
                    &mut window.target,
                    wgpu::Color {
                        r: 0.025,
                        g: 0.035,
                        b: 0.06,
                        a: 1.0,
                    },
                    &meshes,
                ) {
                    Ok(FrameStatus::Presented(_)) => {
                        window.retry_at = None;
                        window.frames += 1;
                        if self.smoke {
                            if window.frames == 1 {
                                if let Err(error) =
                                    check_suspension(&mut state.renderer, &mut window.target)
                                {
                                    self.fail(event_loop, error);
                                    return;
                                }
                                if let Some(size) = window
                                    .window
                                    .request_inner_size(PhysicalSize::new(640, 480))
                                    && let Err(error) =
                                        window.target.resize(size.width, size.height)
                                {
                                    self.fail(event_loop, error);
                                    return;
                                }
                            }
                            if window.frames < 3 || window.target.size() == [640, 480] {
                                window.window.request_redraw();
                            }
                            if state.windows.iter().all(|window| {
                                window.frames >= 3 && window.target.size() == [640, 480]
                            }) {
                                eprintln!(
                                    "Smoke passed: all {} windows presented at least three frames and resized to 640x480",
                                    state.windows.len()
                                );
                                event_loop.exit();
                            }
                        }
                    }
                    Ok(FrameStatus::Retry) => {
                        window.retry_at = Some(Instant::now() + Duration::from_millis(16));
                    }
                    Ok(FrameStatus::Suspended) => window.retry_at = None,
                    Err(Error::SurfaceLost) => {
                        let size = window.window.inner_size();
                        match state.graphics.create_surface(
                            window.window.clone(),
                            size.width,
                            size.height,
                        ) {
                            Ok(target) => {
                                window.target = target;
                                window.window.request_redraw();
                            }
                            Err(error) => self.fail(event_loop, error),
                        }
                    }
                    Err(error) => self.fail(event_loop, error),
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.smoke_deadline.is_some_and(|deadline| now >= deadline) {
            self.fail(
                event_loop,
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "smoke run did not complete presentation and resize within 10 seconds",
                ),
            );
            return;
        }
        let mut wake_at = self.smoke_deadline;
        if let Some(state) = &mut self.state {
            for window in &mut state.windows {
                if window.retry_at.is_some_and(|deadline| now >= deadline) {
                    window.retry_at = None;
                    window.window.request_redraw();
                }
                wake_at = wake_at.into_iter().chain(window.retry_at).min();
            }
        }
        event_loop.set_control_flow(wake_at.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}
