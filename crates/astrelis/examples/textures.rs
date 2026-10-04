//! Upload an sRGB checker image, then crop, filter, and alpha-composite prepared rectangles.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, GraphicsContext, RenderTarget, SurfaceOptions, Texture, TextureBinding,
    TextureDrawOptions, TextureFilter, TextureOptions, TextureRenderer, wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    renderer: TextureRenderer,
    _texture: Texture,
    nearest: TextureBinding,
    linear: TextureBinding,
    crop: TextureBinding,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — textures: nearest, linear, cropped overlay")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let texture = graphics.create_texture(TextureOptions::new(16, 16))?;
        let mut bytes = Vec::with_capacity(16 * 16 * 4);
        for y in 0..16 {
            for x in 0..16 {
                let rgb = if (x / 2 + y / 2) % 2 == 0 {
                    [240, 105, 55]
                } else {
                    [45, 155, 240]
                };
                let alpha = if (5..11).contains(&x) && (5..11).contains(&y) {
                    100
                } else {
                    255
                };
                bytes.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
            }
        }
        texture.write(&bytes)?;
        let mut renderer = TextureRenderer::new(&graphics);
        let nearest = renderer.create_binding(
            texture.view(),
            TextureDrawOptions::new()
                .destination([0.06, 0.12, 0.40, 0.76])
                .filter(TextureFilter::Nearest),
        )?;
        let linear = renderer.create_binding(
            texture.view(),
            TextureDrawOptions::new().destination([0.54, 0.12, 0.40, 0.76]),
        )?;
        let crop = renderer.create_binding(
            texture.view(),
            TextureDrawOptions::new()
                .source([0.0, 0.0, 0.5, 0.5])
                .destination([0.32, 0.34, 0.36, 0.32])
                .tint([0.65, 1.0, 0.8, 0.65]),
        )?;
        for binding in [&nearest, &linear, &crop] {
            renderer.prepare_for_target(binding, &target)?;
        }
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            _texture: texture,
            nearest,
            linear,
            crop,
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
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.025,
                    g: 0.035,
                    b: 0.06,
                    a: 1.0,
                })
                .begin()?;
            state.renderer.draw(&mut pass, &state.nearest)?;
            state.renderer.draw(&mut pass, &state.linear)?;
            state.renderer.draw(&mut pass, &state.crop)?;
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
        for binding in [&state.nearest, &state.linear, &state.crop] {
            state.renderer.prepare_for_target(binding, &state.target)?;
        }
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
