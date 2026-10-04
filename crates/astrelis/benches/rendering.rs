//! Headless CPU benchmarks against equivalent direct-wgpu commands.
//! Run with `cargo bench -p astrelis --bench rendering -- --help`.
//! CSV goes to stdout; adapter, settings, preparation times, and checks go to stderr.

use astrelis::{
    Framebuffer, FramebufferOptions, GraphicsContext, Mesh, MeshRenderer, PreparedTextureDraw,
    Rect, TextureBinding, TextureBindingOptions, TextureDraw, TextureOptions, TextureRenderer,
    Vertex, wgpu,
};
use std::{
    error::Error,
    hint::black_box,
    io::{self, Write},
    sync::mpsc,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SIZE: u32 = 16;
const PAGE_INSTANCES: usize = 1024;
const WAIT: Duration = Duration::from_secs(30);

struct Settings {
    counts: Vec<usize>,
    samples: usize,
    warmup: usize,
}
impl Settings {
    fn parse() -> Result<Option<Self>> {
        let mut settings = Self {
            counts: vec![100, 1000, 10000],
            samples: 40,
            warmup: 8,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    println!(
                        "cargo bench -p astrelis --bench rendering -- [--counts 100,1000,10000] [--samples 40] [--warmup 8]\n\nRelease mode, headless 16x16 RGBA8, prepared pipelines, alternating paired samples.\nCSV: median and p95 CPU begin/record/finish/total, plus completion wait.\nFinish includes pass teardown, uploads and submission; wait is NOT GPU execution time.\nEach workload checks raw/wrapped pixels before timing. No window or FPS measurement."
                    );
                    return Ok(None);
                }
                "--bench" => {} // Cargo may pass this to harness-free benchmarks.
                "--counts" => {
                    settings.counts = args
                        .next()
                        .ok_or("--counts needs a value")?
                        .split(',')
                        .map(str::parse)
                        .collect::<std::result::Result<_, _>>()?;
                }
                "--samples" => {
                    settings.samples = args.next().ok_or("--samples needs a value")?.parse()?
                }
                "--warmup" => {
                    settings.warmup = args.next().ok_or("--warmup needs a value")?.parse()?
                }
                _ => return Err(format!("unknown argument: {arg}; use --help").into()),
            }
        }
        if settings.counts.is_empty()
            || settings.counts.iter().any(|n| *n == 0 || *n > 1_000_000)
            || settings.samples < 5
            || settings.samples > 10000
            || settings.warmup == 0
            || settings.warmup > 10000
        {
            return Err("counts must be 1..=1000000, samples 5..=10000, warmup 1..=10000".into());
        }
        Ok(Some(settings))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workload {
    Mesh,
    MeshAlternating,
    TexturePrepared,
    TextureDynamic,
    TextureBatch,
}
impl Workload {
    const ALL: [Self; 5] = [
        Self::Mesh,
        Self::MeshAlternating,
        Self::TexturePrepared,
        Self::TextureDynamic,
        Self::TextureBatch,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Mesh => "mesh_same",
            Self::MeshAlternating => "mesh_alternating",
            Self::TexturePrepared => "texture_prepared",
            Self::TextureDynamic => "texture_dynamic",
            Self::TextureBatch => "texture_batch",
        }
    }
    fn dynamic(self) -> bool {
        matches!(self, Self::TextureDynamic | Self::TextureBatch)
    }
    fn mesh(self) -> bool {
        matches!(self, Self::Mesh | Self::MeshAlternating)
    }
}

