//! Triangle example.
//!
//! Renders a colored triangle using a vertex buffer and a simple WGSL shader.
//! Demonstrates: shader loading, vertex buffer creation, render pipeline
//! setup, and per-frame drawing.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-gpu --example triangle
//! ```

use astrelis_core::color::Color;
use astrelis_gpu::buffer::{BufferInitDescriptor, BufferUsages};
use astrelis_gpu::command::{ColorAttachment, RenderPassDescriptor};
use astrelis_gpu::pipeline::{
    ColorTargetState, FragmentState, MultisampleState, PrimitiveState, RenderPipelineDescriptor,
    VertexAttribute, VertexBufferLayout, VertexState,
};
use astrelis_gpu::resources::{Buffer, RenderPipeline};
use astrelis_gpu::shader::{ShaderModuleDescriptor, ShaderSource};
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::{
    BlendState, ColorWrites, LoadOp, PresentMode, StoreOp, TextureFormat, VertexFormat,
    VertexStepMode,
};
use astrelis_gpu::{Gpu, GpuConfig, GpuError};
use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;

/// WGSL shader for a colored triangle.
const SHADER_SRC: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
"#;

/// Interleaved vertex data: position (f32x2) + color (f32x3).
#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.5],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5],
        color: [0.0, 0.0, 1.0],
    },
];

struct App {
    window_id: Option<WindowId>,
    gpu: Option<Gpu>,
    surface: Option<astrelis_gpu::Surface<'static>>,
    pipeline: Option<RenderPipeline>,
    vertex_buffer: Option<Buffer>,
    surface_format: TextureFormat,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = WindowBuilder::new()
                    .with_title("Astrelis — Triangle")
                    .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                let gpu =
                    Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");
                println!(
                    "GPU: {} ({:?})",
                    gpu.device().adapter_info().name,
                    gpu.device().adapter_info().backend
                );

                let window = ctx.window(win_id).expect("window not found");
                let mut surface = gpu.create_surface(window).expect("failed to create surface");

                let size = window.inner_size().physical();
                self.surface_format = surface.preferred_format();
                let config = SurfaceConfiguration {
                    format: self.surface_format,
                    width: size.width as u32,
                    height: size.height as u32,
                    present_mode: PresentMode::AutoVsync,
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(&config);

                // Create shader module.
                let shader = gpu
                    .device()
                    .create_shader_module(&ShaderModuleDescriptor {
                        label: Some("triangle_shader"),
                        source: ShaderSource::Wgsl(SHADER_SRC),
                    })
                    .expect("failed to create shader module");

                // Create vertex buffer.
                let vertex_buffer = gpu
                    .device()
                    .create_buffer_init(&BufferInitDescriptor {
                        label: Some("triangle_vertices"),
                        contents: bytemuck::cast_slice(VERTICES),
                        usage: BufferUsages::VERTEX,
                    });

                // Create render pipeline.
                let pipeline = gpu
                    .device()
                    .create_render_pipeline(&RenderPipelineDescriptor {
                        label: Some("triangle_pipeline"),
                        layout: None,
                        vertex: VertexState {
                            module: &shader,
                            entry_point: "vs_main",
                            buffers: &[VertexBufferLayout {
                                array_stride: std::mem::size_of::<Vertex>() as u64,
                                step_mode: VertexStepMode::Vertex,
                                attributes: &[
                                    VertexAttribute {
                                        format: VertexFormat::Float32x2,
                                        offset: 0,
                                        shader_location: 0,
                                    },
                                    VertexAttribute {
                                        format: VertexFormat::Float32x3,
                                        offset: 8,
                                        shader_location: 1,
                                    },
                                ],
                            }],
                        },
                        primitive: PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: MultisampleState::default(),
                        fragment: Some(FragmentState {
                            module: &shader,
                            entry_point: "fs_main",
                            targets: &[ColorTargetState {
                                format: self.surface_format,
                                blend: Some(BlendState::REPLACE),
                                write_mask: ColorWrites::ALL,
                            }],
                        }),
                    });

                // SAFETY: surface lifetime is managed alongside gpu lifetime
                let surface: astrelis_gpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

                self.gpu = Some(gpu);
                self.surface = Some(surface);
                self.pipeline = Some(pipeline);
                self.vertex_buffer = Some(vertex_buffer);
                ctx.set_control_flow(ControlFlow::Wait);
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {
                println!("Goodbye!");
            }
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        astrelis_profiling::profile_function!();
        match event {
            WindowEvent::CloseRequested => ctx.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = &mut self.surface {
                    let phys = size.physical();
                    let w = phys.width as u32;
                    let h = phys.height as u32;
                    if w > 0 && h > 0 {
                        let config = SurfaceConfiguration {
                            format: self.surface_format,
                            width: w,
                            height: h,
                            present_mode: PresentMode::AutoVsync,
                            desired_maximum_frame_latency: 2,
                        };
                        surface.configure(&config);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.render();
                if let Some(win) = ctx.window(window_id) {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();
        if let Some(id) = self.window_id
            && let Some(win) = ctx.window(id)
        {
            win.request_redraw();
        }
    }
}

impl App {
    fn render(&mut self) {
        astrelis_profiling::profile_function!();
        let (Some(gpu), Some(surface), Some(pipeline), Some(vertex_buffer)) =
            (&self.gpu, &mut self.surface, &self.pipeline, &self.vertex_buffer)
        else {
            return;
        };

        // Process GPU profiling results from prior frames.
        gpu.process_profiling_frames();

        astrelis_profiling::profile_scope!("acquire");
        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost) => return,
            Err(GpuError::Timeout) => return,
            Err(e) => panic!("failed to acquire surface texture: {e}"),
        };

        astrelis_profiling::profile_scope!("encode");
        let mut encoder = gpu.device().create_command_encoder(Some("triangle"));
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("triangle_pass"),
                color_attachments: &[ColorAttachment {
                    view: frame.view(),
                    resolve_target: None,
                    load_op: LoadOp::Clear(Color::new(0.1, 0.1, 0.1, 1.0)),
                    store_op: StoreOp::Store,
                }],
                depth_stencil_attachment: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, vertex_buffer, 0, None);
            pass.draw(0..3, 0..1);
        }

        astrelis_profiling::profile_scope!("submit");
        gpu.submit(std::iter::once(encoder));
        astrelis_profiling::profile_scope!("present");
        frame.present();
    }
}

fn main() {
    astrelis_profiling::init();

    
    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        pipeline: None,
        vertex_buffer: None,
        surface_format: TextureFormat::Bgra8UnormSrgb,
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
