//! Matched changing-label workloads: individual versus batched preparation, with visible output.
//! GPU waits and readbacks are outside CPU timings; GPU execution is not measured.
use astrelis::{
    Framebuffer, FramebufferOptions, GraphicsContext, Painter, PreparedText, TextBuffer, TextDraw,
    TextLayout, TextRasterOptions, TextStyle, TextSystem, wgpu,
};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SAMPLES: usize = 40;
const WARMUP: usize = 8;
const SIZE: [u32; 2] = [1920, 1080];
fn wait(g: &GraphicsContext, submission: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(10)),
    })?;
    Ok(())
}
fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}
fn row(count: usize, mode: &str, stage: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    println!(
        "{count},{mode},{stage},{:.3},{:.3}",
        (values[SAMPLES / 2 - 1] + values[SAMPLES / 2]) / 2.,
        values[(SAMPLES as f64 * 0.95).ceil() as usize - 1]
    );
}
fn origin(i: usize) -> [f32; 2] {
    [8. + (i % 20) as f32 * 95., 8. + (i / 20) as f32 * 21.]
}
fn prepare(
    painter: &mut Painter,
    layouts: &[Arc<TextLayout>],
    mode: &str,
) -> Result<Vec<PreparedText>> {
    if mode == "batched" {
        Ok(painter.prepare_texts(layouts, TextRasterOptions::new())?)
    } else {
        layouts
            .iter()
            .map(|l| Ok(painter.prepare_text(l, TextRasterOptions::new())?))
            .collect()
    }
}
fn readback(g: &GraphicsContext, target: &mut Framebuffer, count: usize) -> Result<Vec<u8>> {
    let texture = target.color_texture()?.clone();
    let pitch = SIZE[0] * 4;
    let buffer = g.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("Text batch benchmark verification"),
        size: u64::from(pitch) * u64::from(SIZE[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut frame = target.begin_frame()?;
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
                bytes_per_row: Some(pitch),
                rows_per_image: Some(SIZE[1]),
            },
        },
        texture.size(),
    );
    let submission = frame.finish()?;
    let (tx, rx) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    wait(g, submission)?;
    rx.recv_timeout(Duration::from_secs(10))??;
    let bytes = buffer.slice(..).get_mapped_range()?.to_vec();
    buffer.unmap();
    for i in [0, count - 1] {
        let [x, y] = origin(i).map(|v| v as usize);
        assert!(
            (y..y + 21).any(|y| (x..x + 90).any(|x| bytes[(y * SIZE[0] as usize + x) * 4 + 3] > 0)),
            "label {i} invisible"
        );
    }
    Ok(bytes)
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("use cargo bench --bench text_batches".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    let style = TextStyle::new()
        .family("Source Sans 3")
        .font_size(14.)
        .line_height(21.);
    eprintln!(
        "adapter={:?}; samples={SAMPLES}; warmup={WARMUP}; target=1920x1080 RGBA8 1xMSAA; gpu_execution=not_measured; old_resources=retained_until_replacement_success; glyphs_per_label=9",
        g.adapter().get_info()
    );
    println!("labels,mode,stage,median_us,p95_us");
    for count in [60, 600, 1000] {
        let mut reference = None;
        for mode in ["individual", "batched"] {
            let mut painter = Painter::new(&g);
            let mut target = g
                .create_framebuffer(FramebufferOptions::new(SIZE[0], SIZE[1]).usage(
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                ))?;
            painter.prepare(&target.render_format())?;
            let mut buffers = Vec::with_capacity(count);
            let mut layouts = Vec::with_capacity(count);
            for i in 0..count {
                let mut b = TextBuffer::new();
                b.set_text(&format!("Item {i:05}"), style.clone())?;
                layouts.push(b.layout(&mut fonts)?);
                buffers.push(b);
            }
            let mut texts = prepare(&mut painter, &layouts, mode)?;
            assert_eq!(texts.len(), count);
            wait(&g, g.queue().submit([]))?;
            let mut updates = Vec::new();
            let mut shapes = Vec::new();
            let mut preparations = Vec::new();
            let mut records = Vec::new();
            let mut frames = Vec::new();
            let mut uploads = 0;
            let mut allocations = 0;
            let mut reuses = 0;
            for sample in 0..WARMUP + SAMPLES {
                let before = painter.text().stats();
                let start = Instant::now();
                layouts.clear();
                for (i, b) in buffers.iter_mut().enumerate() {
                    b.set_text(
                        &format!("Item {:05}", 10000 + sample * count + i),
                        style.clone(),
                    )?;
                    layouts.push(b.layout(&mut fonts)?);
                }
                let shaped = start.elapsed();
                let prep_start = Instant::now();
                let next = prepare(&mut painter, &layouts, mode)?;
                let prepared = prep_start.elapsed();
                texts = next; // Replace only after successful preparation in both modes.
                let updated = start.elapsed();
                let after = painter.text().stats();
                assert_eq!(before.cache_misses, after.cache_misses);
                assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
                let allocated =
                    after.geometry_buffer_allocations - before.geometry_buffer_allocations;
                let reused = after.geometry_buffer_reuses - before.geometry_buffer_reuses;
                let uploaded = allocated + reused;
                let labels_per_chunk = (64 * 1024) / (9 * 48);
                assert_eq!(
                    uploaded,
                    if mode == "batched" {
                        count.div_ceil(labels_per_chunk)
                    } else {
                        count
                    } as u64
                );
                assert_eq!(
                    after.geometry_bytes - before.geometry_bytes,
                    count as u64 * 9 * 48
                );
                if mode == "batched" && sample >= WARMUP {
                    assert_eq!(allocated, 0);
                }
                for (i, t) in texts.iter().enumerate() {
                    assert_eq!(t.glyph_count(), 9);
                    let b = t.ink_bounds().unwrap();
                    let [x, y] = origin(i);
                    assert!(
                        x + b.x >= 0.
                            && y + b.y >= 0.
                            && x + b.x + b.width <= SIZE[0] as f32
                            && y + b.y + b.height <= SIZE[1] as f32
                    );
                }
                wait(&g, g.queue().submit([]))?;
                let before_draw = painter.text().stats();
                let frame_start = Instant::now();
                let mut frame = target.begin_frame()?;
                let recorded;
                {
                    let mut pass = frame.render_pass().begin()?;
                    let record_start = Instant::now();
                    let mut paint = painter.begin(&mut pass)?;
                    for (i, text) in texts.iter().enumerate() {
                        paint
                            .draw_text(text, TextDraw::new(origin(i)).color([0.7, 0.85, 1., 1.]))?;
                    }
                    recorded = record_start.elapsed();
                }
                let submission = frame.finish()?;
                let frame_cpu = frame_start.elapsed();
                let after_draw = painter.text().stats();
                assert_eq!(before_draw.geometry_bytes, after_draw.geometry_bytes);
                assert_eq!(after_draw.draw_calls - before_draw.draw_calls, count as u64);
                assert_eq!(
                    after_draw.parameter_bytes - before_draw.parameter_bytes,
                    count as u64 * 48
                );
                wait(&g, submission)?;
                if sample >= WARMUP {
                    updates.push(us(updated));
                    shapes.push(us(shaped));
                    preparations.push(us(prepared));
                    records.push(us(recorded));
                    frames.push(us(frame_cpu));
                    uploads += uploaded;
                    allocations += allocated;
                    reuses += reused;
                }
            }
            row(count, mode, "content_layout_prepare_replace_cpu", updates);
            row(count, mode, "content_layout_cpu", shapes);
            row(count, mode, "cached_geometry_prepare_cpu", preparations);
            row(count, mode, "record_cpu", records);
            row(count, mode, "frame_cpu", frames);
            eprintln!(
                "labels={count}; mode={mode}; measured_geometry_uploads={uploads}; allocations={allocations}; reuses={reuses}; stats={:?}",
                painter.text().stats()
            );
            let pixels = readback(&g, &mut target, count)?;
            if let Some(reference) = &reference {
                assert_eq!(&pixels, reference, "individual and batched pixels differ");
            } else {
                reference = Some(pixels);
            }
        }
    }
    Ok(())
}
