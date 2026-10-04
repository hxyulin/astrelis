//! Two independent renderers draw overlapping meshes into one pass.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, GraphicsContext, Mesh, MeshRenderer, RenderTarget, Vertex, wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

fn quad(graphics: &GraphicsContext, bounds: [f32; 4], color: [f32; 4]) -> Result<Mesh, Error> {
    let [left, bottom, right, top] = bounds;
    graphics.create_mesh(
        &[
            Vertex::new([left, bottom, 0.0], color),
            Vertex::new([right, bottom, 0.0], color),
            Vertex::new([right, top, 0.0], color),
            Vertex::new([left, top, 0.0], color),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    background_renderer: MeshRenderer,
    overlay_renderer: MeshRenderer,
    background: Mesh,
    overlay: Mesh,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — two renderers, one pass")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            size.width,
            size.height,
        ))?;
        let background_renderer = MeshRenderer::new(&graphics);
        let overlay_renderer = MeshRenderer::new(&graphics);
        let background = quad(&graphics, [-0.8, -0.65, 0.3, 0.7], [1.0, 0.18, 0.08, 1.0])?;
        let overlay = quad(&graphics, [-0.3, -0.7, 0.8, 0.65], [0.05, 0.35, 1.0, 0.65])?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            background_renderer,
            overlay_renderer,
            background,
            overlay,
        })
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
            let mut pass = frame.begin_pass(wgpu::LoadOp::Clear(wgpu::Color::BLACK))?;
            state
                .background_renderer
                .draw(&mut pass, &state.background)?;
            state.overlay_renderer.draw(&mut pass, &state.overlay)?;
        }
        frame.present()?;
        self.retry_at = None;
        Ok(())
    }

    fn recreate_surface(&mut self) -> Result<(), Error> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let size = state.window.inner_size();
        state.target =
            state
                .graphics
                .create_surface(state.window.clone(), size.width, size.height)?;
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
