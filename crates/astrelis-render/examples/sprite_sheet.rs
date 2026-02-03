//! Sprite sheet example demonstrating animated sprites.
//!
//! This example shows how to:
//! - Create a sprite sheet from procedurally generated data
//! - Animate through sprite frames
//! - Render sprites with proper UV coordinates
//!
//! The example creates a simple 4-frame "spinning" animation.

use astrelis_core::logging;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor,
    SpriteSheet, SpriteSheetDescriptor, SpriteAnimation,
};
use astrelis_winit::{
    WindowId,
    app::run_app,
    window::{WindowBackend, WindowDescriptor, Window, WinitPhysicalSize},
};
use std::collections::HashMap;
use std::time::Instant;
use wgpu::util::DeviceExt;
use std::sync::Arc;

/// WGSL shader for rendering sprites
const SHADER: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var sprite_texture: texture_2d<f32>;
@group(0) @binding(2) var sprite_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.mvp * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.uv);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
}

/// Generate a 4-frame sprite sheet with a spinning indicator.
fn generate_sprite_sheet_data() -> (Vec<u8>, u32, u32) {
    const SPRITE_SIZE: u32 = 64;
    const COLUMNS: u32 = 4;
    const ROWS: u32 = 1;
    
    let width = SPRITE_SIZE * COLUMNS;
    let height = SPRITE_SIZE * ROWS;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    
    // Generate 4 frames of a spinning indicator.
    // Each frame rotates the "bright spot" by 90° (PI/2) around the circle.
    for frame in 0..4 {
        let base_x = frame * SPRITE_SIZE;
        let center = SPRITE_SIZE as f32 / 2.0;
        let radius = SPRITE_SIZE as f32 / 2.0 - 4.0; // 4px inset from sprite edge
        
        for y in 0..SPRITE_SIZE {
            for x in 0..SPRITE_SIZE {
                let px = (base_x + x) as usize;
                let py = y as usize;
                let idx = (py * width as usize + px) * 4;
                
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);
                
                // Draw a 3px-wide circle outline (anti-aliased ring)
                if (dist - radius).abs() < 3.0 {
                    // Rotate the "bright spot" origin by 90° per frame
                    let segment_angle = std::f32::consts::PI / 2.0 * frame as f32;
                    // Compute angle relative to this frame's origin, then wrap to [0, 2π]
                    let mut rel_angle = angle - segment_angle;
                    while rel_angle < 0.0 {
                        rel_angle += std::f32::consts::PI * 2.0;
                    }
                    while rel_angle > std::f32::consts::PI * 2.0 {
                        rel_angle -= std::f32::consts::PI * 2.0;
                    }

                    // Brightness fades from 1.0 (at the origin) to 0.0 going around the circle.
                    // This creates a "comet tail" gradient effect.
                    let brightness = 1.0 - (rel_angle / (std::f32::consts::PI * 2.0));
                    let r = (100.0 + 155.0 * brightness) as u8; // 100..255
                    let g = (150.0 + 105.0 * brightness) as u8; // 150..255
                    let b = 255; // always full blue
                    
                    pixels[idx] = r;
                    pixels[idx + 1] = g;
                    pixels[idx + 2] = b;
                    pixels[idx + 3] = 255;
                } else if dist < radius - 3.0 {
                    // Inner fill: alpha fades from 0.3 at 10px inside the ring to 0.0 at the ring edge.
                    // This gives a subtle glow effect inside the spinner.
                    let alpha = ((radius - 3.0 - dist) / 10.0).clamp(0.0, 0.3);
                    pixels[idx] = 100;
                    pixels[idx + 1] = 150;
                    pixels[idx + 2] = 200;
                    pixels[idx + 3] = (alpha * 255.0) as u8;
                }
            }
        }
    }
    
    (pixels, width, height)
}

struct App {
    _context: Arc<GraphicsContext>,
    windows: HashMap<WindowId, RenderableWindow>,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    sprite_sheet: SpriteSheet,
    animation: SpriteAnimation,
    last_update: Instant,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut windows = HashMap::new();

        let scale = Window::platform_dpi() as f32;
        let window = ctx
            .create_window(WindowDescriptor {
                title: "Sprite Sheet Animation Example".to_string(),
                size: Some(WinitPhysicalSize::new(400.0 * scale, 400.0 * scale)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let renderable_window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        ).expect("Failed to create renderable window");

        let window_id = renderable_window.id();
        windows.insert(window_id, renderable_window);

        // Generate sprite sheet
        let (sprite_data, tex_width, tex_height) = generate_sprite_sheet_data();
        let sprite_sheet = SpriteSheet::from_data(
            &graphics_ctx,
            &sprite_data,
            tex_width,
            tex_height,
            SpriteSheetDescriptor {
                sprite_width: 64,
                sprite_height: 64,
                columns: 4,
                rows: 1,
                ..Default::default()
            },
        );

        // Create animation (4 frames at 8 fps)
        let animation = SpriteAnimation::new(4, 8.0);

        // Create shader module
        let shader = graphics_ctx.device().create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        // Create bind group layout
        let bind_group_layout = graphics_ctx.device().create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sprite Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = graphics_ctx.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sprite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = graphics_ctx.device().create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sprite Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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

        // Create uniform buffer
        let uniforms = Uniforms {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let uniform_buffer = graphics_ctx.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create sampler
        let sampler = graphics_ctx.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sprite Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group
        let bind_group = graphics_ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sprite Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(sprite_sheet.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Initial vertex buffer (will be updated each frame with new UVs)
        let vertices = create_quad_vertices(0.0, 0.0, 1.0, 1.0);
        let vertex_buffer = graphics_ctx.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Box::new(App {
            _context: graphics_ctx,
            windows,
            pipeline,
            bind_group,
            vertex_buffer,
            uniform_buffer,
            sprite_sheet,
            animation,
            last_update: Instant::now(),
        })
    });
}

fn create_quad_vertices(u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> [Vertex; 6] {
    [
        Vertex { position: [-0.5, -0.5], uv: [u_min, v_max] },
        Vertex { position: [0.5, -0.5], uv: [u_max, v_max] },
        Vertex { position: [0.5, 0.5], uv: [u_max, v_min] },
        Vertex { position: [-0.5, -0.5], uv: [u_min, v_max] },
        Vertex { position: [0.5, 0.5], uv: [u_max, v_min] },
        Vertex { position: [-0.5, 0.5], uv: [u_min, v_min] },
    ]
}

impl astrelis_winit::app::App for App {
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx, _time: &astrelis_winit::FrameTime) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        // Update animation
        if self.animation.update(dt) {
            // Frame changed - update vertex buffer with new UVs
            let frame = self.animation.current_frame();
            let uv = self.sprite_sheet.sprite_uv(frame);
            let vertices = create_quad_vertices(uv.u_min, uv.v_min, uv.u_max, uv.v_max);
            
            // Get context from first window
            if let Some(window) = self.windows.values().next() {
                window.context().graphics_context().queue().write_buffer(
                    &self.vertex_buffer,
                    0,
                    bytemuck::cast_slice(&vertices),
                );
            }
        }
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        window_id: WindowId,
        events: &mut astrelis_winit::event::EventBatch,
    ) {
        let Some(window) = self.windows.get_mut(&window_id) else {
            return;
        };

        // Handle resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        let mut frame = window.begin_drawing();

        // Render with automatic scoping (no manual {} block needed)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::rgb(0.1, 0.1, 0.15),
            |pass| {
                let pass = pass.wgpu_pass();
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.draw(0..6, 0..1);
            },
        );

        frame.finish();
    }
}
