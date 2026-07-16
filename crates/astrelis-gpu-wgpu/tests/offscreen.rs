//! Headless wgpu backend integration tests.

use astrelis_gpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, BufferBinding, BufferBindingType, BufferDescriptor,
    BufferTextureCopy, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipelineDescriptor, DeviceDescriptor,
    Extent3d, FragmentState, LoadOp, MapMode, PipelineLayoutDescriptor, PollMode, PrimitiveState,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderStages, StoreOp, TextureCopy,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

fn gpu_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[test]
fn clears_and_reads_back_one_pixel() {
    let _guard = gpu_test_lock().lock().expect("GPU test lock poisoned");
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping GPU integration test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .expect("request device");

        let texture = device.create_texture(TextureDescriptor {
            label: Some("readback target".into()),
            size: Extent3d::d2(1, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        });
        let view = texture.create_view(TextureViewDescriptor::default());
        let readback = device.create_buffer(BufferDescriptor {
            label: Some("readback buffer".into()),
            size: 256,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        encoder
            .render_pass(RenderPassDescriptor {
                label: Some("clear red".into()),
                color_attachments: vec![Some(RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    load: LoadOp::Clear(Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                })],
                timestamp_writes: None,
            })
            .expect("record clear");
        encoder
            .copy_texture_to_buffer(
                &TextureCopy {
                    texture,
                    mip_level: 0,
                    origin: Default::default(),
                },
                &BufferTextureCopy {
                    buffer: readback.clone(),
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
                Extent3d::d2(1, 1),
            )
            .expect("record readback");
        queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");

        let mapping = readback.map_async(MapMode::Read, 0..256);
        device.poll(PollMode::Wait).expect("wait for GPU");
        mapping.await.expect("map readback");
        let bytes = readback.read_mapped(0..4).expect("read mapped bytes");
        readback.unmap();
        assert_eq!(bytes, [255, 0, 0, 255]);
    });
}

#[test]
fn draws_a_triangle_and_reads_back_a_pixel() {
    let _guard = gpu_test_lock().lock().expect("GPU test lock poisoned");
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping GPU integration test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .expect("request device");

        let vertices: [[f32; 2]; 3] = [[-1.0, -1.0], [3.0, -1.0], [-1.0, 3.0]];
        let vertex_buffer = device
            .create_buffer_init(
                &queue,
                Some("fullscreen triangle".into()),
                bytemuck::cast_slice(&vertices),
                BufferUsages::VERTEX,
            )
            .expect("vertex buffer");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("triangle shader".into()),
            wgsl: r#"
                struct VertexOutput {
                    @builtin(position) position: vec4<f32>,
                };

                @vertex
                fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
                    var output: VertexOutput;
                    output.position = vec4<f32>(position, 0.0, 1.0);
                    return output;
                }

                @fragment
                fn fs_main() -> @location(0) vec4<f32> {
                    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
                }
            "#
            .into(),
        });
        let pipeline = device
            .create_render_pipeline(RenderPipelineDescriptor {
                label: Some("triangle pipeline".into()),
                layout: None,
                vertex: VertexState {
                    module: shader.clone(),
                    entry_point: "vs_main".into(),
                    buffers: vec![VertexBufferLayout {
                        array_stride: 8,
                        step_mode: VertexStepMode::Vertex,
                        attributes: vec![VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x2,
                        }],
                    }],
                },
                primitive: PrimitiveState::default(),
                fragment: Some(FragmentState {
                    module: shader,
                    entry_point: "fs_main".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::Rgba8Unorm,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
            })
            .expect("pipeline");

        let texture = device.create_texture(TextureDescriptor {
            label: Some("triangle target".into()),
            size: Extent3d::d2(1, 1),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        });
        let view = texture.create_view(TextureViewDescriptor::default());
        let readback = device.create_buffer(BufferDescriptor {
            label: Some("triangle readback".into()),
            size: 256,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        {
            let mut pass = encoder
                .begin_render_pass(RenderPassDescriptor {
                    label: Some("draw triangle".into()),
                    color_attachments: vec![Some(RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        load: LoadOp::Clear(Color::default()),
                        store: StoreOp::Store,
                    })],
                    timestamp_writes: None,
                })
                .expect("begin render pass");
            pass.set_pipeline(&pipeline).expect("set pipeline");
            pass.set_vertex_buffer(0, &vertex_buffer, 0..vertex_buffer.size())
                .expect("set vertex buffer");
            pass.draw(0..3, 0..1);
        }
        encoder
            .copy_texture_to_buffer(
                &TextureCopy {
                    texture,
                    mip_level: 0,
                    origin: Default::default(),
                },
                &BufferTextureCopy {
                    buffer: readback.clone(),
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
                Extent3d::d2(1, 1),
            )
            .expect("record readback");
        queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");
        let mapping = readback.map_async(MapMode::Read, 0..256);
        device.poll(PollMode::Wait).expect("wait for GPU");
        mapping.await.expect("map readback");
        let bytes = readback.read_mapped(0..4).expect("read mapped bytes");
        readback.unmap();
        assert_eq!(bytes, [0, 255, 0, 255]);
    });
}

#[test]
fn compute_pipeline_writes_through_a_bind_group() {
    let _guard = gpu_test_lock().lock().expect("GPU test lock poisoned");
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping GPU integration test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .expect("request device");
        let storage = device.create_buffer(BufferDescriptor {
            label: Some("compute storage".into()),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(BufferDescriptor {
            label: Some("compute readback".into()),
            size: 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(BindGroupLayoutDescriptor {
            label: Some("compute layout".into()),
            entries: vec![BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            }],
        });
        let pipeline_layout = device
            .create_pipeline_layout(PipelineLayoutDescriptor {
                label: Some("compute pipeline layout".into()),
                bind_group_layouts: vec![bind_group_layout.clone()],
            })
            .expect("pipeline layout");
        let bind_group = device
            .create_bind_group(BindGroupDescriptor {
                label: Some("compute bind group".into()),
                layout: bind_group_layout,
                entries: vec![BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: storage.clone(),
                        offset: 0,
                        size: None,
                    }),
                }],
            })
            .expect("bind group");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("compute shader".into()),
            wgsl: r#"
                @group(0) @binding(0)
                var<storage, read_write> output: array<u32>;

                @compute @workgroup_size(1)
                fn main() {
                    output[0] = 42u;
                }
            "#
            .into(),
        });
        let pipeline = device
            .create_compute_pipeline(ComputePipelineDescriptor {
                label: Some("compute pipeline".into()),
                layout: Some(pipeline_layout),
                module: shader,
                entry_point: "main".into(),
            })
            .expect("compute pipeline");
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        {
            let mut pass = encoder
                .begin_compute_pass(ComputePassDescriptor::default())
                .expect("begin compute pass");
            pass.set_pipeline(&pipeline).expect("set compute pipeline");
            pass.set_bind_group(0, &bind_group, &[])
                .expect("set compute bind group");
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder
            .copy_buffer_to_buffer(&storage, 0, &readback, 0, 4)
            .expect("copy compute result");
        queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit compute");
        let mapping = readback.map_async(MapMode::Read, 0..4);
        device.poll(PollMode::Wait).expect("wait for compute");
        mapping.await.expect("map compute result");
        let bytes = readback.read_mapped(0..4).expect("read compute result");
        readback.unmap();
        assert_eq!(
            u32::from_le_bytes(bytes.try_into().expect("four bytes")),
            42
        );
    });
}
