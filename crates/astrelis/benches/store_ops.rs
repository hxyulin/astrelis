//! Wall time from frame start to GPU completion for a 1920x1080 target, comparing
//! attachment store operations. With one submission in flight and a small CPU
//! recording, completion time approximates GPU execution of clear, draws, stores
//! and resolve. This is not a timestamp measurement.
use astrelis::{FramebufferOptions, GraphicsContext, Rect, ShapeDraw, ShapeRenderer, wgpu};
use std::{
    error::Error,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WAIT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
enum Case {
    /// Builder defaults.
    Default,
    /// Color samples and depth/stencil stored explicitly.
    Store,
    /// Color samples and depth/stencil discarded explicitly.
    Discard,
}
impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Store => "store",
            Self::Discard => "discard",
        }
    }
}

fn main() -> Result<()> {
    let mut samples = 60usize;
    let mut warmup = 10usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--samples" => samples = args.next().ok_or("missing samples")?.parse()?,
            "--warmup" => warmup = args.next().ok_or("missing warmup")?.parse()?,
            "--help" | "-h" => {
                println!(
                    "cargo bench -p astrelis --bench store_ops -- [--samples 60] [--warmup 10]\nFrame start to completion wall time, 1920x1080 RGBA8, 1x/4x MSAA with Depth24PlusStencil8, flat and edge-heavy workloads.\nCases: builder defaults, explicit store, explicit discard. Interleaved per sample."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if cfg!(debug_assertions) {
        return Err("run in release mode with cargo bench".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "adapter={:?}; samples={samples}; warmup={warmup}; gpu_timestamps=not_used",
        g.adapter().get_info()
    );
    println!("msaa,workload,case,completion_median_us,completion_p95_us");
    for sample_count in [1, 4] {
        let mut target = g.create_framebuffer(
            FramebufferOptions::new(1920, 1080)
                .sample_count(sample_count)
                .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8),
        )?;
        let mut shapes = ShapeRenderer::new(&g);
        shapes.prepare(&target.render_format())?;
        let flat: Vec<_> = (0..64)
            .map(|i| {
                let x = (i % 8) as f32 * 240.;
                let y = (i / 8) as f32 * 135.;
                ShapeDraw::rounded_rect(
                    Rect::new(x + 4., y + 4., 232., 127.),
                    12.,
                    [0.2, 0.4, 0.8, 0.5],
                )
            })
            .collect();
        // Many small anti-aliased edges leave little uniform sample data to compress.
        let edges: Vec<_> = flat
            .iter()
            .copied()
            .chain((0..20000).map(|i| {
                let x = (i * 37 % 1913) as f32;
                let y = (i * 53 % 1073) as f32;
                ShapeDraw::ellipse(
                    Rect::new(x, y, 5.5, 3.5),
                    [(i % 7) as f32 / 7., 0.5, (i % 3) as f32 / 3., 1.],
                )
            }))
            .collect();
        for (workload, draws) in [("flat", flat), ("edges", edges)] {
            let cases = [Case::Default, Case::Store, Case::Discard];
            let mut times = vec![Vec::new(); cases.len()];
            for i in 0..warmup + samples {
                for (index, &case) in cases.iter().enumerate() {
                    let start = Instant::now();
                    let mut frame = target.begin_frame()?;
                    {
                        let mut pass = frame.render_pass();
                        let ops = |store| wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store,
                        };
                        pass = match case {
                            Case::Default => pass,
                            Case::Store => pass
                                .color_ops(ops(wgpu::StoreOp::Store))
                                .depth_ops(Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(1.),
                                    store: wgpu::StoreOp::Store,
                                }))
                                .stencil_ops(Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(0),
                                    store: wgpu::StoreOp::Store,
                                })),
                            Case::Discard => pass
                                .color_ops(ops(if sample_count > 1 {
                                    wgpu::StoreOp::Discard
                                } else {
                                    wgpu::StoreOp::Store
                                }))
                                .depth_ops(Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(1.),
                                    store: wgpu::StoreOp::Discard,
                                }))
                                .stencil_ops(Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(0),
                                    store: wgpu::StoreOp::Discard,
                                })),
                        };
                        let mut pass = pass.begin()?;
                        shapes.draw_many(&mut pass, &draws)?;
                    }
                    let index_ = frame.finish()?;
                    g.device().poll(wgpu::PollType::Wait {
                        submission_index: Some(index_),
                        timeout: Some(WAIT),
                    })?;
                    if i >= warmup {
                        times[index].push(start.elapsed().as_secs_f64() * 1e6);
                    }
                }
            }
            for (case, mut values) in cases.into_iter().zip(times) {
                values.sort_by(f64::total_cmp);
                let n = values.len();
                println!(
                    "{sample_count},{workload},{},{:.1},{:.1}",
                    case.name(),
                    values[n / 2],
                    values[(n as f64 * 0.95).ceil() as usize - 1]
                );
            }
        }
    }
    Ok(())
}
