//! CPU recording experiments for primitives and an ordered mixed-renderer workload.
//! References use direct Astrelis renderer calls; this is not a direct-wgpu benchmark.
use astrelis::{
    Frame, Framebuffer, FramebufferOptions, GraphicsContext, LineCap, LineDraw, LineRenderer, Mesh,
    MeshRenderer, Painter, Rect, ShapeDraw, ShapeRenderer, Stroke, TextureBinding,
    TextureBindingOptions, TextureDraw, TextureOptions, TextureRenderer, Vertex, wgpu,
};
use std::{
    error::Error as StdError,
    hint::black_box,
    sync::mpsc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn StdError>>;
const SIZE: u32 = 64;
const WAIT: Duration = Duration::from_secs(30);
#[derive(Clone, Copy)]
enum Mode {
    Shapes,
    ShapesScoped,
    ShapesBatch,
    Lines,
    LinesScoped,
    LinesBatch,
    Mixed,
    MixedScoped,
    PainterMixed,
    PainterShapesBatch,
    PainterLinesBatch,
    Outlines,
    OutlinesScoped,
    OutlinesBatch,
    PainterOutlinesBatch,
}
impl Mode {
    const CASES: [Self; 11] = [
        Self::ShapesScoped,
        Self::ShapesBatch,
        Self::LinesScoped,
        Self::LinesBatch,
        Self::MixedScoped,
        Self::PainterMixed,
        Self::PainterShapesBatch,
        Self::PainterLinesBatch,
        Self::OutlinesScoped,
        Self::OutlinesBatch,
        Self::PainterOutlinesBatch,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Shapes => "shapes_individual",
            Self::ShapesScoped => "shapes_scoped",
            Self::ShapesBatch => "shapes_batch",
            Self::Lines => "lines_individual",
            Self::LinesScoped => "lines_scoped",
            Self::LinesBatch => "lines_batch",
            Self::Mixed => "mixed_individual",
            Self::MixedScoped => "mixed_scoped",
            Self::PainterMixed => "painter_mixed",
            Self::PainterShapesBatch => "painter_shapes_batch",
            Self::PainterLinesBatch => "painter_lines_batch",
            Self::Outlines => "fills_outlines_individual",
            Self::OutlinesScoped => "fills_outlines_scoped",
            Self::OutlinesBatch => "fills_outlines_batch",
            Self::PainterOutlinesBatch => "painter_fills_outlines_batch",
        }
    }
    fn reference(self) -> Self {
        match self {
            Self::ShapesScoped | Self::ShapesBatch => Self::Shapes,
            Self::LinesScoped | Self::LinesBatch => Self::Lines,
            Self::PainterShapesBatch => Self::ShapesBatch,
            Self::PainterLinesBatch => Self::LinesBatch,
            Self::OutlinesScoped | Self::OutlinesBatch => Self::Outlines,
            Self::PainterOutlinesBatch => Self::OutlinesBatch,
            _ => Self::Mixed,
        }
    }
}
struct Work {
    painter: Painter,
    shapes: ShapeRenderer,
    lines: LineRenderer,
    textures: TextureRenderer,
    meshes: MeshRenderer,
    image: TextureBinding,
    mesh: Mesh,
    rectangles: Vec<ShapeDraw>,
    segments: Vec<LineDraw>,
    placements: Vec<TextureDraw>,
    markers: Vec<ShapeDraw>,
    outlines: Vec<ShapeDraw>,
}
impl Work {
    fn new(g: &GraphicsContext, target: &Framebuffer, count: usize) -> Result<Self> {
        let mut shapes = ShapeRenderer::new(g);
        let mut lines = LineRenderer::new(g);
        let mut textures = TextureRenderer::new(g);
        let mut meshes = MeshRenderer::new(g);
        let mut painter = Painter::new(g);
        painter.prepare(&target.render_format())?;
        shapes.prepare(&target.render_format())?;
        lines.prepare(&target.render_format())?;
        meshes.prepare(&target.render_format())?;
        let texture = g.create_texture(TextureOptions::new(1, 1))?;
        texture.write(&[255; 4])?;
        let image = textures.create_binding(texture.view(), TextureBindingOptions::new())?;
        textures.prepare(&image, &target.render_format())?;
        painter.prepare_image(&image, &target.render_format())?;
        let mesh = g.create_mesh(
            &[
                Vertex::new([-0.95, 0.95, 0.], [0.4, 0.9, 0.7, 0.1]),
                Vertex::new([-0.75, 0.95, 0.], [0.4, 0.9, 0.7, 0.1]),
                Vertex::new([-0.85, 0.75, 0.], [0.4, 0.9, 0.7, 0.1]),
            ],
            &[0, 1, 2],
        )?;
        let mut rectangles = Vec::with_capacity(count);
        let mut segments = Vec::with_capacity(count);
        let mut placements = Vec::with_capacity(count);
        let mut markers = Vec::with_capacity(count);
        let mut outlines = Vec::with_capacity(count);
        for i in 0..count {
            let x = (i % 8) as f32 * 8.;
            let y = ((i / 8) % 8) as f32 * 8.;
            rectangles.push(ShapeDraw::rounded_rect(
                Rect::new(x + 0.5, y + 0.5, 7., 7.),
                1.5,
                [0.1, 0.3, 0.7, 0.6],
            ));
            segments.push(
                LineDraw::new([x + 1., y + 6.], [x + 6., y + 1.], [0.2, 0.8, 0.4, 0.7])
                    .width(1.5)
                    .cap(LineCap::Round),
            );
            placements.push(
                TextureDraw::new(Rect::new(x + 1., y + 1., 4., 4.)).tint([0.8, 0.4, 0.1, 0.4]),
            );
            markers.push(ShapeDraw::ellipse(
                Rect::new(x + 3., y + 2., 4., 4.),
                [0.9, 0.2, 0.3, 0.4],
            ));
            let rect = Rect::new(x + 1.5, y + 1.5, 5., 4.);
            let color = [0.3, 0.6, 0.9, 0.5];
            let draw = match i % 3 {
                0 => ShapeDraw::rect(rect, color),
                1 => ShapeDraw::rounded_rect(rect, 1.5, color),
                _ => ShapeDraw::ellipse(rect, color),
            };
            let stroke = match (i / 3) % 3 {
                0 => Stroke::new(0.75).inside(),
                1 => Stroke::new(0.75),
                _ => Stroke::new(0.75).outside(),
            };
            outlines.push(if (i / 9) % 2 == 0 {
                draw.stroke(stroke)
            } else {
                draw
            });
        }
        Ok(Self {
            painter,
            shapes,
            lines,
            textures,
            meshes,
            image,
            mesh,
            rectangles,
            segments,
            placements,
            markers,
            outlines,
        })
    }
    fn record(&mut self, mode: Mode, frame: &mut Frame<'_, '_>) -> Result<f64> {
        let mut p = frame.render_pass().begin()?;
        let start = Instant::now();
        match mode {
            Mode::Shapes => {
                for &d in &self.rectangles {
                    self.shapes.draw(&mut p, black_box(d))?;
                }
            }
            Mode::ShapesScoped => {
                let mut s = self.shapes.bind(&mut p)?;
                for &d in &self.rectangles {
                    s.draw(black_box(d))?;
                }
            }
            Mode::ShapesBatch => self.shapes.draw_many(&mut p, black_box(&self.rectangles))?,
            Mode::Lines => {
                for &d in &self.segments {
                    self.lines.draw(&mut p, black_box(d))?;
                }
            }
            Mode::LinesScoped => {
                let mut s = self.lines.bind(&mut p)?;
                for &d in &self.segments {
                    s.draw(black_box(d))?;
                }
            }
            Mode::LinesBatch => self.lines.draw_many(&mut p, black_box(&self.segments))?,
            Mode::Mixed => {
                for i in 0..self.rectangles.len() {
                    p.set_scissor_rect(4, 4, 56, 56)?;
                    self.shapes.draw(&mut p, self.rectangles[i])?;
                    self.textures
                        .draw(&mut p, &self.image, self.placements[i])?;
                    self.shapes.draw(&mut p, self.markers[i])?;
                    self.lines.draw(&mut p, self.segments[i])?;
                    if i % 16 == 0 {
                        self.meshes.draw(&mut p, &self.mesh)?;
                    }
                }
            }
            Mode::MixedScoped => {
                let mut s = self.shapes.bind(&mut p)?;
                for i in 0..self.rectangles.len() {
                    s.pass().set_scissor_rect(4, 4, 56, 56)?;
                    s.draw(self.rectangles[i])?;
                    self.textures
                        .draw(s.pass(), &self.image, self.placements[i])?;
                    s.draw(self.markers[i])?;
                    self.lines.draw(s.pass(), self.segments[i])?;
                    if i % 16 == 0 {
                        self.meshes.draw(s.pass(), &self.mesh)?;
                    }
                }
            }
            Mode::PainterShapesBatch => self
                .painter
                .begin(&mut p)?
                .draw_shapes(black_box(&self.rectangles))?,
            Mode::PainterLinesBatch => self
                .painter
                .begin(&mut p)?
                .draw_lines(black_box(&self.segments))?,
            Mode::PainterMixed => {
                let mut paint = self.painter.begin(&mut p)?;
                for i in 0..self.rectangles.len() {
                    paint.pass().set_scissor_rect(4, 4, 56, 56)?;
                    paint.draw_shape(self.rectangles[i])?;
                    paint.draw_image(&self.image, self.placements[i])?;
                    paint.draw_shape(self.markers[i])?;
                    paint.draw_line(self.segments[i])?;
                    if i % 16 == 0 {
                        self.meshes.draw(paint.pass(), &self.mesh)?;
                    }
                }
            }
            Mode::Outlines => {
                for &d in &self.outlines {
                    self.shapes.draw(&mut p, black_box(d))?;
                }
            }
            Mode::OutlinesScoped => {
                let mut s = self.shapes.bind(&mut p)?;
                for &d in &self.outlines {
                    s.draw(black_box(d))?;
                }
            }
            Mode::OutlinesBatch => self.shapes.draw_many(&mut p, black_box(&self.outlines))?,
            Mode::PainterOutlinesBatch => self
                .painter
                .begin(&mut p)?
                .draw_shapes(black_box(&self.outlines))?,
        }
        Ok(start.elapsed().as_secs_f64() * 1e6)
    }
}
#[derive(Clone, Copy)]
struct Sample {
    record: f64,
    total: f64,
}
fn sample(
    g: &GraphicsContext,
    target: &mut Framebuffer,
    work: &mut Work,
    mode: Mode,
) -> Result<Sample> {
    let start = Instant::now();
    let mut frame = target.begin_frame()?;
    let record = work.record(mode, &mut frame)?;
    let index = frame.finish()?;
    let total = start.elapsed().as_secs_f64() * 1e6;
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    Ok(Sample { record, total })
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
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    rx.recv_timeout(WAIT)??;
    let data = buffer.slice(..).get_mapped_range()?.to_vec();
    buffer.unmap();
    Ok(data)
}
fn stats(mut values: Vec<f64>) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n.is_multiple_of(2) {
        (values[n / 2 - 1] + values[n / 2]) * 0.5
    } else {
        values[n / 2]
    };
    (median, values[(n as f64 * 0.95).ceil() as usize - 1])
}
fn main() -> Result<()> {
    let mut counts = vec![100, 1000, 10000];
    let mut samples = 40usize;
    let mut warmup = 8usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--help" | "-h" => {
                println!(
                    "cargo bench -p astrelis --bench primitives -- [--counts 100,1000,10000] [--samples 40] [--warmup 8]\nCPU record and total, no completion waits/presentation/GPU timing. Alternating paired samples.\nPrimitive references use individual Astrelis draws; Painter references use matching direct renderer calls. Batches reduce GPU draw count; scopes retain it."
                );
                return Ok(());
            }
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
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if cfg!(debug_assertions) {
        return Err("run in release mode with cargo bench".into());
    }
    if counts.is_empty()
        || counts.iter().any(|&n| n == 0 || n > 1000000)
        || !(5..=10000).contains(&samples)
        || !(1..=10000).contains(&warmup)
    {
        return Err("counts 1..=1000000, samples 5..=10000, warmup 1..=10000 required".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "adapter={:?}; samples={samples}; warmup={warmup}; counts={counts:?}; gpu_execution_timing=not_measured; reference=direct_astrelis_renderers",
        g.adapter().get_info()
    );
    let mut target = g.create_framebuffer(
        FramebufferOptions::new(SIZE, SIZE)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )?;
    println!(
        "count,workload,side,record_median_us,record_p95_us,cpu_total_median_us,cpu_total_p95_us"
    );
    for count in counts {
        let mut work = Work::new(&g, &target, count)?;
        for mode in Mode::CASES {
            let reference = mode.reference();
            let a = pixels(&g, &mut target, &mut work, reference)?;
            let b = pixels(&g, &mut target, &mut work, mode)?;
            if a != b || !a.iter().any(|&v| v != 0) {
                return Err(
                    format!("pixel equivalence failed: {} count={count}", mode.name()).into(),
                );
            }
            eprintln!("pixel equivalence passed: {} count={count}", mode.name());
            for _ in 0..warmup {
                sample(&g, &mut target, &mut work, reference)?;
                sample(&g, &mut target, &mut work, mode)?;
            }
            let mut reference_samples = Vec::new();
            let mut variant_samples = Vec::new();
            for i in 0..samples {
                let order = if i % 2 == 0 {
                    [reference, mode]
                } else {
                    [mode, reference]
                };
                let first = sample(&g, &mut target, &mut work, order[0])?;
                let second = sample(&g, &mut target, &mut work, order[1])?;
                let (a, b) = if i % 2 == 0 {
                    (first, second)
                } else {
                    (second, first)
                };
                reference_samples.push(a);
                variant_samples.push(b);
            }
            for (side, values) in [
                ("reference", reference_samples),
                ("variant", variant_samples),
            ] {
                let (r, rp) = stats(values.iter().map(|s| s.record).collect());
                let (t, tp) = stats(values.iter().map(|s| s.total).collect());
                println!(
                    "{count},{},{side},{r:.3},{rp:.3},{t:.3},{tp:.3}",
                    mode.name()
                );
            }
        }
    }
    Ok(())
}