// Matched normalized, untransformed rectangles. Packing is measured on BOTH dynamic paths.
#[derive(Clone, Copy)]
struct Input {
    rect: Rect,
    tint: [f32; 4],
}
impl Input {
    fn parameters(self) -> [f32; 16] {
        let r = self.rect;
        [
            r.x,
            r.y,
            r.width,
            0.,
            0.,
            r.height,
            0.,
            0.,
            0.,
            0.,
            1.,
            1.,
            self.tint[0],
            self.tint[1],
            self.tint[2],
            self.tint[3],
        ]
    }
    fn draw(self) -> TextureDraw {
        TextureDraw::normalized(self.rect).tint(self.tint)
    }
}
struct Data {
    inputs: Vec<Input>,
    draws: Vec<TextureDraw>,
    prepared: PreparedTextureDraw,
    static_buffer: wgpu::Buffer,
    pages: Vec<wgpu::Buffer>,
    packed: Vec<[f32; 16]>,
}
impl Data {
    fn new(g: &GraphicsContext, renderer: &TextureRenderer, count: usize) -> Result<Self> {
        let inputs: Vec<_> = (0..count)
            .map(|i| Input {
                rect: Rect::new(0.125 + (i % 4) as f32 * 0.03125, 0.125, 0.5, 0.5),
                tint: [0.25 + (i % 5) as f32 * 0.125, 0.5, 0.75, 1.],
            })
            .collect();
        let draws = inputs.iter().map(|input| input.draw()).collect();
        let static_input = Input {
            rect: Rect::new(0.125, 0.125, 0.5, 0.5),
            tint: [0.5, 0.75, 1., 1.],
        };
        let prepared = renderer.prepare_draws(&[static_input.draw()], [SIZE as f32; 2])?;
        let static_buffer = g
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("raw static parameters"),
                contents: bytemuck::cast_slice(&static_input.parameters()),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let pages = (0..count.div_ceil(PAGE_INSTANCES))
            .map(|_| {
                g.device().create_buffer(&wgpu::BufferDescriptor {
                    label: Some("raw reusable upload page"),
                    size: (PAGE_INSTANCES * 64) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        Ok(Self {
            inputs,
            draws,
            prepared,
            static_buffer,
            pages,
            packed: Vec::with_capacity(count),
        })
    }
}
struct Rig {
    graphics: GraphicsContext,
    target: Framebuffer,
    meshes: [Mesh; 2],
    meshes_renderer: MeshRenderer,
    textures: TextureRenderer,
    binding: TextureBinding,
    mesh_pipeline: wgpu::RenderPipeline,
    texture_pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
}
impl Rig {
    async fn new() -> Result<Self> {
        let start = Instant::now();
        let graphics = GraphicsContext::headless().await?;
        eprintln!(
            "adapter={:?}; os={}; arch={}; debug_assertions={}; validation=wgpu_defaults",
            graphics.adapter().get_info(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            cfg!(debug_assertions)
        );
        eprintln!("context_initialization_us={:.3}", micros(start.elapsed()));
        let scope = graphics
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let start = Instant::now();
        let target = graphics.create_framebuffer(FramebufferOptions::new(SIZE, SIZE).usage(
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        ))?;
        let vertices = [
            Vertex::new([-1., -1., 0.], [0.5, 0.75, 1., 1.]),
            Vertex::new([1., -1., 0.], [0.5, 0.75, 1., 1.]),
            Vertex::new([0., 1., 0.], [0.5, 0.75, 1., 1.]),
        ];
        let meshes = [
            graphics.create_mesh(&vertices, &[0, 1, 2])?,
            graphics.create_mesh(
                &vertices.map(|vertex| Vertex::new(vertex.position, [1., 0.4, 0.25, 1.])),
                &[0, 1, 2],
            )?,
        ];
        let mut meshes_renderer = MeshRenderer::new(&graphics);
        let image = graphics.create_texture(TextureOptions::new(2, 2))?;
        image.write(&[255; 16])?;
        let mut textures = TextureRenderer::new(&graphics);
        let binding = textures.create_binding(image.view(), TextureBindingOptions::new())?;
        eprintln!("wrapped_resources_us={:.3}", micros(start.elapsed()));
        let start = Instant::now();
        meshes_renderer.prepare(target.format(), target.sample_count())?;
        eprintln!(
            "wrapped_mesh_pipeline_prepare_us={:.3}",
            micros(start.elapsed())
        );
        let start = Instant::now();
        textures.prepare(&binding, &target.render_format())?;
        eprintln!(
            "wrapped_texture_pipeline_prepare_us={:.3}",
            micros(start.elapsed())
        );
        let device = graphics.device();
        let start = Instant::now();
        let mesh_pipeline = pipeline(
            device,
            target.format(),
            meshes_renderer.default_material().pipeline_layout(),
            meshes_renderer.default_material().shader(),
            "fragment_main",
            &Vertex::layout(),
        );
        eprintln!(
            "raw_mesh_pipeline_prepare_us={:.3}",
            micros(start.elapsed())
        );
        let start = Instant::now();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raw texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raw texture binding"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(image.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("raw matching texture shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../src/texture.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("raw texture pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let texture_pipeline = pipeline(
            device,
            target.format(),
            &pipeline_layout,
            &shader,
            "fragment_straight",
            &wgpu::VertexBufferLayout {
                array_stride: 64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &attributes,
            },
        );
        eprintln!(
            "raw_texture_resources_and_pipeline_us={:.3}; preparation_times_are_single_observations=true",
            micros(start.elapsed())
        );
        // Drain setup writes/compilation before samples. Error scope is not in the timed loop.
        wait(&graphics, graphics.queue().submit([]))?;
        if let Some(error) = scope.pop().await {
            return Err(error.into());
        }
        Ok(Self {
            graphics,
            target,
            meshes,
            meshes_renderer,
            textures,
            binding,
            mesh_pipeline,
            texture_pipeline,
            group,
        })
    }
    fn wrapped(&mut self, workload: Workload, data: &Data) -> Result<Sample> {
        let start = Instant::now();
        let mut frame = self.target.begin_frame()?;
        let mut pass = frame.render_pass().begin()?;
        let begin = start.elapsed();
        let start = Instant::now();
        if workload.mesh() {
            for i in 0..data.inputs.len() {
                let slot = if workload == Workload::MeshAlternating {
                    i % 2
                } else {
                    0
                };
                self.meshes_renderer.draw(&mut pass, &self.meshes[slot])?;
            }
        } else if workload == Workload::TexturePrepared {
            for _ in 0..data.inputs.len() {
                self.textures
                    .draw_prepared(&mut pass, &self.binding, &data.prepared)?;
            }
        } else if workload == Workload::TextureBatch {
            self.textures
                .draw_many(&mut pass, &self.binding, black_box(&data.draws))?;
        } else {
            for draw in &data.draws {
                self.textures
                    .draw(&mut pass, &self.binding, black_box(*draw))?;
            }
        }
        let record = start.elapsed();
        let start = Instant::now();
        drop(pass);
        let index = frame.finish()?;
        let finish = start.elapsed();
        let start = Instant::now();
        wait(&self.graphics, index)?;
        Ok(Sample {
            begin,
            record,
            finish,
            wait: start.elapsed(),
        })
    }
    fn raw(&self, workload: Workload, data: &mut Data) -> Result<Sample> {
        let g = &self.graphics;
        let start = Instant::now();
        let mut encoder = g.device().create_command_encoder(&Default::default());
        let colors = [Some(wgpu::RenderPassColorAttachment {
            view: self.target.color_view()?,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &colors,
            ..Default::default()
        });
        pass.set_viewport(0., 0., SIZE as f32, SIZE as f32, 0., 1.);
        pass.set_scissor_rect(0, 0, SIZE, SIZE);
        let begin = start.elapsed();
        let start = Instant::now();
        if workload.mesh() {
            pass.set_pipeline(&self.mesh_pipeline);
            for i in 0..data.inputs.len() {
                let slot = if workload == Workload::MeshAlternating {
                    i % 2
                } else {
                    0
                };
                if i == 0 || workload == Workload::MeshAlternating {
                    pass.set_vertex_buffer(0, self.meshes[slot].vertex_buffer().slice(..));
                    pass.set_index_buffer(
                        self.meshes[slot]
                            .index_buffer()
                            .ok_or("missing indices")?
                            .slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                }
                pass.draw_indexed(0..3, 0, 0..1);
            }
        } else {
            pass.set_pipeline(&self.texture_pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            if workload == Workload::TexturePrepared {
                pass.set_vertex_buffer(0, data.static_buffer.slice(..));
                for _ in 0..data.inputs.len() {
                    pass.draw(0..6, 0..1);
                }
            } else {
                data.packed.clear();
                // Preallocated storage, same 64-byte instance format and page size as Astrelis.
                for input in &data.inputs {
                    data.packed.push(black_box(*input).parameters());
                }
                for (page_index, chunk) in data.packed.chunks(PAGE_INSTANCES).enumerate() {
                    pass.set_vertex_buffer(0, data.pages[page_index].slice(..));
                    if workload == Workload::TextureBatch {
                        pass.draw(0..6, 0..chunk.len() as u32);
                    } else {
                        for i in 0..chunk.len() as u32 {
                            pass.draw(0..6, i..i + 1);
                        }
                    }
                }
            }
        }
        let record = start.elapsed();
        let start = Instant::now();
        drop(pass);
        if workload.dynamic() {
            for (page, chunk) in data.pages.iter().zip(data.packed.chunks(PAGE_INSTANCES)) {
                g.queue().write_buffer(page, 0, bytemuck::cast_slice(chunk));
            }
        }
        let index = g.queue().submit([encoder.finish()]);
        let finish = start.elapsed();
        let start = Instant::now();
        wait(g, index)?;
        Ok(Sample {
            begin,
            record,
            finish,
            wait: start.elapsed(),
        })
    }
    fn pixels(&self) -> Result<Vec<u8>> {
        let g = &self.graphics;
        let buffer = g.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("benchmark equivalence readback"),
            size: u64::from(SIZE) * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = g.device().create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.target.color_texture()?,
                mip_level: 0,
                origin: Default::default(),
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(SIZE),
                },
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        let index = g.queue().submit([encoder.finish()]);
        let (send, receive) = mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        wait(g, index)?;
        receive.recv_timeout(WAIT)??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let pixels = mapped
            .as_chunks::<256>()
            .0
            .iter()
            .flat_map(|row| row[..SIZE as usize * 4].iter().copied())
            .collect();
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
}
fn pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fragment: &str,
    vertices: &wgpu::VertexBufferLayout<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("benchmark reference pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex_main"),
            compilation_options: Default::default(),
            buffers: &[Some(vertices.clone())],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
#[derive(Clone, Copy)]
struct Sample {
    begin: Duration,
    record: Duration,
    finish: Duration,
    wait: Duration,
}
impl Sample {
    fn metrics(self) -> [Duration; 5] {
        [
            self.begin,
            self.record,
            self.finish,
            self.begin + self.record + self.finish,
            self.wait,
        ]
    }
}
fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}
fn percentile(values: &[f64], fraction: f64) -> f64 {
    values[((values.len() as f64 * fraction).ceil() as usize).saturating_sub(1)]
}
fn wait(g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    Ok(())
}
fn report(
    output: &mut impl Write,
    workload: Workload,
    count: usize,
    samples: &[Vec<Sample>; 2],
) -> Result<()> {
    for (metric_index, metric) in ["begin", "record", "finish", "cpu_total", "completion_wait"]
        .iter()
        .enumerate()
    {
        let mut metrics: [Vec<f64>; 2] = std::array::from_fn(|side| {
            samples[side]
                .iter()
                .map(|s| micros(s.metrics()[metric_index]))
                .collect()
        });
        for values in &mut metrics {
            values.sort_by(f64::total_cmp);
        }
        let raw_median = percentile(&metrics[0], 0.5);
        let wrapped_median = percentile(&metrics[1], 0.5);
        // The relative number is descriptive; it is not a portable pass/fail threshold.
        writeln!(
            output,
            "{},{count},{metric},{:.3},{:.3},{:.3},{:.3},{:.3},{:.2}",
            workload.name(),
            raw_median,
            percentile(&metrics[0], 0.95),
            wrapped_median,
            percentile(&metrics[1], 0.95),
            wrapped_median - raw_median,
            (wrapped_median / raw_median - 1.) * 100.
        )?;
    }
    Ok(())
}
async fn run(settings: Settings) -> Result<()> {
    let mut rig = Rig::new().await?;
    eprintln!(
        "counts={:?}; samples={}; warmup={}; gpu_execution_timing=not_measured",
        settings.counts, settings.samples, settings.warmup
    );
    let stdout = io::stdout();
    let mut output = io::BufWriter::new(stdout.lock());
    writeln!(
        output,
        "workload,count,metric,raw_median_us,raw_p95_us,astrelis_median_us,astrelis_p95_us,delta_us,overhead_percent"
    )?;
    for count in settings.counts {
        let start = Instant::now();
        let mut data = Data::new(&rig.graphics, &rig.textures, count)?;
        eprintln!(
            "data_preparation_us count={count}: {:.3}",
            micros(start.elapsed())
        );
        wait(&rig.graphics, rig.graphics.queue().submit([]))?;
        for workload in Workload::ALL {
            // Untimed pixel check uses all draws and catches divergent reference commands.
            rig.raw(workload, &mut data)?;
            let raw = rig.pixels()?;
            rig.wrapped(workload, &data)?;
            let wrapped = rig.pixels()?;
            if raw != wrapped || !wrapped.as_chunks::<4>().0.iter().any(|p| p[3] != 0) {
                return Err(format!(
                    "pixel equivalence failed: {} count={count}",
                    workload.name()
                )
                .into());
            }
            let mut samples: [Vec<Sample>; 2] =
                std::array::from_fn(|_| Vec::with_capacity(settings.samples));
            for iteration in 0..settings.warmup + settings.samples {
                // Alternate pair order to reduce systematic drift from temperature/backend state.
                for step in 0..2 {
                    let side = (iteration + step) % 2;
                    let sample = if side == 0 {
                        rig.raw(workload, &mut data)?
                    } else {
                        rig.wrapped(workload, &data)?
                    };
                    if iteration >= settings.warmup {
                        samples[side].push(sample);
                    }
                }
            }
            report(&mut output, workload, count, &samples)?;
            output.flush()?;
            eprintln!("checked and measured {} count={count}", workload.name());
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    let Some(settings) = Settings::parse()? else {
        return Ok(());
    };
    if cfg!(debug_assertions) {
        return Err("use cargo bench (release mode) for performance measurements".into());
    }
    pollster::block_on(run(settings))
}
