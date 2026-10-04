//! Mix solid shapes, capped lines, images, and custom mesh draws in explicit order.
//! Space toggles layer MSAA when 4x is supported; P pauses animation. Resize freely.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

use astrelis::{
    Error, Frame, FrameError, Framebuffer, FramebufferOptions, GraphicsContext, LineCap, LineDraw,
    LineRenderer, Mesh, MeshRenderer, Rect, RenderTarget, ShapeDraw, ShapeRenderer, SurfaceOptions,
    TextureBinding, TextureBindingOptions, TextureDraw, TextureOptions, TextureRenderer,
    Transform2D, Vertex, wgpu,
};

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    scene: CanvasLayer,
    last_tick: Instant,
    phase: f32,
    paused: bool,
}
// Application state groups the offscreen layer separately from its window target.
struct CanvasLayer {
    layer: Framebuffer,
    textures: TextureRenderer,
    checker: TextureBinding,
    composite: TextureBinding,
    shapes: ShapeRenderer,
    lines: LineRenderer,
    meshes: MeshRenderer,
    mesh: Mesh,
}
impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — mixed 2D (Space: MSAA, P: pause)")
                    .with_inner_size(PhysicalSize::new(960, 640)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut layer =
            graphics.create_framebuffer(FramebufferOptions::new(size.width, size.height))?;
        if layer.supported_sample_counts().contains(&4) {
            layer.set_sample_count(4)?;
        }
        let checker_image = graphics.create_texture(TextureOptions::new(8, 8))?;
        let mut pixels = [0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let color = if (x + y) % 2 == 0 {
                    [235, 244, 255, 255]
                } else {
                    [160, 190, 230, 255]
                };
                pixels[(y * 8 + x) * 4..(y * 8 + x + 1) * 4].copy_from_slice(&color);
            }
        }
        checker_image.write(&pixels)?;
        let textures = TextureRenderer::new(&graphics);
        let checker =
            textures.create_binding(checker_image.view(), TextureBindingOptions::new())?;
        let composite = textures.create_sampled_binding(&layer.sampled_color())?;
        let mut state = Self {
            window,
            graphics: graphics.clone(),
            target,
            scene: CanvasLayer {
                layer,
                textures,
                checker,
                composite,
                shapes: ShapeRenderer::new(&graphics),
                lines: LineRenderer::new(&graphics),
                meshes: MeshRenderer::new(&graphics),
                mesh: graphics.create_mesh(
                    &[
                        Vertex::new([-0.9, -0.8, 0.], [0.25, 1., 0.6, 1.]),
                        Vertex::new([0.9, -0.8, 0.], [0.2, 0.6, 1., 1.]),
                        Vertex::new([0., 0.9, 0.], [1., 0.8, 0.2, 1.]),
                    ],
                    &[0, 1, 2],
                )?,
            },
            last_tick: Instant::now(),
            phase: 0.,
            paused: false,
        };
        state.prepare()?;
        state.window.request_redraw();
        Ok(state)
    }
    fn surface_options(&self, width: u32, height: u32) -> SurfaceOptions {
        SurfaceOptions::new(width, height)
    }
    fn prepare(&mut self) -> Result<(), Error> {
        let format = self.scene.layer.render_format();
        self.scene.shapes.prepare(&format)?;
        self.scene.lines.prepare(&format)?;
        self.scene
            .meshes
            .prepare(format.colors[0].unwrap(), format.sample_count)?;
        self.scene
            .textures
            .prepare(&self.scene.checker, &self.scene.layer.render_format())?;
        self.scene
            .textures
            .prepare(&self.scene.composite, &self.target.render_format())
    }
    fn resize(&mut self, width: u32, height: u32) -> Result<(), Error> {
        self.target.resize(width, height)?;
        self.scene.layer.resize(width, height)
        // The composite binding follows replacement storage automatically.
    }
    fn toggle_msaa(&mut self) -> Result<(), Error> {
        if !self.scene.layer.supported_sample_counts().contains(&4) {
            return Ok(());
        }
        self.scene
            .layer
            .set_sample_count(if self.scene.layer.sample_count() == 1 {
                4
            } else {
                1
            })?;
        self.prepare()
    }
}
impl CanvasLayer {
    fn render(&mut self, phase: f32, frame: &mut Frame<'_, '_>) -> Result<(), Error> {
        let [width, height] = frame.size();
        let w = width as f32;
        let h = height as f32;
        {
            let mut pass = frame
                .render_to(&mut self.layer)
                .label("mixed shapes, images, lines, and meshes")
                .clear_color(wgpu::Color::TRANSPARENT)
                .begin()?;
            self.shapes.draw_many(
                &mut pass,
                &[
                    ShapeDraw::rounded_rect(
                        Rect::new(w * 0.03, h * 0.03, w * 0.94, h * 0.94),
                        h * 0.025,
                        [0.035, 0.055, 0.09, 1.],
                    ),
                    ShapeDraw::rounded_rect(
                        Rect::new(w * 0.05, h * 0.06, w * 0.90, h * 0.10),
                        h * 0.02,
                        [0.08, 0.18, 0.3, 1.],
                    ),
                ],
            )?;
            // Capped segments and a rotated ellipse use the same coordinate semantics.
            for (column, cap) in [LineCap::Butt, LineCap::Square, LineCap::Round]
                .into_iter()
                .enumerate()
            {
                let x = w * (0.1 + column as f32 * 0.3);
                self.lines.draw(
                    &mut pass,
                    LineDraw::new(
                        [x, h * 0.11],
                        [x + w * 0.16, h * 0.11],
                        [0.3, 0.85, 0.7, 1.],
                    )
                    .width(h * 0.015)
                    .cap(cap),
                )?;
            }
            let x = width / 20;
            let y = height / 5;
            pass.set_scissor_rect(x, y, (width - x * 2).max(1), (height * 3 / 5).max(1))?;
            for row in 0..4 {
                for column in 0..6 {
                    let wave = phase + row as f32 * 0.7 + column as f32 * 0.4;
                    let x = w * (0.065 + column as f32 * 0.15);
                    let y = h * (0.24 + row as f32 * 0.13 + wave.sin() * 0.018);
                    // Keep these operations adjacent and ordered. Translucent image
                    // and marker overlap, so regrouping all primitives changes pixels.
                    self.shapes.draw(
                        &mut pass,
                        ShapeDraw::rounded_rect(
                            Rect::new(x, y, w * 0.13, h * 0.11),
                            h * 0.012,
                            [0.08, 0.15, 0.24, 1.],
                        ),
                    )?;
                    self.textures.draw(
                        &mut pass,
                        &self.checker,
                        TextureDraw::new(Rect::new(
                            x + w * 0.007,
                            y + h * 0.012,
                            w * 0.065,
                            h * 0.08,
                        ))
                        .tint([0.6, 0.8, 1., 0.8]),
                    )?;
                    self.shapes.draw(
                        &mut pass,
                        ShapeDraw::ellipse(
                            Rect::new(-w * 0.02, -h * 0.017, w * 0.04, h * 0.034),
                            [0.9, 0.4, 0.15, 0.7],
                        )
                        .transform(
                            Transform2D::rotation(wave.sin() * 0.6)
                                .then(Transform2D::translation(x + w * 0.06, y + h * 0.06)),
                        ),
                    )?;
                    self.lines.draw(
                        &mut pass,
                        LineDraw::new(
                            [x + w * 0.015, y + h * 0.085],
                            [x + w * 0.105, y + h * 0.085],
                            [0.25, 0.8, 0.6, 1.],
                        )
                        .width(h * 0.004)
                        .cap(LineCap::Round),
                    )?;
                    // Custom GPU geometry joins the same pass through a local viewport.
                    pass.set_viewport(x + w * 0.083, y + h * 0.025, w * 0.035, h * 0.05, 0., 1.)?;
                    self.meshes.draw(&mut pass, &self.mesh)?;
                    pass.set_viewport(0., 0., w, h, 0., 1.)?;
                }
            }
            pass.set_scissor_rect(0, 0, width, height)?;
            let mut shapes = self.shapes.bind(&mut pass)?;
            shapes.draw(ShapeDraw::rounded_rect(
                Rect::new(w * 0.07, h * 0.86, w * 0.86, h * 0.035),
                h * 0.015,
                [0.06, 0.12, 0.18, 1.],
            ))?;
            shapes.draw(ShapeDraw::rounded_rect(
                Rect::new(
                    w * 0.07,
                    h * 0.86,
                    w * 0.86 * (0.5 + 0.45 * phase.sin()),
                    h * 0.035,
                ),
                h * 0.015,
                [0.25, 0.8, 0.65, 1.],
            ))?;
        }
        {
            let mut pass = frame
                .render_pass()
                .label("composite mixed layer")
                .clear_color(wgpu::Color {
                    r: 0.012,
                    g: 0.02,
                    b: 0.035,
                    a: 1.,
                })
                .begin()?;
            self.textures
                .draw(&mut pass, &self.composite, TextureDraw::default())?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
    failure: Option<Box<dyn StdError>>,
    retry_at: Option<Instant>,
    occluded: bool,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn StdError>>) {
        self.failure = Some(error.into());
        event_loop.exit();
    }

    fn redraw(&mut self) -> Result<(), Box<dyn StdError>> {
        if self.occluded {
            return Ok(());
        }
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let now = Instant::now();
        if !state.paused {
            state.phase += now.duration_since(state.last_tick).as_secs_f32() * 1.;
        }
        state.last_tick = now;
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
        state.scene.render(state.phase, &mut frame)?;
        frame.finish()?;
        self.retry_at = (!state.paused).then(|| Instant::now() + Duration::from_millis(16));
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
            state
                .surface_options(size.width, size.height)
                .sample_count(count),
        )?;
        state.prepare()?;
        state.window.request_redraw();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        self.occluded = false;
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
                    Key::Character(ref key) if key.eq_ignore_ascii_case("p") => {
                        state.paused = !state.paused;
                        state.last_tick = Instant::now();
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
                state.last_tick = Instant::now();
                if let Err(error) = state.resize(size.width, size.height) {
                    self.fail(event_loop, error);
                    return;
                }
                state.window.request_redraw();
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                state.last_tick = Instant::now();
                self.retry_at = None;
                if !occluded {
                    state.window.request_redraw();
                }
            }
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
