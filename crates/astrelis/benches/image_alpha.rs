//! Wall time from frame start to GPU completion for fill-bound image drawing,
//! comparing straight-alpha and premultiplied linear filtering. With one
//! submission in flight and few draw calls, completion time approximates GPU
//! fragment cost. This is not a timestamp measurement.
use astrelis::{
    FramebufferOptions, GraphicsContext, Rect, TextureAlpha, TextureBindingOptions, TextureDraw,
    TextureOptions, TextureRenderer, wgpu,
};
use std::{
    error::Error,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WAIT: Duration = Duration::from_secs(30);

fn main() -> Result<()> {
    let mut samples = 60usize;
    let mut warmup = 10usize;
    let mut layers = 16usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--samples" => samples = args.next().ok_or("missing samples")?.parse()?,
            "--warmup" => warmup = args.next().ok_or("missing warmup")?.parse()?,
            "--layers" => layers = args.next().ok_or("missing layers")?.parse()?,
            "--help" | "-h" => {
                println!(
                    "cargo bench -p astrelis --bench image_alpha -- [--samples 60] [--warmup 10] [--layers 16]\nFrame start to completion wall time, 1920x1080 RGBA8 1x MSAA, `layers` full-target\nlinear-filtered draws of 512x512 RGBA8 images (magnified). Straight and premultiplied interleaved."
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
        "adapter={:?}; samples={samples}; warmup={warmup}; layers={layers}; gpu_timestamps=not_used",
        g.adapter().get_info()
    );
    let mut target = g.create_framebuffer(FramebufferOptions::new(1920, 1080))?;
    let mut renderer = TextureRenderer::new(&g);
    // `varying` changes alpha between every pair of texels (worst case for
    // straight-alpha filtering). `icon` is opaque art on a transparent background
    // with anti-aliased edges, like typical UI images. `opaque` is a photo-like image.
    let mut bindings = Vec::new();
    for image in ["varying", "icon", "opaque"] {
        let texture = g.create_texture(TextureOptions::new(512, 512))?;
        let pixels: Vec<u8> = (0..512 * 512)
            .flat_map(|i| {
                let (x, y) = (i % 512, i / 512);
                let alpha = if image == "varying" {
                    ((x + y) / 4) as u8
                } else if image == "opaque" {
                    255
                } else {
                    let d = (((x as f32 - 256.).powi(2) + (y as f32 - 256.).powi(2)).sqrt() - 200.)
                        .clamp(-0.5, 0.5);
                    ((0.5 - d) * 255.) as u8
                };
                [(x / 2) as u8, (y / 2) as u8, ((x ^ y) & 255) as u8, alpha]
            })
            .collect();
        texture.write(&pixels)?;
        for (name, alpha) in [
            ("straight", TextureAlpha::Straight),
            ("premultiplied", TextureAlpha::Premultiplied),
        ] {
            let binding = renderer
                .create_binding(texture.view(), TextureBindingOptions::new().alpha(alpha))?;
            renderer.prepare(&binding, &target.render_format())?;
            bindings.push((image, name, binding));
        }
    }
    // Slightly offset layers so every fragment filters between texels.
    let draws: Vec<_> = (0..layers)
        .map(|i| {
            let o = i as f32 * 0.37;
            TextureDraw::new(Rect::new(-o, -o, 1920. + 2. * o, 1080. + 2. * o))
                .tint([1., 1., 1., 0.9])
        })
        .collect();
    let mut times = vec![Vec::new(); bindings.len()];
    for i in 0..warmup + samples {
        for (index, (_, _, binding)) in bindings.iter().enumerate() {
            let start = Instant::now();
            let mut frame = target.begin_frame()?;
            renderer.draw_many(&mut frame.render_pass().begin()?, binding, &draws)?;
            let submission = frame.finish()?;
            g.device().poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(WAIT),
            })?;
            if i >= warmup {
                times[index].push(start.elapsed().as_secs_f64() * 1e6);
            }
        }
    }
    println!("layers,image,alpha,completion_median_us,completion_p95_us");
    for ((image, name, _), mut values) in bindings.iter().zip(times) {
        values.sort_by(f64::total_cmp);
        let n = values.len();
        println!(
            "{layers},{image},{name},{:.1},{:.1}",
            values[n / 2],
            values[(n as f64 * 0.95).ceil() as usize - 1]
        );
    }
    Ok(())
}
