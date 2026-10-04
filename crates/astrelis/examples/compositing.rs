//! Composite a cropped 4x MSAA framebuffer into a clipped window region in one submission.
//! Space switches the offscreen target between 1x and 4x MSAA without manually rebuilding source bindings.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, Framebuffer, FramebufferOptions, GraphicsContext, Mesh, MeshRenderer, Rect,
    RenderTarget, SurfaceOptions, TextureBinding, TextureDraw, TextureRenderer, UvRect, Vertex,
    wgpu,
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
    renderer: MeshRenderer,
    mesh: Mesh,
    framebuffer: Framebuffer,
    compositor: TextureRenderer,
    binding: TextureBinding,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — framebuffer compositing (Space: MSAA)")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height)
                .sample_count(4)
                .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8),
        ))?;
        let mut renderer = MeshRenderer::new(&graphics);
        let mesh = graphics.create_mesh(
            &[
                Vertex::new([0.0, 0.75, 0.0], [1.0, 0.15, 0.15, 1.0]),
                Vertex::new([-0.75, -0.65, 0.0], [0.15, 1.0, 0.15, 1.0]),
                Vertex::new([0.75, -0.65, 0.0], [0.15, 0.35, 1.0, 1.0]),
            ],
            &[0, 1, 2],
        )?;
        let framebuffer = graphics
            .create_framebuffer(FramebufferOptions::new(size.width, size.height).sample_count(4))?;
        let mut compositor = TextureRenderer::new(&graphics);
        let binding = compositor.create_sampled_binding(&framebuffer.sampled_color())?;
        renderer.prepare(framebuffer.format(), framebuffer.sample_count())?;
        compositor.prepare_for_target(&binding, &target)?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            mesh,
            framebuffer,
            compositor,
            binding,
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
            let mut pass = frame
                .render_to(&mut state.framebuffer)
                .label("offscreen triangle")
                .clear_color(wgpu::Color::TRANSPARENT)
                .begin()?;
            state.renderer.draw(&mut pass, &state.mesh)?;
        }
        {
            let [width, height] = frame.size();
            let mut pass = frame
                .render_pass()
                .label("composite resolved color")
                .clear_color(wgpu::Color {
                    r: 0.025,
                    g: 0.035,
                    b: 0.06,
                    a: 1.0,
                })
                .scissor_rect(width / 5, height / 5, width * 3 / 5, height * 3 / 5)
                .begin()?;
            state.compositor.draw(
                &mut pass,
                &state.binding,
                TextureDraw::normalized(Rect::new(0.125, 0.125, 0.75, 0.75))
                    .uv(UvRect::new(0.1, 0.1, 0.8, 0.8)),
            )?;
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
            SurfaceOptions::new(size.width, size.height)
                .sample_count(count)
                .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8),
        )?;
        state
            .compositor
            .prepare_for_target(&state.binding, &state.target)?;
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Space) =>
            {
                let count = if state.framebuffer.sample_count() == 4 {
                    1
                } else {
                    4
                };
                if let Err(error) = state
                    .framebuffer
                    .set_sample_count(count)
                    .and_then(|()| state.renderer.prepare(state.framebuffer.format(), count))
                {
                    self.fail(event_loop, error);
                    return;
                }
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                if let Err(error) = state
                    .target
                    .resize(size.width, size.height)
                    .and_then(|()| state.framebuffer.resize(size.width, size.height))
                {
                    self.fail(event_loop, error);
                    return;
                }
                // The live sampled binding follows the framebuffer's new storage.
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
