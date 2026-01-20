use astrelis_core::logging;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor,
};
use astrelis_winit::{
    WindowId,
    app::run_app,
    window::{WindowBackend, WindowDescriptor},
};

struct App {
    window: RenderableWindow,
    window_id: WindowId,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Textured Window".to_string(),
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

        let shader = graphics_ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Texture Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("textured_window.wgsl").into()),
            });

        let texture_size = wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        };

        let texture = graphics_ctx
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Example Texture"),
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
                texture_data[idx] = x as u8;
                texture_data[idx + 1] = y as u8;
                texture_data[idx + 2] = ((x + y) / 2) as u8;
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
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
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
                            format: wgpu::TextureFormat::Bgra8UnormSrgb,
                            blend: Some(wgpu::BlendState::REPLACE),
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

        let window_id = window.id();

        Box::new(App {
            window,
            window_id,
            pipeline,
            bind_group,
            vertex_buffer,
        })
    });
}

impl astrelis_winit::app::App for App {
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx, _time: &astrelis_winit::app::FrameTime) {
        // Global logic (none needed for this example)
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        let mut frame = self.window.begin_drawing();

        // Render with automatic scoping (no manual {} block needed)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.1, 0.2, 0.3),
            |pass| {
                let pass = pass.descriptor();
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.draw(0..6, 0..1);
            },
        );

        frame.finish();
    }
}
