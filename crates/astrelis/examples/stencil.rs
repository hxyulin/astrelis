//! A stencil mask clips a colored quad to a diamond without writing mask color.
//! Press Space to toggle stencil clipping.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, GraphicsContext, Material, MaterialOptions, Mesh, MeshRenderer,
    RenderTarget, SurfaceOptions, Vertex, wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Stencil8;

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    renderer: MeshRenderer,
    mesh: Mesh,
    mask: Mesh,
    mask_material: Material,
    clip_material: Material,
    enabled: bool,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — stencil diamond clip (Space toggles)")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height)
                .sample_count(4)
                .depth_stencil(FORMAT),
        ))?;
        let mut renderer = MeshRenderer::new(&graphics);
        let mesh = graphics.create_mesh(
            &[
                Vertex::new([-0.85, -0.85, 0.0], [1.0, 0.2, 0.1, 1.0]),
                Vertex::new([0.85, -0.85, 0.0], [0.1, 1.0, 0.3, 1.0]),
                Vertex::new([0.85, 0.85, 0.0], [0.2, 0.4, 1.0, 1.0]),
                Vertex::new([-0.85, 0.85, 0.0], [0.9, 0.1, 1.0, 1.0]),
            ],
            &[0, 1, 2, 0, 2, 3],
        )?;
        let mask = graphics.create_mesh(
            &[
                Vertex::new([0.0, -0.7, 0.0], [1.0; 4]),
                Vertex::new([0.7, 0.0, 0.0], [1.0; 4]),
                Vertex::new([0.0, 0.7, 0.0], [1.0; 4]),
                Vertex::new([-0.7, 0.0, 0.0], [1.0; 4]),
            ],
            &[0, 1, 2, 0, 2, 3],
        )?;
        let replace = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            pass_op: wgpu::StencilOperation::Replace,
            ..Default::default()
        };
        let mask_material = graphics.create_material(
            MaterialOptions::new(renderer.default_material().shader())
                .write_mask(wgpu::ColorWrites::empty())
                .depth_stencil(Some(wgpu::DepthStencilState::stencil(
                    FORMAT,
                    wgpu::StencilState {
                        front: replace,
                        back: replace,
                        read_mask: 0xff,
                        write_mask: 0xff,
                    },
                ))),
        );
        let equal = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            ..Default::default()
        };
        let clip_material = graphics.create_material(
            MaterialOptions::new(renderer.default_material().shader()).depth_stencil(Some(
                wgpu::DepthStencilState::stencil(
                    FORMAT,
                    wgpu::StencilState {
                        front: equal,
                        back: equal,
                        read_mask: 0xff,
                        write_mask: 0,
                    },
                ),
            )),
        );
        renderer.prepare_for_target(&target)?;
        renderer.prepare_material_for_target(&mask_material, &target)?;
        renderer.prepare_material_for_target(&clip_material, &target)?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            mesh,
            mask,
            mask_material,
            clip_material,
            enabled: true,
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
            let mut pass = frame.render_pass().begin()?;
            if state.enabled {
                // Stencil starts at zero. Write one inside the diamond without writing color.
                pass.set_stencil_reference(1);
                state
                    .renderer
                    .draw_with_material(&mut pass, &state.mask, &state.mask_material)?;
                // The quad draws only where stored stencil equals the reference (one).
                state
                    .renderer
                    .draw_with_material(&mut pass, &state.mesh, &state.clip_material)?;
            } else {
                state.renderer.draw(&mut pass, &state.mesh)?;
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
            SurfaceOptions::new(size.width, size.height)
                .sample_count(count)
                .depth_stencil(FORMAT),
        )?;
        state.renderer.prepare_for_target(&state.target)?;
        state
            .renderer
            .prepare_material_for_target(&state.mask_material, &state.target)?;
        state
            .renderer
            .prepare_material_for_target(&state.clip_material, &state.target)?;
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && event.physical_key == PhysicalKey::Code(KeyCode::Space) =>
            {
                state.enabled = !state.enabled;
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
