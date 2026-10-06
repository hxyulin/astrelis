//! Retained dense-data update/recording/GPU-pass costs against matched direct wgpu.
//! Completion waits are separate; the same point buffers, shader, and geometry are used.
use astrelis::{
    Framebuffer, FramebufferOptions, GraphicsContext, MarkerDraw, MarkerRenderer, Point2D,
    PointBuffer, PointBufferOptions, PolylineDraw, PolylineRenderer, RenderPass,
    RenderPassDescriptor, Transform2D, wgpu,
};
use bytemuck::{Pod, Zeroable};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const WAIT: Duration = Duration::from_secs(30);
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Parameters {
    axes: [f32; 4],
    placement: [f32; 4],
    color: [f32; 4],
    style: [f32; 4],
    range: [u32; 4],
}
#[derive(Clone, Copy)]
enum Mode {
    Unchanged,
    Append,
    Replace,
    Visible,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Append => "append_256",
            Self::Replace => "replace_all",
            Self::Visible => "visible_4096",
        }
    }
}
struct Timer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}
impl Timer {
    fn new(g: &GraphicsContext) -> Option<Self> {
        g.device()
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Self {
                queries: g.device().create_query_set(&wgpu::QuerySetDescriptor {
                    label: None,
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                resolve: g.device().create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                read: g.device().create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            })
    }
}
struct Work {
    points: PointBuffer,
    data: Vec<Point2D>,
    tail: Vec<Point2D>,
    next: usize,
    lines: PolylineRenderer,
    markers: MarkerRenderer,
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    parameters: wgpu::Buffer,
    marker: bool,
    packed: Option<Parameters>,
}
impl Work {
    fn new(g: &GraphicsContext, t: &Framebuffer, count: usize, marker: bool) -> Result<Self> {
        let data: Vec<_> = (0..count).map(sample).collect();
        let mut points = g.create_point_buffer(PointBufferOptions::new(count).ring())?;
        points.replace(&data)?;
        let mut lines = PolylineRenderer::new(g);
        let mut markers = MarkerRenderer::new(g);
        lines.prepare(&t.render_format())?;
        markers.prepare(&t.render_format())?;
        let layout = g
            .device()
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(8),
                    },
                    count: None,
                }],
            });
        let group = g.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: points.as_wgpu().as_entire_binding(),
            }],
        });
        let shader = g
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Matched dense points"),
                source: wgpu::ShaderSource::Wgsl(include_str!("../src/points.wgsl").into()),
            });
        let pipeline_layout = g
            .device()
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let attrs = wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4,3=>Float32x4,4=>Uint32x4];
        let pipeline = g
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(if marker {
                        "marker_vertex"
                    } else {
                        "polyline_vertex"
                    }),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 80,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &attrs,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        let parameters = g.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 80,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            points,
            data,
            tail: Vec::with_capacity(256),
            next: count,
            lines,
            markers,
            pipeline,
            group,
            parameters,
            marker,
            packed: None,
        })
    }
    fn update(&mut self, mode: Mode) -> Result<u64> {
        let before = self.points.uploaded_bytes();
        match mode {
            Mode::Append => {
                self.tail.clear();
                self.tail.extend((self.next..self.next + 256).map(sample));
                self.next += 256;
                self.points.append(&self.tail)?;
            }
            Mode::Replace => {
                self.points.replace(&self.data)?;
                self.next = self.data.len();
            }
            _ => {}
        }
        Ok(self.points.uploaded_bytes() - before)
    }
    fn draw(&mut self, p: &mut RenderPass<'_>, raw: bool, mode: Mode) -> Result<()> {
        let count = if matches!(mode, Mode::Visible) {
            self.points.len().min(4096)
        } else {
            self.points.len()
        };
        let start = self.points.len() - count;
        let x = (self.next - self.points.len() + start) as f64;
        let sx = (WIDTH - 64) as f64 / (count - 1) as f64;
        let transform = Transform2D([sx as f32, 0., 0., -480., (32. - x * sx) as f32, 540.]);
        let color = [0.1, 0.7, 1., 1.];
        if raw {
            let [a, b, c, d, tx, ty] = transform.0;
            let stride = if self.marker { 6 } else { 9 };
            let data = Parameters {
                axes: [a, b, c, d],
                placement: [tx, ty, WIDTH as f32, HEIGHT as f32],
                color,
                style: [1.5, if self.marker { 0. } else { 4. }, 1., 0.],
                range: [
                    ((self.points.physical_start() + start) % self.points.capacity()) as u32,
                    count as u32,
                    self.points.capacity() as u32,
                    stride,
                ],
            };
            self.packed = Some(data);
            let p = p.as_wgpu();
            p.set_pipeline(&self.pipeline);
            p.set_bind_group(0, &self.group, &[]);
            p.set_vertex_buffer(0, self.parameters.slice(..));
            p.draw(0..(count as u32 - u32::from(!self.marker)) * stride, 0..1);
        } else if self.marker {
            self.markers.draw(
                p,
                &self.points,
                MarkerDraw::new(color)
                    .radius_pixels(1.5)
                    .transform(transform)
                    .range(start..self.points.len()),
            )?;
        } else {
            self.lines.draw(
                p,
                &self.points,
                PolylineDraw::new(color)
                    .width_pixels(1.5)
                    .transform(transform)
                    .range(start..self.points.len()),
            )?;
        }
        Ok(())
    }
}
fn sample(i: usize) -> Point2D {
    Point2D::new([
        i as f32,
        (i as f32 * 0.013).sin() * 0.7 + (i as f32 * 0.031).sin() * 0.12,
    ])
}
struct Timing {
    update: f64,
    record: f64,
    total: f64,
    completed: f64,
    gpu: Option<f64>,
    bytes: u64,
}
fn run(
    g: &GraphicsContext,
    t: &mut Framebuffer,
    w: &mut Work,
    timer: Option<&Timer>,
    raw: bool,
    mode: Mode,
    update: bool,
) -> Result<Timing> {
    let start = Instant::now();
    let bytes = if update { w.update(mode)? } else { 0 };
    let update = start.elapsed().as_secs_f64() * 1e6;
    let mut frame = t.begin_frame()?;
    let colors = [Some(frame.color_attachment()?)];
    let record;
    {
        let mut p = frame.begin_render_pass(&RenderPassDescriptor {
            colors: &colors,
            timestamp_writes: timer.map(|t| wgpu::RenderPassTimestampWrites {
                query_set: &t.queries,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
            ..Default::default()
        })?;
        let begin = Instant::now();
        w.draw(&mut p, raw, mode)?;
        record = begin.elapsed().as_secs_f64() * 1e6;
    }
    // Both routes pack during recording and queue parameter uploads afterwards.
    if let Some(data) = w.packed.take() {
        g.queue()
            .write_buffer(&w.parameters, 0, bytemuck::bytes_of(&data));
    }
    if let Some(t) = timer {
        frame
            .encoder()
            .resolve_query_set(&t.queries, 0..2, &t.resolve, 0);
        frame
            .encoder()
            .copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, 16);
    }
    let index = frame.finish()?;
    let total = start.elapsed().as_secs_f64() * 1e6;
    let rx = timer.map(|t| {
        let (tx, rx) = std::sync::mpsc::channel();
        t.read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        rx
    });
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    let completed = start.elapsed().as_secs_f64() * 1e6;
    let gpu = if let Some(t) = timer {
        rx.unwrap().recv_timeout(WAIT)??;
        let ticks = bytemuck::cast_slice::<u8, u64>(&t.read.slice(..).get_mapped_range()?).to_vec();
        t.read.unmap();
        let elapsed = ticks[1]
            .checked_sub(ticks[0])
            .map(|delta| delta as f64 * f64::from(g.queue().get_timestamp_period()) / 1000.);
        // An advertised feature does not guarantee functioning driver counters.
        // Never turn reversed/zero timestamps into a plausible measurement.
        match elapsed {
            Some(us) if ticks[0] != 0 && us > 0. && us <= completed * 1.1 => Some(us),
            _ => {
                eprintln!(
                    "rejected GPU timestamps: begin={}, end={}, completed_us={completed:.3}",
                    ticks[0], ticks[1]
                );
                None
            }
        }
    } else {
        None
    };
    Ok(black_box(Timing {
        update,
        record,
        total,
        completed,
        gpu,
        bytes,
    }))
}
fn pixels(g: &GraphicsContext, t: &mut Framebuffer) -> Result<Vec<u8>> {
    let texture = t.color_texture()?.clone();
    let stride = WIDTH * 4;
    let b = g.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride) * u64::from(HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut f = t.begin_frame()?;
    f.encoder().copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &b,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(HEIGHT),
            },
        },
        texture.size(),
    );
    let index = f.finish()?;
    let (tx, rx) = std::sync::mpsc::channel();
    b.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    rx.recv_timeout(WAIT)??;
    let result = b.slice(..).get_mapped_range()?.to_vec();
    b.unmap();
    Ok(result)
}
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n.is_multiple_of(2) {
        (values[n / 2 - 1] + values[n / 2]) * 0.5
    } else {
        values[n / 2]
    }
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("run with cargo bench --bench points".into());
    }
    let mut samples = 15;
    let mut counts = vec![10_000usize, 100_000, 1_000_000];
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bench" => {}
            "--samples" => samples = args.next().ok_or("missing samples")?.parse()?,
            "--counts" => {
                counts = args
                    .next()
                    .ok_or("missing counts")?
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?
            }
            _ => return Err(format!("unknown argument {a}").into()),
        }
    }
    if samples < 5 || counts.is_empty() || counts.iter().any(|n| *n < 256 || *n > 1_000_000) {
        return Err("samples >=5; counts 256..=1000000".into());
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: features,
        ..Default::default()
    }))?;
    let g = GraphicsContext::from_wgpu(instance, adapter, device, queue);
    let mut timer = Timer::new(&g);
    eprintln!(
        "adapter={:?}; samples={samples}; warmup=4; GPU pass timestamp feature={}; target=1920x1080 RGBA8 1x",
        g.adapter().get_info(),
        timer.is_some()
    );
    let mut t = g.create_framebuffer(
        FramebufferOptions::new(WIDTH, HEIGHT)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )?;
    if timer.is_some() {
        let mut calibration = Work::new(&g, &t, 10_000, false)?;
        for _ in 0..3 {
            if run(
                &g,
                &mut t,
                &mut calibration,
                timer.as_ref(),
                false,
                Mode::Unchanged,
                false,
            )?
            .gpu
            .is_none()
            {
                timer = None;
                eprintln!(
                    "GPU timestamps disabled after failed calibration; completed_frame_us is synchronized CPU wall time, not GPU pass time"
                );
                break;
            }
        }
    }
    eprintln!("GPU pass timestamps calibrated={}", timer.is_some());
    println!(
        "renderer,count,mode,route,update_us,record_us,cpu_total_us,completed_frame_us,gpu_pass_us,point_upload_bytes,parameter_bytes,rendered_points,draws"
    );
    for count in counts {
        for marker in [false, true] {
            let mut w = Work::new(&g, &t, count, marker)?;
            let name = if marker { "markers" } else { "polyline" };
            let buffer = w.points.as_wgpu().clone();
            for mode in [Mode::Unchanged, Mode::Append, Mode::Replace, Mode::Visible] {
                run(&g, &mut t, &mut w, timer.as_ref(), true, mode, false)?;
                let a = pixels(&g, &mut t)?;
                run(&g, &mut t, &mut w, timer.as_ref(), false, mode, false)?;
                let b = pixels(&g, &mut t)?;
                assert_eq!(a, b, "{name} {count} {}", mode.name());
                assert!(a.iter().any(|b| *b != 0));
                eprintln!("pixel equivalence passed: {name} {count} {}", mode.name());
                let (mut raw, mut wrapped) = (Vec::new(), Vec::new());
                for i in 0..samples + 4 {
                    let order = if i % 2 == 0 {
                        [true, false]
                    } else {
                        [false, true]
                    };
                    for reference in order {
                        let timing =
                            run(&g, &mut t, &mut w, timer.as_ref(), reference, mode, true)?;
                        if i >= 4 {
                            if reference {
                                raw.push(timing)
                            } else {
                                wrapped.push(timing)
                            }
                        }
                    }
                }
                assert_eq!(w.points.as_wgpu(), &buffer);
                for (route, data) in [("raw", raw), ("astrelis", wrapped)] {
                    let gpu = if timer.is_some() && data.iter().all(|t| t.gpu.is_some()) {
                        format!("{:.3}", median(data.iter().filter_map(|t| t.gpu).collect()))
                    } else {
                        String::new()
                    };
                    let bytes = data[0].bytes;
                    assert!(data.iter().all(|t| t.bytes == bytes));
                    println!(
                        "{name},{count},{},{route},{:.3},{:.3},{:.3},{:.3},{gpu},{bytes},80,{},1",
                        mode.name(),
                        median(data.iter().map(|t| t.update).collect()),
                        median(data.iter().map(|t| t.record).collect()),
                        median(data.iter().map(|t| t.total).collect()),
                        median(data.iter().map(|t| t.completed).collect()),
                        if matches!(mode, Mode::Visible) {
                            count.min(4096)
                        } else {
                            count
                        }
                    );
                }
            }
        }
    }
    Ok(())
}
