//! Application-owned winit loop using only WindowContext for surface lifecycle.
//! Copy this file into a desktop binary using astrelis-winit and pollster 0.4.
use astrelis_winit::{
    SurfaceSettings, WindowContext,
    astrelis::{FrameError, Mesh, MeshRenderer, Vertex, wgpu},
    winit::{
        application::ApplicationHandler,
        dpi::LogicalSize,
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
struct Drawing {
    // Keep this native reference independently of WindowContext's frame borrow.
    window: Arc<Window>,
    context: WindowContext,
    renderer: MeshRenderer,
    mesh: Mesh,
}
#[derive(Default)]
struct App {
    drawing: Option<Drawing>,
    failure: Option<Box<dyn Error>>,
    retry: Option<Instant>,
    recoveries: u32,
}
impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn Error>>) {
        if self.failure.is_none() {
            self.failure = Some(error.into());
        }
        event_loop.exit();
    }
    fn resume(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        if let Some(drawing) = &mut self.drawing {
            drawing.context.resume()?;
        } else {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Application-owned loop — WindowContext")
                        .with_inner_size(LogicalSize::new(800., 600.)),
                )?,
            );
            let context =
                pollster::block_on(WindowContext::new(window.clone(), SurfaceSettings::new()))?;
            let renderer = MeshRenderer::new(context.graphics());
            let mesh = context.graphics().create_mesh(
                &[
                    Vertex::new([0., 0.75, 0.], [1., 0.15, 0.15, 1.]),
                    Vertex::new([-0.75, -0.65, 0.], [0.15, 1., 0.15, 1.]),
                    Vertex::new([0.75, -0.65, 0.], [0.15, 0.35, 1., 1.]),
                ],
                &[0, 1, 2],
            )?;
            self.drawing = Some(Drawing {
                window,
                context,
                renderer,
                mesh,
            });
        }
        self.drawing.as_ref().unwrap().window.request_redraw();
        Ok(())
    }
    fn draw(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(drawing) = &mut self.drawing else {
            return Ok(());
        };
        if !drawing.context.can_present() {
            return Ok(());
        }
        drawing
            .renderer
            .prepare(drawing.context.render_format().unwrap())?;
        let acquired = drawing.context.begin_frame();
        let mut frame = match acquired {
            Ok(frame) => frame,
            Err(FrameError::Retry) => {
                self.retry = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            Err(FrameError::Suspended) => {
                self.retry = None;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        {
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color::BLACK)
                .begin()?;
            drawing.renderer.draw(&mut pass, &drawing.mesh)?;
        }
        drawing.window.pre_present_notify();
        frame.finish()?;
        self.retry = None;
        self.recoveries = 0;
        Ok(())
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.resume(event_loop) {
            self.fail(event_loop, error);
        }
    }
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(drawing) = &mut self.drawing {
            drawing.context.suspend();
        }
        self.retry = None;
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
        let Some(drawing) = &mut self.drawing else {
            return;
        };
        if drawing.window.id() != id {
            return;
        }
        match drawing.context.process_event(&event) {
            Ok(update) => {
                if update.needs_redraw() && drawing.context.can_present() {
                    drawing.window.request_redraw();
                }
            }
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    if error.downcast_ref::<FrameError>() == Some(&FrameError::SurfaceLost)
                        && self.recoveries < 3
                    {
                        self.recoveries += 1;
                        let drawing = self.drawing.as_mut().unwrap();
                        drawing.context.suspend();
                        if let Err(error) = drawing.context.resume() {
                            self.fail(event_loop, error);
                        } else {
                            self.retry = Some(Instant::now() + Duration::from_millis(16));
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
        let eligible = self
            .drawing
            .as_ref()
            .is_some_and(|d| d.context.can_present());
        if self.retry.is_some_and(|at| at <= Instant::now()) {
            self.retry = None;
            if let Some(drawing) = &self.drawing
                && eligible
            {
                drawing.window.request_redraw();
            }
        }
        event_loop.set_control_flow(if eligible {
            self.retry.map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
        } else {
            ControlFlow::Wait
        });
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.drawing = None;
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::default();
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.failure {
        Err(error)
    } else {
        Ok(())
    }
}
