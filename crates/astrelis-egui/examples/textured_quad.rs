use std::sync::Arc;
use astrelis_core::logging;
use astrelis_egui::Egui;
use astrelis_render::{GraphicsContext, RenderableWindow};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

#[allow(dead_code)]
struct TexturedQuadApp {
    _context: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    egui: Egui,

    // Render resources
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,

    // Render target for custom rendering
    render_texture: wgpu::Texture,
    render_texture_view: wgpu::TextureView,
    render_texture_id: Option<egui::TextureId>,

    // Animation
    time: f32,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Textured Quad in EGUI".to_string(),
                size: Some(PhysicalSize::new(1280.0, 720.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new(window, graphics_ctx.clone());
        let window_id = window.id();
        let egui = Egui::new(&window, &graphics_ctx);

        // Create shader
        let shader = graphics_ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Quad Shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
            });

        // Create procedural texture
        let texture_size = wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        };

        let texture = graphics_ctx
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Procedural Texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        let mut texture_data = vec![0u8; (256 * 256 * 4) as usize];
        for y in 0..256 {
            for x in 0..256 {
                let idx = ((y * 256 + x) * 4) as usize;
                let dx = (x as f32 - 128.0) / 128.0;
                let dy = (y as f32 - 128.0) / 128.0;
                let dist = (dx * dx + dy * dy).sqrt();
                let pattern = ((dist * 10.0).sin() * 0.5 + 0.5) * 255.0;

                texture_data[idx] = pattern as u8;
                texture_data[idx + 1] = (x as f32 / 256.0 * 255.0) as u8;
                texture_data[idx + 2] = (y as f32 / 256.0 * 255.0) as u8;
                texture_data[idx + 3] = 255;
            }
        }

        graphics_ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: Some(256),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = graphics_ctx
            .device
            .create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

        let bind_group_layout =
            graphics_ctx
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Texture Bind Group Layout"),
                    entries: &[
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
                });

        let bind_group = graphics_ctx
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Texture Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

        let pipeline_layout =
            graphics_ctx
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline =
            graphics_ctx
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8UnormSrgb,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
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

        let vertex_buffer = graphics_ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (vertices.len() * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        graphics_ctx
            .queue
            .write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));

        // Create render target for our custom rendering
        let render_texture_size = wgpu::Extent3d {
            width: 512,
            height: 512,
            depth_or_array_layers: 1,
        };

        let render_texture = graphics_ctx
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Render Target"),
                size: render_texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        let render_texture_view =
            render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Box::new(TexturedQuadApp {
            _context: graphics_ctx,
            window,
            window_id,
            egui,
            pipeline,
            bind_group,
            vertex_buffer,
            render_texture,
            render_texture_view,
            render_texture_id: None,
            time: 0.0,
        })
    });
}

impl App for TexturedQuadApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Global logic - called once per frame
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

        self.egui.handle_events(&self.window, events);

        // Register the texture with egui once (on first frame)
        if self.render_texture_id.is_none() {
            let texture_id = self.egui.register_wgpu_texture(
                &self._context.device,
                &self.render_texture_view,
                wgpu::FilterMode::Linear,
            );
            self.render_texture_id = Some(texture_id);
        }

        // UI
        self.egui.ui(&self.window, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Custom Rendered Quad in EGUI");
                ui.separator();

                ui.label("This example demonstrates rendering a textured quad using");
                ui.label("the astrelis-render library and displaying it in an EGUI image.");

                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    ui.label("Animation time:");
                    ui.label(format!("{:.2}s", self.time));
                });

                ui.add_space(20.0);

                ui.group(|ui| {
                    ui.label(astrelis_egui::RichText::new("Custom Rendered Content").strong());
                    ui.separator();

                    // Display our custom rendered texture
                    if let Some(texture_id) = self.render_texture_id {
                        ui.image(egui::ImageSource::Texture(egui::load::SizedTexture {
                            id: texture_id,
                            size: egui::Vec2::new(512.0, 512.0),
                        }));
                    } else {
                        ui.label("Loading texture...");
                    }

                    ui.label("The quad is rendered using your custom render pipeline");
                    ui.label("and displayed as an EGUI image widget.");
                });
            });
        });

        // Custom rendering to texture
        {
            let graphics_ctx = &self._context;
            let mut encoder =
                graphics_ctx
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render to Texture Encoder"),
                    });

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render to Texture Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.render_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.05,
                                g: 0.05,
                                b: 0.1,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..6, 0..1);
            }

            graphics_ctx.queue.submit(std::iter::once(encoder.finish()));
        }

        // Main window rendering
        let mut frame = self.window.begin_drawing();

        // Clear to dark background with automatic scoping
        {
            use astrelis_render::{RenderPassBuilder, RenderTarget};
            let render_pass = RenderPassBuilder::new()
                .label("Clear Pass")
                .target(RenderTarget::Surface)
                .clear_color(astrelis_render::Color::rgb(0.15, 0.15, 0.15))
                .build(&mut frame);
            drop(render_pass);
        }

        self.egui.render(&self.window, &mut frame);
        frame.finish();
    }
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
