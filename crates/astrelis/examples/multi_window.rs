//! Two windows with different sample counts share a context, renderer, and mesh.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, GraphicsContext, Mesh, MeshRenderer, RenderTarget, SurfaceOptions, Vertex,
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct WindowState {
    window: Arc<Window>,
    target: RenderTarget<'static>,
    retry_at: Option<Instant>,
}

struct State {
    graphics: GraphicsContext,
    renderer: MeshRenderer,
    mesh: Mesh,
    windows: Vec<WindowState>,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let first = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — first window (1x)")
                    .with_inner_size(PhysicalSize::new(800, 600))
                    .with_position(PhysicalPosition::new(80, 80)),
            )?,
        );
        let size = first.inner_size();
        let (graphics, first_target) = pollster::block_on(GraphicsContext::with_surface(
            first.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;

        let second = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — second window (4x MSAA)")
                    .with_inner_size(PhysicalSize::new(600, 500))
                    .with_position(PhysicalPosition::new(460, 120)),
            )?,
        );
        let size = second.inner_size();
        let second_target = graphics.create_surface(
            second.clone(),
            SurfaceOptions::new(size.width, size.height).sample_count(4),
        )?;

        let renderer = MeshRenderer::new(&graphics);
        let mesh = graphics.create_mesh(
            &[
                Vertex::new([0.0, 0.7, 0.0], [1.0, 0.0, 0.0, 1.0]),
                Vertex::new([-0.7, -0.7, 0.0], [0.0, 1.0, 0.0, 1.0]),
                Vertex::new([0.7, -0.7, 0.0], [0.0, 0.0, 1.0, 1.0]),
            ],
            &[0, 1, 2],
        )?;
        first.request_redraw();
        second.request_redraw();
        Ok(Self {
            graphics,
            renderer,
            mesh,
            windows: vec![
                WindowState {
                    window: first,
                    target: first_target,
                    retry_at: None,
                },
                WindowState {
                    window: second,
                    target: second_target,
                    retry_at: None,
                },
            ],
        })
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
    failure: Option<Box<dyn StdError>>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn StdError>>) {
        self.failure = Some(error.into());
        event_loop.exit();
    }

    fn redraw(&mut self, index: usize) -> Result<(), Box<dyn StdError>> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let window = &mut state.windows[index];
        let mut frame = match window.target.begin_frame() {
            Ok(frame) => frame,
            Err(FrameError::Retry) => {
                window.retry_at = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            Err(FrameError::Suspended) => {
                window.retry_at = None;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        {
            let mut pass = frame.render_pass().begin()?;
            state.renderer.draw(&mut pass, &state.mesh)?;
        }
        frame.finish()?;
        window.retry_at = None;
        Ok(())
    }

    fn recreate_surface(&mut self, index: usize) -> Result<(), Error> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let window = &mut state.windows[index];
        let size = window.window.inner_size();
        let count = window.target.sample_count();
        window.target = state.graphics.create_surface(
            window.window.clone(),
            SurfaceOptions::new(size.width, size.height).sample_count(count),
        )?;
        window.window.request_redraw();
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
        let Some(state) = &mut self.state else { return };
        let Some(index) = state
            .windows
            .iter()
            .position(|window| window.window.id() == id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                state.windows.remove(index);
                if state.windows.is_empty() {
                    event_loop.exit()
                }
            }
            WindowEvent::Resized(size) => {
                let window = &mut state.windows[index];
                if let Err(error) = window.target.resize(size.width, size.height) {
                    self.fail(event_loop, error);
                    return;
                }
                window.window.request_redraw();
            }
            WindowEvent::Occluded(false) => state.windows[index].window.request_redraw(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw(index) {
                    if error.downcast_ref::<FrameError>() == Some(&FrameError::SurfaceLost) {
                        if let Err(error) = self.recreate_surface(index) {
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
        let mut wake_at = None;
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

fn main() -> Result<(), Box<dyn StdError>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    match app.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
