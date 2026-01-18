use std::sync::Arc;
use astrelis_core::logging;
use astrelis_render::{
    BlendMode, Color, Framebuffer, GraphicsContext, RenderTarget, RenderableWindow,
    Renderer, WindowContextDescriptor, wgpu,
};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

struct RendererApp {
    context: Arc<GraphicsContext>,
    renderer: Renderer,
    window: RenderableWindow,
    window_id: WindowId,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    // Offscreen framebuffer for demonstrating framebuffer rendering
    offscreen_fb: Framebuffer,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group: wgpu::BindGroup,
    time: f32,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();
        let renderer = Renderer::new(graphics_ctx.clone());

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Renderer API Example".to_string(),
                size: Some(PhysicalSize::new(800.0, 600.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        );

        let window_id = window.id();

        // Create shader using Renderer API
        let shader = renderer.create_shader(Some("Color Shader"), SHADER_SOURCE);

        // Create texture using Renderer helper
        let texture_data = create_gradient_texture();
        let texture = renderer.create_texture_2d(
            Some("Gradient Texture"),
            256,
            256,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::TEXTURE_BINDING,
            &texture_data,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = renderer.create_linear_sampler(Some("Linear Sampler"));

        // Create bind group using Renderer API
        let bind_group_layout = renderer.create_bind_group_layout(
            Some("Texture Bind Group Layout"),
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
        );

        let bind_group = renderer.create_bind_group(
            Some("Texture Bind Group"),
            &bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        let pipeline_layout = renderer.create_pipeline_layout(
            Some("Render Pipeline Layout"),
            &[&bind_group_layout],
            &[],
        );

        // Create pipeline using Renderer API with BlendMode
        let pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                // Use BlendMode for transparent rendering
                targets: &[Some(
                    BlendMode::Alpha.to_color_target_state(wgpu::TextureFormat::Rgba8UnormSrgb),
                )],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        #[rustfmt::skip]
        let vertices: &[f32] = &[
            -0.8, -0.8,  0.0, 1.0,
             0.8, -0.8,  1.0, 1.0,
             0.8,  0.8,  1.0, 0.0,
            -0.8, -0.8,  0.0, 1.0,
             0.8,  0.8,  1.0, 0.0,
            -0.8,  0.8,  0.0, 0.0,
        ];

        // Create vertex buffer using Renderer helper
        let vertex_buffer = renderer.create_vertex_buffer(Some("Vertex Buffer"), vertices);

        // Create offscreen framebuffer using the new Framebuffer abstraction
        let offscreen_fb = Framebuffer::builder(400, 300)
            .format(wgpu::TextureFormat::Rgba8UnormSrgb)
            .label("Offscreen FB")
            .build(&graphics_ctx);

        // Create blit shader and pipeline for rendering framebuffer to surface
        let blit_shader = renderer.create_shader(Some("Blit Shader"), BLIT_SHADER_SOURCE);

        let blit_bind_group_layout = renderer.create_bind_group_layout(
            Some("Blit Bind Group Layout"),
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
        );

        let blit_bind_group = renderer.create_bind_group(
            Some("Blit Bind Group"),
            &blit_bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(offscreen_fb.color_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        let blit_pipeline_layout = renderer.create_pipeline_layout(
            Some("Blit Pipeline Layout"),
            &[&blit_bind_group_layout],
            &[],
        );

        let blit_pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                // Use PremultipliedAlpha for framebuffer blitting
                targets: &[Some(
                    BlendMode::PremultipliedAlpha
                        .to_color_target_state(wgpu::TextureFormat::Bgra8UnormSrgb),
                )],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        tracing::info!("Renderer initialized successfully");
        tracing::info!("Device: {:?}", renderer.context().info());

        Box::new(RendererApp {
            context: graphics_ctx,
            renderer,
            window,
            window_id,
            pipeline,
            bind_group,
            vertex_buffer,
            offscreen_fb,
            blit_pipeline,
            blit_bind_group,
            time: 0.0,
        })
    });
}

impl App for RendererApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Global logic - update animation time
        self.time += 0.016;
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window-specific resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        let mut frame = self.window.begin_drawing();

        // Pass 1: Render to offscreen framebuffer with automatic scoping
        frame.clear_and_render(
            RenderTarget::Framebuffer(&self.offscreen_fb),
            Color::rgb(0.2, 0.1, 0.3),
            |pass| {
                let pass = pass.descriptor();
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.draw(0..6, 0..1);
            },
        );

        // Pass 2: Blit framebuffer to surface with automatic scoping
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.1, 0.2, 0.3),
            |pass| {
                let pass = pass.descriptor();
                pass.set_pipeline(&self.blit_pipeline);
                pass.set_bind_group(0, &self.blit_bind_group, &[]);
                // Draw fullscreen triangle
                pass.draw(0..3, 0..1);
            },
        );

        frame.finish();
    }
}

fn create_gradient_texture() -> Vec<u8> {
    let mut texture_data = vec![0u8; (256 * 256 * 4) as usize];
    for y in 0..256 {
        for x in 0..256 {
            let idx = ((y * 256 + x) * 4) as usize;
            texture_data[idx] = x as u8;
            texture_data[idx + 1] = y as u8;
            texture_data[idx + 2] = ((x + y) / 2) as u8;
            texture_data[idx + 3] = 255;
        }
    }
    texture_data
}

const SHADER_SOURCE: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.tex_coords = in.tex_coords;
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
"#;

const BLIT_SHADER_SOURCE: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    var tex_coords = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0)
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

@group(0) @binding(0)
var t_source: texture_2d<f32>;
@group(0) @binding(1)
var s_source: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_source, s_source, in.tex_coords);
}
"#;
