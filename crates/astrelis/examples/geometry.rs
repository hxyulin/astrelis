//! Two instanced quads with custom vertex streams, Uint16 indices, and a checked shader.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, GraphicsContext, Material, MaterialOptions, Mesh, MeshIndices, MeshOptions,
    MeshRenderer, RenderTarget, SurfaceOptions, VertexLayout, VertexStream, wgpu,
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
    renderer: MeshRenderer,
    mesh: Mesh,
    material: Material,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — custom instanced geometry")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut renderer = MeshRenderer::new(&graphics);
        let layouts = [
            VertexLayout {
                stride: 8,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: wgpu::vertex_attr_array![0 => Float32x2].to_vec(),
            },
            VertexLayout {
                stride: 24,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: wgpu::vertex_attr_array![1 => Float32x2, 2 => Float32x4].to_vec(),
            },
        ];
        let positions = [
            [-0.35f32, 0.55],
            [-0.35, -0.55],
            [0.35, 0.55],
            [0.35, -0.55],
        ];
        let instances = [
            [-0.5f32, 0., 1., 0.25, 0.15, 1.],
            [0.5, 0., 0.15, 0.65, 1., 1.],
        ];
        let streams = [
            VertexStream::new(&positions, layouts[0].clone()),
            VertexStream::new(&instances, layouts[1].clone()),
        ];
        let mesh = graphics.create_mesh_with_options(
            MeshOptions::new(&streams).indices(MeshIndices::U16(&[0, 1, 2, 2, 1, 3])),
        )?;
        let shader = graphics
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("instanced custom geometry"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
                struct Output {
                    @builtin(position) position: vec4<f32>,
                    @location(0) color: vec4<f32>,
                };
                @vertex fn vertex_main(@location(0) position: vec2<f32>,
                    @location(1) offset: vec2<f32>, @location(2) color: vec4<f32>) -> Output {
                    var output: Output;
                    output.position = vec4(position + offset, 0.0, 1.0);
                    output.color = color;
                    return output;
                }
                @fragment fn fragment_main(input: Output) -> @location(0) vec4<f32> {
                    return input.color;
                }
            "#
                    .into(),
                ),
            });
        let material = graphics.create_material(
            MaterialOptions::new(&shader)
                .blend(None)
                .vertex_layouts(&layouts),
        );
        pollster::block_on(renderer.try_prepare_material(&material, &target.render_format()))?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            mesh,
            material,
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
            let mut quads =
                state
                    .renderer
                    .bind_with_material(&mut pass, &state.mesh, &state.material)?;
            quads.draw_range(&state.mesh.full_draw().instances(0..2))?;
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
