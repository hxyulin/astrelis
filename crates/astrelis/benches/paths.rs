//! Matched path recording against direct wgpu, plus explicit preparation costs.
//! Small RGBA8 target isolates CPU costs. Completion waits are outside timing.
use astrelis::{
    EdgeAntialiasing, Frame, Framebuffer, FramebufferOptions, GraphicsContext, LineCap, LineJoin,
    Painter, Path, PathDraw, PathOptions, PathRenderer, PathStroke, PreparedPath, Transform2D,
    wgpu,
};
use bytemuck::{Pod, Zeroable};
use std::{
    hint::black_box,
    sync::mpsc,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt as _;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SIZE: u32 = 64;
const WAIT: Duration = Duration::from_secs(30);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    incoming: [f32; 2],
    outgoing: [f32; 2],
    outer: f32,
    padding: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Parameters {
    axes: [f32; 4],
    translation_viewport: [f32; 4],
    color: [f32; 4],
    style: [f32; 4],
}
fn parameters(draw: PathDraw) -> Parameters {
    let [a, b, c, d, x, y] = draw.transform.0;
    Parameters {
        axes: [a, b, c, d],
        translation_viewport: [x, y, SIZE as f32, SIZE as f32],
        color: draw.color,
        style: [
            if draw.antialiasing == EdgeAntialiasing::Coverage {
                1.
            } else {
                0.
            },
            1.,
            0.,
            0.,
        ],
    }
}
#[derive(Clone, Copy)]
enum Mode {
    RawIndividual,
    Individual,
    Scoped,
    RawBatch,
    Batch,
    PainterBatch,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::RawIndividual => "raw_individual",
            Self::Individual => "individual",
            Self::Scoped => "scoped",
            Self::RawBatch => "raw_batch",
            Self::Batch => "batch",
            Self::PainterBatch => "painter_batch",
        }
    }
    fn reference(self) -> Self {
        match self {
            Self::Individual | Self::Scoped => Self::RawIndividual,
            _ => Self::RawBatch,
        }
    }
    fn raw(self) -> bool {
        matches!(self, Self::RawIndividual | Self::RawBatch)
    }
}
struct Work {
    renderer: PathRenderer,
    painter: Painter,
    path: PreparedPath,
    draws: Vec<PathDraw>,
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    pages: Vec<wgpu::Buffer>,
    no_clip: wgpu::Buffer,
    packed: Vec<Parameters>,
    coverage: bool,
}
impl Work {
    fn new(
        g: &GraphicsContext,
        target: &Framebuffer,
        count: usize,
        coverage: bool,
    ) -> Result<Self> {
        let mut builder = Path::builder();
        builder
            .move_to([0., 0.])
            .line_to([8., 0.])
            .line_to([8., 8.])
            .line_to([0., 8.])
            .close();
        let mut renderer = PathRenderer::new(g);
        let path = renderer.prepare_path(&builder.build()?, PathOptions::new())?;
        renderer.prepare(&target.render_format())?;
        let mut painter = Painter::new(g);
        painter.prepare(&target.render_format())?;
        let positions = [[0., 0.], [8., 0.], [8., 8.], [0., 8.]];
        let incoming = [[0., -1.], [1., 0.], [0., 1.], [-1., 0.]];
        let outgoing = [[1., 0.], [0., 1.], [-1., 0.], [0., -1.]];
        let mut vertices: Vec<_> = (0..4)
            .map(|i| Vertex {
                position: positions[i],
                incoming: incoming[i],
                outgoing: outgoing[i],
                outer: 0.,
                padding: 0.,
            })
            .collect();
        let mut indices = vec![0u32, 1, 2, 0, 2, 3];
        for a in 0..4u32 {
            let b = (a + 1) % 4;
            let outer_a = vertices.len() as u32;
            let mut v = vertices[a as usize];
            v.outer = 1.;
            vertices.push(v);
            let outer_b = vertices.len() as u32;
            let mut v = vertices[b as usize];
            v.outer = 1.;
            vertices.push(v);
            indices.extend([a, outer_a, b, b, outer_a, outer_b]);
        }
        assert_eq!(vertices.len(), path.vertex_count() as usize);
        assert_eq!(indices.len(), path.index_count() as usize);
        let vertices = g
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("raw matching path vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let indices = g
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("raw matching path indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let shader = g
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("raw matching path shader"),
                source: wgpu::ShaderSource::Wgsl(
                    concat!(
                        include_str!("../src/clip.wgsl"),
                        "\n",
                        include_str!("../src/path/path.wgsl")
                    )
                    .into(),
                ),
            });
        let attributes =
            wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2,3=>Float32];
        let instance_attributes =
            wgpu::vertex_attr_array![4=>Float32x4,5=>Float32x4,6=>Float32x4,7=>Float32x4];
        let clip_attributes = wgpu::vertex_attr_array![8=>Float32x4,9=>Float32x4,10=>Float32x4];
        // Disabled rounded clip: a negative half size turns clipping off.
        let no_clip = g
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("raw disabled clip"),
                contents: bytemuck::cast_slice(&[
                    0f32, 0., 0., 0., 0., 0., -1., -1., 0., 0., 0., 0.,
                ]),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let layout = g
            .device()
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        let pipeline = g
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("raw matching paths"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex_main"),
                    compilation_options: Default::default(),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 32,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &attributes,
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &instance_attributes,
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: 0,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &clip_attributes,
                        }),
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target.format(),
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
        let draws = (0..count)
            .map(|i| {
                PathDraw::new([0.2, 0.7, 0.4, 0.35])
                    .transform(Transform2D::scale(0.7, 0.8).then(Transform2D::translation(
                        (i % 8) as f32 * 8. + 0.25,
                        ((i / 8) % 8) as f32 * 8. + 0.5,
                    )))
                    .antialiasing(if coverage {
                        EdgeAntialiasing::Coverage
                    } else {
                        EdgeAntialiasing::None
                    })
            })
            .collect();
        let pages = (0..count.div_ceil(1024))
            .map(|_| {
                g.device().create_buffer(&wgpu::BufferDescriptor {
                    label: Some("raw reused path parameters"),
                    size: 65536,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        Ok(Self {
            renderer,
            painter,
            path,
            draws,
            pipeline,
            vertices,
            indices,
            pages,
            no_clip,
            packed: Vec::with_capacity(count),
            coverage,
        })
    }
    fn record(&mut self, mode: Mode, frame: &mut Frame<'_, '_>) -> Result<f64> {
        let mut pass = frame.render_pass().begin()?;
        pass.set_viewport(0., 0., SIZE as f32, SIZE as f32, 0., 1.)?;
        pass.set_scissor_rect(0, 0, SIZE, SIZE)?;
        let start = Instant::now();
        match mode {
            Mode::Individual => {
                for &draw in &self.draws {
                    self.renderer.draw(&mut pass, &self.path, black_box(draw))?;
                }
            }
            Mode::Scoped => {
                let mut scope = self.renderer.bind(&mut pass)?;
                for &draw in &self.draws {
                    scope.draw(&self.path, black_box(draw))?;
                }
            }
            Mode::Batch => {
                self.renderer
                    .draw_many(&mut pass, &self.path, black_box(&self.draws))?
            }
            Mode::PainterBatch => self
                .painter
                .begin(&mut pass)?
                .draw_paths(&self.path, black_box(&self.draws))?,
            Mode::RawIndividual | Mode::RawBatch => {
                self.packed.clear();
                let pass = pass.as_wgpu();
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_vertex_buffer(2, self.no_clip.slice(..));
                let indices = if self.coverage { 30 } else { 6 };
                if matches!(mode, Mode::RawIndividual) {
                    for (i, &draw) in self.draws.iter().enumerate() {
                        self.packed.push(parameters(black_box(draw)));
                        let offset = (i % 1024) as u64 * 64;
                        pass.set_vertex_buffer(1, self.pages[i / 1024].slice(offset..offset + 64));
                        pass.draw_indexed(0..indices, 0, 0..1);
                    }
                } else {
                    self.packed
                        .extend(self.draws.iter().map(|&d| parameters(black_box(d))));
                    for (i, chunk) in self.packed.chunks(1024).enumerate() {
                        pass.set_vertex_buffer(1, self.pages[i].slice(0..chunk.len() as u64 * 64));
                        pass.draw_indexed(0..indices, 0, 0..chunk.len() as u32);
                    }
                }
            }
        }
        Ok(start.elapsed().as_secs_f64() * 1e6)
    }
    fn upload(&self, g: &GraphicsContext, mode: Mode) {
        if mode.raw() {
            for (page, chunk) in self.pages.iter().zip(self.packed.chunks(1024)) {
                g.queue().write_buffer(page, 0, bytemuck::cast_slice(chunk));
            }
        }
    }
}
fn wait(g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    Ok(())
}
fn sample(
    g: &GraphicsContext,
    target: &mut Framebuffer,
    work: &mut Work,
    mode: Mode,
) -> Result<[f64; 2]> {
    let start = Instant::now();
    let mut frame = target.begin_frame()?;
    let record = work.record(mode, &mut frame)?;
    work.upload(g, mode);
    let index = frame.finish()?;
    let total = start.elapsed().as_secs_f64() * 1e6;
    wait(g, index)?;
    Ok([record, total])
}
fn pixels(
    g: &GraphicsContext,
    target: &mut Framebuffer,
    work: &mut Work,
    mode: Mode,
) -> Result<Vec<u8>> {
    let texture = target.color_texture()?.clone();
    let buffer = g.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut frame = target.begin_frame()?;
    work.record(mode, &mut frame)?;
    work.upload(g, mode);
    frame.encoder().copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        texture.size(),
    );
    let index = frame.finish()?;
    let (tx, rx) = mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    wait(g, index)?;
    rx.recv_timeout(WAIT)??;
    let bytes = buffer.slice(..).get_mapped_range()?.to_vec();
    buffer.unmap();
    Ok(bytes)
}
fn print(count: usize, mode: &str, coverage: bool, stage: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n.is_multiple_of(2) {
        (values[n / 2 - 1] + values[n / 2]) * 0.5
    } else {
        values[n / 2]
    };
    println!(
        "{count},{mode},{coverage},{stage},{median:.3},{:.3}",
        values[(n as f64 * 0.95).ceil() as usize - 1]
    );
}
fn prepare_cases(g: &GraphicsContext, samples: usize, warmup: usize) -> Result<()> {
    let mut renderer = PathRenderer::new(g);
    let mut cases = Vec::new();
    for segments in [16, 64, 256] {
        let mut builder = Path::builder();
        builder.move_to([0., 32.]);
        for i in 0..segments {
            let x = i as f32 * 4.;
            builder.cubic_to(
                [x + 1., if i % 2 == 0 { 0. } else { 64. }],
                [x + 3., if i % 2 == 0 { 64. } else { 0. }],
                [x + 4., 32.],
            );
        }
        let path = builder.build()?;
        for (name, options) in [
            ("prepare_fill", PathOptions::new()),
            (
                "prepare_stroke",
                PathOptions::new().stroke(
                    PathStroke::new(3.)
                        .join(LineJoin::Round)
                        .cap(LineCap::Round),
                ),
            ),
        ] {
            cases.push((segments, name, path.clone(), options));
        }
    }
    let mut builder = Path::builder();
    builder
        .move_to([50., 90.])
        .cubic_to([30., 75.], [0., 55.], [5., 30.])
        .cubic_to([10., 0.], [40., 5.], [50., 25.])
        .cubic_to([60., 5.], [90., 0.], [95., 30.])
        .cubic_to([100., 55.], [70., 75.], [50., 90.])
        .close();
    cases.push((4, "prepare_icon_fill", builder.build()?, PathOptions::new()));
    let mut builder = Path::builder();
    builder.move_to([0., 32.]);
    for i in 1..=128 {
        builder.line_to([i as f32 * 4., 32. + (i as f32 * 0.4).sin() * 12.]);
    }
    cases.push((
        128,
        "prepare_polyline_stroke",
        builder.build()?,
        PathOptions::new().stroke(
            PathStroke::new(3.)
                .join(LineJoin::Round)
                .cap(LineCap::Round),
        ),
    ));
    for (segments, name, path, options) in cases {
        let mut values = Vec::new();
        let mut counts = None;
        for i in 0..samples + warmup {
            let start = Instant::now();
            let prepared = renderer.prepare_path(black_box(&path), options)?;
            let elapsed = start.elapsed().as_secs_f64() * 1e6;
            counts = Some((
                prepared.vertex_count(),
                prepared.index_count(),
                prepared.geometry_bytes(),
            ));
            wait(g, g.queue().submit([]))?;
            drop(prepared);
            if i >= warmup {
                values.push(elapsed);
            }
        }
        eprintln!("{name} segments={segments}; vertices,indices,bytes={counts:?}");
        print(segments, name, true, "tessellate_and_upload", values);
    }
    Ok(())
}
fn main() -> Result<()> {
    let mut counts = vec![100, 1000, 10000];
    let mut samples = 40usize;
    let mut warmup = 8usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--counts" => {
                counts = args
                    .next()
                    .ok_or("missing counts")?
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?
            }
            "--samples" => samples = args.next().ok_or("missing samples")?.parse()?,
            "--warmup" => warmup = args.next().ok_or("missing warmup")?.parse()?,
            "--help" | "-h" => {
                println!(
                    "cargo bench -p astrelis --bench paths -- [--counts 100,1000,10000] [--samples 40] [--warmup 8]\nMatched direct-wgpu quad paths, alternating paired timings, full pixel equivalence. Preparation includes tessellation and allocation/upload, not GPU execution. Record excludes pass setup; cpu_total includes setup/uploads/submission. Completion waits excluded."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if cfg!(debug_assertions) {
        return Err("use cargo bench --bench paths".into());
    }
    if counts.is_empty()
        || counts.iter().any(|&n| n == 0 || n > 1_000_000)
        || !(5..=10000).contains(&samples)
        || !(1..=10000).contains(&warmup)
    {
        return Err("invalid counts/samples/warmup".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
    eprintln!(
        "adapter={:?}; counts={counts:?}; samples={samples}; warmup={warmup}; target=64x64 linear RGBA8 1xMSAA; gpu_execution=not_measured; reference=direct_wgpu; same_shader_and_layouts; reused_64KiB_parameter_pages; geometry=quad_2interior_8fringe_triangles",
        g.adapter().get_info()
    );
    println!("count,mode,coverage,stage,median_us,p95_us");
    prepare_cases(&g, samples, warmup)?;
    let mut target = g.create_framebuffer(
        FramebufferOptions::new(SIZE, SIZE)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )?;
    for count in counts {
        for coverage in [false, true] {
            let mut work = Work::new(&g, &target, count, coverage)?;
            for mode in [
                Mode::Individual,
                Mode::Scoped,
                Mode::Batch,
                Mode::PainterBatch,
            ] {
                let reference = mode.reference();
                let a = pixels(&g, &mut target, &mut work, reference)?;
                let b = pixels(&g, &mut target, &mut work, mode)?;
                if a != b || !a.iter().any(|&v| v != 0) {
                    return Err(format!(
                        "pixel equivalence failed: {} count={count} coverage={coverage}",
                        mode.name()
                    )
                    .into());
                }
                eprintln!(
                    "pixel equivalence passed: {} count={count} coverage={coverage}",
                    mode.name()
                );
                let mut raw = Vec::new();
                let mut wrapped = Vec::new();
                for i in 0..samples + warmup {
                    let order = if i % 2 == 0 {
                        [reference, mode]
                    } else {
                        [mode, reference]
                    };
                    let a = sample(&g, &mut target, &mut work, order[0])?;
                    let b = sample(&g, &mut target, &mut work, order[1])?;
                    let (a, b) = if i % 2 == 0 { (a, b) } else { (b, a) };
                    if i >= warmup {
                        raw.push(a);
                        wrapped.push(b);
                    }
                }
                for (name, index) in [("record", 0), ("cpu_total", 1)] {
                    print(
                        count,
                        &format!("{}_for_{}", reference.name(), mode.name()),
                        coverage,
                        name,
                        raw.iter().map(|v| v[index]).collect(),
                    );
                    print(
                        count,
                        mode.name(),
                        coverage,
                        name,
                        wrapped.iter().map(|v| v[index]).collect(),
                    );
                }
            }
        }
    }
    if let Some(error) = pollster::block_on(errors.pop()) {
        return Err(error.into());
    }
    Ok(())
}
