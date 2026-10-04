//! Default and custom mesh materials, with an application-owned tint uniform.
//! Press Space to update the tint without recreating the material or its pipeline.
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

const SHADER: &str = r#"
@group(0) @binding(0) var<uniform> tint: vec4<f32>;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}
@vertex
fn vertex_main(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> Output {
    var output: Output;
    output.position = vec4(position, 1.0);
    output.color = color;
    return output;
}
@fragment
fn fragment_main(input: Output) -> @location(0) vec4<f32> {
    let color = input.color * tint;
    return vec4(color.rgb * color.a, color.a);
}
"#;

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    renderer: MeshRenderer,
    mesh: Mesh,
    material: Material,
    tint_buffer: wgpu::Buffer,
    tint_group: wgpu::BindGroup,
    cool_tint: bool,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — default / custom material (Space changes tint)")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
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
        let device = graphics.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Application tint shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tint layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            }],
        });
        let material = graphics
            .create_material(MaterialOptions::new(&shader).bind_group_layouts(&[Some(&layout)]));
        let tint_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Application tint uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tint_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Application tint binding"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: tint_buffer.as_entire_binding(),
            }],
        });
        // Use standard byte conversion so this file needs no bytemuck dependency.
        let tint = [1.0_f32, 0.45, 0.2, 1.0].map(f32::to_le_bytes).concat();
        graphics.queue().write_buffer(&tint_buffer, 0, &tint);
        renderer.prepare(target.format(), target.sample_count())?;
        renderer.prepare_material(&material, target.format(), target.sample_count())?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            mesh,
            material,
            tint_buffer,
            tint_group,
            cool_tint: false,
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
        let [width, height] = frame.size();
        {
            let mut pass = frame.render_pass().begin()?;
            pass.set_viewport(0.0, 0.0, width as f32 / 2.0, height as f32, 0.0, 1.0)?;
            state.renderer.draw(&mut pass, &state.mesh)?;
            pass.set_viewport(
                width as f32 / 2.0,
                0.0,
                width as f32 / 2.0,
                height as f32,
                0.0,
                1.0,
            )?;
            pass.as_wgpu().set_bind_group(0, &state.tint_group, &[]);
            state
                .renderer
                .draw_with_material(&mut pass, &state.mesh, &state.material)?;
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
        state
            .renderer
            .prepare(state.target.format(), state.target.sample_count())?;
        state.renderer.prepare_material(
            &state.material,
            state.target.format(),
            state.target.sample_count(),
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && event.physical_key == PhysicalKey::Code(KeyCode::Space) =>
            {
                state.cool_tint = !state.cool_tint;
                let tint = if state.cool_tint {
                    [0.2_f32, 0.65, 1.0, 1.0]
                } else {
                    [1.0_f32, 0.45, 0.2, 1.0]
                };
                let bytes = tint.map(f32::to_le_bytes).concat();
                state
                    .graphics
                    .queue()
                    .write_buffer(&state.tint_buffer, 0, &bytes);
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
