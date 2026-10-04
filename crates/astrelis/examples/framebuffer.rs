//! Render a 4x MSAA triangle offscreen, then sample it into the window in one submission.
//! Copy this file into a binary using astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    Error, FrameError, Framebuffer, FramebufferOptions, GraphicsContext, Mesh, MeshRenderer,
    RenderPass, RenderTarget, SurfaceOptions, Vertex, wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const COMPOSITE_SHADER: &str = r#"
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> Output {
    let positions = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let position = positions[index];
    var out: Output;
    out.position = vec4(position, 0.0, 1.0);
    out.uv = position * vec2(0.5, -0.5) + vec2(0.5, 0.5);
    return out;
}
@fragment fn fragment_main(in: Output) -> @location(0) vec4<f32> {
    let color = textureSample(image, image_sampler, in.uv);
    // Swap red and blue to demonstrate that this pass samples the offscreen result.
    return vec4(color.bgr, color.a);
}
"#;

// Application-defined renderer; Astrelis exposes the pass for custom GPU drawing.
struct Compositor {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    binding: wgpu::BindGroup,
}

impl Compositor {
    fn new(
        graphics: &GraphicsContext,
        format: wgpu::TextureFormat,
        view: &wgpu::TextureView,
    ) -> Self {
        let device = graphics.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Framebuffer compositor shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Framebuffer compositor bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Framebuffer compositor layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Framebuffer compositor pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Framebuffer compositor sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let binding = Self::bind(graphics, &layout, &sampler, view);
        Self {
            pipeline,
            layout,
            sampler,
            binding,
        }
    }

    fn bind(
        graphics: &GraphicsContext,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        graphics
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Framebuffer compositor image"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
    }

    fn rebind(&mut self, graphics: &GraphicsContext, view: &wgpu::TextureView) {
        self.binding = Self::bind(graphics, &self.layout, &self.sampler, view);
    }

    fn draw(&self, pass: &mut RenderPass<'_>) {
        let raw = pass.as_wgpu();
        raw.set_pipeline(&self.pipeline);
        raw.set_bind_group(0, &self.binding, &[]);
        raw.draw(0..3, 0..1);
    }
}

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    renderer: MeshRenderer,
    mesh: Mesh,
    framebuffer: Framebuffer,
    compositor: Compositor,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis — framebuffer, 4x MSAA and color swap")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let renderer = MeshRenderer::new(&graphics);
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
        let compositor = Compositor::new(&graphics, target.format(), framebuffer.color_view()?);
        window.request_redraw();
        Ok(Self {
            window,
            graphics,
            target,
            renderer,
            mesh,
            framebuffer,
            compositor,
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
                .clear_color(wgpu::Color::BLACK)
                .begin()?;
            state.renderer.draw(&mut pass, &state.mesh)?;
        }
        {
            let mut pass = frame
                .render_pass()
                .label("sample offscreen color")
                .begin()?;
            state.compositor.draw(&mut pass);
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
        state.compositor = Compositor::new(
            &state.graphics,
            state.target.format(),
            state.framebuffer.color_view()?,
        );
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
                let changed = state.framebuffer.size() != [size.width, size.height];
                if let Err(error) = state
                    .target
                    .resize(size.width, size.height)
                    .and_then(|()| state.framebuffer.resize(size.width, size.height))
                {
                    self.fail(event_loop, error);
                    return;
                }
                // The output view changes on resize, so update the cached bind group.
                if changed && size.width != 0 && size.height != 0 {
                    match state.framebuffer.color_view() {
                        Ok(view) => state.compositor.rebind(&state.graphics, view),
                        Err(error) => {
                            self.fail(event_loop, error);
                            return;
                        }
                    }
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
