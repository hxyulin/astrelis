//! Example demonstrating image blitting (CPU-side pixel manipulation and GPU upload).
//!
//! This example shows how to:
//! - Create textures in memory
//! - Manipulate pixels on the CPU
//! - Upload texture data to the GPU
//! - Render textured quads
//!
//! This pattern is useful for:
//! - Procedural texture generation
//! - Dynamic image manipulation
//! - Software rendering to texture
//! - Animated sprites/textures

use astrelis_core::logging;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor,
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

/// WGSL shader for rendering textured quads
const SHADER: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    tint: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

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
    let tex_color = textureSample(tex, tex_sampler, in.uv);
    return tex_color * uniforms.tint;
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
    tint: [f32; 4],
}

/// A CPU-side image buffer that can be blitted to GPU.
struct ImageBuffer {
    width: u32,
    height: u32,
    pixels: Vec<u8>, // RGBA8
}

impl ImageBuffer {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    /// Clear to a solid color.
    fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    /// Set a pixel at (x, y).
    fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.pixels[idx] = r;
            self.pixels[idx + 1] = g;
            self.pixels[idx + 2] = b;
            self.pixels[idx + 3] = a;
        }
    }

    /// Draw a filled rectangle.
    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_pixel(x + dx, y + dy, r, g, b, a);
            }
        }
    }

    /// Draw a circle using midpoint algorithm.
    fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, r: u8, g: u8, b: u8, a: u8) {
        for y in (cy - radius)..=(cy + radius) {
            for x in (cx - radius)..=(cx + radius) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= radius * radius {
                    if x >= 0 && y >= 0 {
                        self.set_pixel(x as u32, y as u32, r, g, b, a);
                    }
                }
            }
        }
    }

    /// Draw a horizontal gradient.
    fn gradient_h(&mut self, y: u32, h: u32, r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) {
        for dy in 0..h {
            for x in 0..self.width {
                let t = x as f32 / self.width as f32;
                let r = (r1 as f32 * (1.0 - t) + r2 as f32 * t) as u8;
                let g = (g1 as f32 * (1.0 - t) + g2 as f32 * t) as u8;
                let b = (b1 as f32 * (1.0 - t) + b2 as f32 * t) as u8;
                self.set_pixel(x, y + dy, r, g, b, 255);
            }
        }
    }
}

struct App {
    context: Arc<GraphicsContext>,
    windows: HashMap<WindowId, RenderableWindow>,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    image_buffer: ImageBuffer,
    start_time: Instant,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut windows = HashMap::new();

        let scale = Window::platform_dpi() as f32;
        let window = ctx
            .create_window(WindowDescriptor {
                title: "Image Blitting Example".to_string(),
                size: Some(WinitPhysicalSize::new(800.0 * scale, 600.0 * scale)),
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

        // Create shader module
        let shader = graphics_ctx.device().create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        // Create bind group layout
        let bind_group_layout = graphics_ctx.device().create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blit Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = graphics_ctx.device().create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
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
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create vertex buffer for a fullscreen quad
        let vertices = [
            Vertex { position: [-0.8, -0.8], uv: [0.0, 1.0] },
            Vertex { position: [0.8, -0.8], uv: [1.0, 1.0] },
            Vertex { position: [0.8, 0.8], uv: [1.0, 0.0] },
            Vertex { position: [-0.8, -0.8], uv: [0.0, 1.0] },
            Vertex { position: [0.8, 0.8], uv: [1.0, 0.0] },
            Vertex { position: [-0.8, 0.8], uv: [0.0, 0.0] },
        ];
        let vertex_buffer = graphics_ctx.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create uniform buffer
        let uniforms = Uniforms {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            tint: [1.0, 1.0, 1.0, 1.0],
        };
        let uniform_buffer = graphics_ctx.device().create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create CPU-side image buffer
        let mut image_buffer = ImageBuffer::new(256, 256);
        image_buffer.clear(30, 30, 40, 255);

        // Create GPU texture
        let texture = graphics_ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Blit Texture"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create texture view and sampler
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = graphics_ctx.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blit Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest, // Pixel-perfect rendering
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group
        let bind_group = graphics_ctx.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Box::new(App {
            context: graphics_ctx,
            windows,
            pipeline,
            bind_group_layout,
            vertex_buffer,
            texture,
            bind_group,
            uniform_buffer,
            image_buffer,
            start_time: Instant::now(),
        })
    });
}

impl astrelis_winit::app::App for App {
    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx, _time: &astrelis_winit::FrameTime) {
        let time = self.start_time.elapsed().as_secs_f32();
        
        // Animate the CPU-side image buffer
        self.image_buffer.clear(30, 30, 40, 255);
        
        // Draw animated gradient background
        let phase = (time * 0.5).sin() * 0.5 + 0.5;
        let r1 = (50.0 + phase * 50.0) as u8;
        let b1 = (80.0 + (1.0 - phase) * 50.0) as u8;
        self.image_buffer.gradient_h(0, 256, r1, 40, b1, 40, r1, b1);
        
        // Draw bouncing circles
        for i in 0..5 {
            let offset = i as f32 * 0.4;
            let x = 128.0 + (time * 2.0 + offset).sin() * 80.0;
            let y = 128.0 + (time * 3.0 + offset).cos() * 80.0;
            let hue = (time * 0.5 + offset) % 1.0;
            let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);
            self.image_buffer.fill_circle(x as i32, y as i32, 20, r, g, b, 255);
        }
        
        // Draw animated rectangles
        for i in 0..3 {
            let x = ((time * (1.0 + i as f32 * 0.3)).sin() * 100.0 + 128.0) as u32;
            let y = 20 + i * 80;
            let w = 30 + (time.sin() * 10.0) as u32;
            let h = 20;
            self.image_buffer.fill_rect(x.saturating_sub(w/2), y, w, h, 255, 255, 255, 200);
        }
        
        // Upload to GPU (this is the "blit" operation)
        self.context.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.image_buffer.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.image_buffer.width * 4),
                rows_per_image: Some(self.image_buffer.height),
            },
            wgpu::Extent3d {
                width: self.image_buffer.width,
                height: self.image_buffer.height,
                depth_or_array_layers: 1,
            },
        );
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
            Color::rgb(0.05, 0.05, 0.08),
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

/// Convert HSV to RGB (h in [0,1], s in [0,1], v in [0,1])
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h * 6.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
