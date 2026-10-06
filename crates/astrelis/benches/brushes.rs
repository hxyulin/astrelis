//! CPU cost of explicit retained brushes versus ordinary color-only drawing.
//! Both use Astrelis frame/pass scaffolding. GPU completion is outside timing.
use astrelis::{
    Brush, BrushOptions, EdgeAntialiasing, Framebuffer, FramebufferOptions, GradientStop,
    GraphicsContext, LineDraw, LineRenderer, Path, PathDraw, PathOptions, PathRenderer,
    PreparedPath, Rect, ShapeDraw, ShapeRenderer, Transform2D, wgpu,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Renderer {
    Path,
    Shape,
    Line,
}
impl Renderer {
    fn name(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Shape => "shape",
            Self::Line => "line",
        }
    }
    fn bytes(self, brushed: bool) -> usize {
        match self {
            Self::Path => 64,
            _ if brushed => 112,
            _ => 80,
        }
    }
    fn capacity(self, brushed: bool) -> usize {
        match self {
            Self::Path => 1024,
            _ if brushed => 585,
            _ => 819,
        }
    }
}
struct Work {
    paths: PathRenderer,
    shapes: ShapeRenderer,
    lines: LineRenderer,
    path: PreparedPath,
    path_draws: Vec<PathDraw>,
    shape_draws: Vec<ShapeDraw>,
    line_draws: Vec<LineDraw>,
}
impl Work {
    fn new(g: &GraphicsContext, t: &Framebuffer, count: usize) -> Result<Self> {
        let mut paths = PathRenderer::new(g);
        let mut shapes = ShapeRenderer::new(g);
        let mut lines = LineRenderer::new(g);
        let mut b = Path::builder();
        b.move_to([0., 0.])
            .line_to([8., 0.])
            .line_to([8., 8.])
            .line_to([0., 8.])
            .close();
        let path = paths.prepare_path(&b.build()?, PathOptions::new())?;
        let format = t.render_format();
        paths.prepare(&format)?;
        paths.prepare_brush(&format)?;
        shapes.prepare(&format)?;
        shapes.prepare_brush(&format)?;
        lines.prepare(&format)?;
        lines.prepare_brush(&format)?;
        let color = [0.5, 0.8, 1., 0.25];
        let transforms: Vec<_> = (0..count)
            .map(|i| Transform2D::translation((i % 8) as f32 * 7., ((i / 8) % 8) as f32 * 7.))
            .collect();
        Ok(Self {
            paths,
            shapes,
            lines,
            path,
            path_draws: transforms
                .iter()
                .map(|t| {
                    PathDraw::new(color)
                        .transform(*t)
                        .antialiasing(EdgeAntialiasing::None)
                })
                .collect(),
            shape_draws: transforms
                .iter()
                .map(|t| {
                    ShapeDraw::rect(Rect::new(0., 0., 8., 8.), color)
                        .transform(*t)
                        .antialiasing(EdgeAntialiasing::None)
                })
                .collect(),
            line_draws: transforms
                .iter()
                .map(|t| {
                    LineDraw::new([0., 4.], [8., 4.], color)
                        .width(8.)
                        .transform(*t)
                        .antialiasing(EdgeAntialiasing::None)
                })
                .collect(),
        })
    }
    fn run(
        &mut self,
        g: &GraphicsContext,
        t: &mut Framebuffer,
        renderer: Renderer,
        brush: Option<&Brush>,
    ) -> Result<(f64, f64)> {
        let start = Instant::now();
        let mut frame = t.begin_frame()?;
        let record;
        {
            let mut pass = frame.render_pass().begin()?;
            let begin = Instant::now();
            match (renderer, brush) {
                (Renderer::Path, None) => {
                    self.paths
                        .draw_many(&mut pass, &self.path, &self.path_draws)?
                }
                (Renderer::Path, Some(b)) => {
                    self.paths
                        .draw_many_with_brush(&mut pass, &self.path, b, &self.path_draws)?
                }
                (Renderer::Shape, None) => self.shapes.draw_many(&mut pass, &self.shape_draws)?,
                (Renderer::Shape, Some(b)) => {
                    self.shapes
                        .draw_many_with_brush(&mut pass, b, &self.shape_draws)?
                }
                (Renderer::Line, None) => self.lines.draw_many(&mut pass, &self.line_draws)?,
                (Renderer::Line, Some(b)) => {
                    self.lines
                        .draw_many_with_brush(&mut pass, b, &self.line_draws)?
                }
            }
            record = begin.elapsed().as_secs_f64() * 1e6;
        }
        let index = frame.finish()?;
        let total = start.elapsed().as_secs_f64() * 1e6;
        g.device().poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: Some(Duration::from_secs(30)),
        })?;
        Ok(black_box((record, total)))
    }
}
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
fn pixels(g: &GraphicsContext, t: &mut Framebuffer) -> Result<Vec<u8>> {
    let texture = t.color_texture()?.clone();
    let buffer = g.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut frame = t.begin_frame()?;
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
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(Duration::from_secs(30)),
    })?;
    rx.recv_timeout(Duration::from_secs(30))??;
    let bytes = buffer.slice(..).get_mapped_range()?.to_vec();
    buffer.unmap();
    Ok(bytes)
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("run with cargo bench --bench brushes".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "{}; {:?}; 30 samples, 6 warm-ups, alternating reference/brush order",
        g.adapter().get_info().name,
        g.adapter().get_info().backend
    );
    let mut t = g.create_framebuffer(
        FramebufferOptions::new(64, 64)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )?;
    let mut brushes = vec![("solid", g.create_brush(BrushOptions::solid([1.; 4]))?)];
    for count in [2, 8, 64] {
        // Runtime stop buffers keep search/interpolation real, even though the
        // constant white result matches ordinary solid output mathematically.
        let stops: Vec<_> = (0..count)
            .map(|i| GradientStop::new(i as f32 / (count - 1) as f32, [1.; 4]))
            .collect();
        let name = match count {
            2 => "linear_2",
            8 => "linear_8",
            _ => "linear_64",
        };
        brushes.push((
            name,
            g.create_brush(BrushOptions::linear([0., 0.], [8., 0.], &stops))?,
        ));
        if count == 8 {
            brushes.push((
                "radial_8",
                g.create_brush(BrushOptions::radial([4., 4.], 4., &stops))?,
            ));
        }
    }
    println!(
        "renderer,count,brush,color_record_us,brush_record_us,color_cpu_total_us,brush_cpu_total_us,color_bytes,brush_bytes,color_draws,brush_draws"
    );
    for count in [100, 1000, 10000] {
        let mut work = Work::new(&g, &t, count)?;
        for renderer in [Renderer::Path, Renderer::Shape, Renderer::Line] {
            for (name, brush) in &brushes {
                work.run(&g, &mut t, renderer, None)?;
                let reference = pixels(&g, &mut t)?;
                work.run(&g, &mut t, renderer, Some(brush))?;
                assert_eq!(
                    reference,
                    pixels(&g, &mut t)?,
                    "{} {count} {name}",
                    renderer.name()
                );
                eprintln!(
                    "pixel equivalence passed: {} {count} {name}",
                    renderer.name()
                );
                let (mut color_record, mut brush_record, mut color_total, mut brush_total) =
                    (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                for i in 0..36 {
                    let (color, brushed) = if i % 2 == 0 {
                        (
                            work.run(&g, &mut t, renderer, None)?,
                            work.run(&g, &mut t, renderer, Some(brush))?,
                        )
                    } else {
                        let brushed = work.run(&g, &mut t, renderer, Some(brush))?;
                        (work.run(&g, &mut t, renderer, None)?, brushed)
                    };
                    if i >= 6 {
                        color_record.push(color.0);
                        color_total.push(color.1);
                        brush_record.push(brushed.0);
                        brush_total.push(brushed.1);
                    }
                }
                println!(
                    "{},{count},{name},{:.3},{:.3},{:.3},{:.3},{},{},{},{}",
                    renderer.name(),
                    median(&mut color_record),
                    median(&mut brush_record),
                    median(&mut color_total),
                    median(&mut brush_total),
                    count * renderer.bytes(false),
                    count * renderer.bytes(true),
                    count.div_ceil(renderer.capacity(false)),
                    count.div_ceil(renderer.capacity(true))
                );
            }
        }
    }
    Ok(())
}
