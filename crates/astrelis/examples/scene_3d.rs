//! Indexed instanced cubes with a perspective camera, custom shading, depth, and MSAA.
//! Space toggles MSAA when 4x is supported; P pauses animation. Resize freely.
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
    Error, FrameError, GraphicsContext, Material, MaterialOptions, Mesh, MeshIndices, MeshOptions,
    MeshRenderer, RenderTarget, SurfaceOptions, Vertex, VertexLayout, VertexStream, wgpu,
};
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const SHADER: &str = r#"
@group(0) @binding(0) var<uniform> camera: vec4<f32>; // angle, aspect, near, far
struct Output { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vertex_main(@location(0) position: vec3<f32>, @location(1) face: vec4<f32>,
    @location(2) offset_scale: vec4<f32>, @location(3) tint: vec4<f32>) -> Output {
    let c = cos(camera.x); let s = sin(camera.x);
    let tilted = vec3(position.x, position.y * 0.88 - position.z * 0.48,
                     position.y * 0.48 + position.z * 0.88);
    let rotated = vec3(c * tilted.x + s * tilted.z, tilted.y, -s * tilted.x + c * tilted.z);
    let view = rotated * offset_scale.w + offset_scale.xyz + vec3(0.0, 0.0, 7.0);
    let focal = 1.7;
    let depth = camera.w / (camera.w - camera.z);
    var output: Output;
    output.position = vec4(view.x * focal / camera.y, view.y * focal,
                          depth * view.z - depth * camera.z, view.z);
    output.color = face * tint;
    return output;
}
@fragment fn fragment_main(input: Output) -> @location(0) vec4<f32> {
    return input.color;
}
"#;
struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    renderer: MeshRenderer,
    mesh: Mesh,
    material: Material,
    camera: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    last_tick: Instant,
    angle: f32,
    paused: bool,
}
impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — 3D workload (Space: MSAA, P: pause)")
                    .with_inner_size(PhysicalSize::new(960, 640)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, mut target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height).depth_stencil(DEPTH),
        ))?;
        if target.supported_sample_counts().contains(&4) {
            target.set_sample_count(4)?;
        }
        // Separate face vertices give each face a different light level.
        let corners = [
            [-1., -1., -1.],
            [1., -1., -1.],
            [1., 1., -1.],
            [-1., 1., -1.],
            [-1., -1., 1.],
            [1., -1., 1.],
            [1., 1., 1.],
            [-1., 1., 1.],
        ];
        let faces = [
            [0, 1, 2, 3],
            [5, 4, 7, 6],
            [4, 0, 3, 7],
            [1, 5, 6, 2],
            [3, 2, 6, 7],
            [4, 5, 1, 0],
        ];
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (face, corners_of_face) in faces.iter().enumerate() {
            let base = vertices.len() as u16;
            let light = [0.65, 0.8, 0.45, 0.7, 1., 0.35][face];
            for &corner in corners_of_face {
                vertices.push(Vertex::new(corners[corner], [light, light, light, 1.]));
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let instances: Vec<[f32; 8]> = (0..9)
            .map(|i| {
                let x = (i % 3) as f32 - 1.;
                let y = (i / 3) as f32 - 1.;
                [
                    x * 2.,
                    y * 2.,
                    (x + y) * 0.4,
                    0.55,
                    0.3 + i as f32 * 0.07,
                    0.65,
                    1.,
                    1.,
                ]
            })
            .collect();
        let layouts = [
            VertexLayout::new(&Vertex::layout()),
            VertexLayout {
                stride: 32,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4].to_vec(),
            },
        ];
        let streams = [
            VertexStream::new(&vertices, layouts[0].clone()),
            VertexStream::new(&instances, layouts[1].clone()),
        ];
        let mesh = graphics.create_mesh_with_options(
            MeshOptions::new(&streams).indices(MeshIndices::U16(&indices)),
        )?;
        let device = graphics.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perspective cubes"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            }],
        });
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera binding"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let material = graphics.create_material(
            MaterialOptions::new(&shader)
                .vertex_layouts(&layouts)
                .bind_group_layouts(&[Some(&layout)])
                .blend(None)
                .depth_stencil(Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                })),
        );
        let mut renderer = MeshRenderer::new(&graphics);
        pollster::block_on(renderer.try_prepare_material(&material, &target.render_format()))?;
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            mesh,
            material,
            camera,
            camera_group,
            last_tick: Instant::now(),
            angle: 0.,
            paused: false,
        })
    }
    fn surface_options(&self, width: u32, height: u32) -> SurfaceOptions {
        SurfaceOptions::new(width, height).depth_stencil(DEPTH)
    }
    fn prepare(&mut self) -> Result<(), Error> {
        self.renderer
            .prepare_material_for_target(&self.material, &self.target)
    }
    fn resize(&mut self, width: u32, height: u32) -> Result<(), Error> {
        self.target.resize(width, height)
    }
    fn toggle_msaa(&mut self) -> Result<(), Error> {
        if !self.target.supported_sample_counts().contains(&4) {
            return Ok(());
        }
        self.target
            .set_sample_count(if self.target.sample_count() == 1 {
                4
            } else {
                1
            })?;
        self.prepare()
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
            state.angle += now.duration_since(state.last_tick).as_secs_f32() * 0.6;
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
        let [width, height] = frame.size();
        // One application-owned uniform update; geometry, material, and bindings persist.
        let camera = [state.angle, width as f32 / height as f32, 0.1, 100.];
        let mut bytes = [0u8; 16];
        for (slot, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(camera) {
            slot.copy_from_slice(&value.to_ne_bytes());
        }
        state
            .graphics
            .queue()
            .write_buffer(&state.camera, 0, &bytes);
        {
            let mut pass = frame
                .render_pass()
                .label("instanced depth-tested cubes")
                .clear_color(wgpu::Color {
                    r: 0.015,
                    g: 0.025,
                    b: 0.045,
                    a: 1.,
                })
                .begin()?;
            pass.set_bind_group(0, &state.camera_group, &[]);
            state.renderer.draw_range_with_material(
                &mut pass,
                &state.mesh,
                &state.material,
                &state.mesh.full_draw().instances(0..9),
            )?;
        }
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
