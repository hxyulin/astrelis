//! Screen-sized text, content updates, and raster cache churn. GPU pass timestamps are optional.
//! Completion and readback are outside CPU intervals; GPU timings include attachment clear/store.
use astrelis::{
    Framebuffer, FramebufferOptions, GraphicsContext, Painter, PreparedText, RenderPassDescriptor,
    TextBuffer, TextDraw, TextLayout, TextRasterOptions, TextRenderError, TextRenderer,
    TextRendererOptions, TextStyle, TextSystem, wgpu,
};
use std::{
    collections::HashSet,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SAMPLES: usize = 40;
const WARMUP: usize = 8;
const SIZE: [u32; 2] = [1920, 1080];
const LABELS: usize = 600;
fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}
fn print(case: &str, stage: &str, mut values: Vec<f64>) {
    if values.is_empty() {
        println!("{case},{stage},unavailable,unavailable");
        return;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    println!(
        "{case},{stage},{:.3},{:.3}",
        (values[n / 2 - 1] + values[n / 2]) / 2.,
        values[(n as f64 * 0.95).ceil() as usize - 1]
    );
}
fn wait(g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(Duration::from_secs(10)),
    })?;
    Ok(())
}
fn mapped(
    g: &GraphicsContext,
    buffer: &wgpu::Buffer,
    index: wgpu::SubmissionIndex,
) -> Result<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    wait(g, index)?;
    rx.recv_timeout(Duration::from_secs(10))??;
    let data = buffer.slice(..).get_mapped_range()?.to_vec();
    buffer.unmap();
    Ok(data)
}
struct Timer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}
impl Timer {
    fn new(g: &GraphicsContext) -> Self {
        Self {
            queries: g.device().create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("Text pass timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: g.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("Text timestamp resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: g.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("Text timestamp readback"),
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
    fn writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }
    fn read(&self, g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<Option<f64>> {
        let data = mapped(g, &self.read, index)?;
        let begin = u64::from_ne_bytes(data[..8].try_into()?);
        let end = u64::from_ne_bytes(data[8..16].try_into()?);
        // Some backends omit an end-of-fragment timestamp for a clear-only pass.
        // Missing/stale query values are not a duration; never wrap them into one.
        if begin == 0 || end == 0 || end < begin {
            return Ok(None);
        }
        Ok(Some(
            (end - begin) as f64 * f64::from(g.queue().get_timestamp_period()) / 1000.,
        ))
    }
}
fn layout(
    fonts: &mut TextSystem,
    text: &str,
    size: f32,
    family: &str,
    width: Option<f32>,
) -> Result<Arc<TextLayout>> {
    let mut buffer = TextBuffer::new();
    buffer.set_text(
        text,
        TextStyle::new()
            .family(family)
            .font_size(size)
            .line_height(size * 1.5),
    )?;
    buffer.set_width(width)?;
    let layout = buffer.layout(fonts)?;
    assert!(
        layout.missing_glyphs().is_empty(),
        "workload has missing glyphs"
    );
    Ok(layout)
}
fn visible(text: &PreparedText, origin: [f32; 2]) {
    if let Some(b) = text.ink_bounds() {
        assert!(
            origin[0] + b.x >= 0.
                && origin[1] + b.y >= 0.
                && origin[0] + b.x + b.width <= SIZE[0] as f32
                && origin[1] + b.y + b.height <= SIZE[1] as f32,
            "text quad outside framebuffer: {b:?} at {origin:?}"
        );
    }
}
fn origin(i: usize) -> [f32; 2] {
    [8. + (i % 20) as f32 * 95., 8. + (i / 20) as f32 * 35.]
}
fn frame(
    g: &GraphicsContext,
    target: &mut Framebuffer,
    painter: &mut Painter,
    texts: &[(PreparedText, [f32; 2])],
    timer: Option<&Timer>,
) -> Result<(f64, f64, Option<f64>)> {
    let start_total = Instant::now();
    let mut frame = target.begin_frame()?;
    let colors = [Some(frame.color_attachment()?)];
    let descriptor = RenderPassDescriptor {
        colors: &colors,
        timestamp_writes: timer.map(Timer::writes),
        ..Default::default()
    };
    let record;
    {
        let mut pass = frame.begin_render_pass(&descriptor)?;
        let start = Instant::now();
        let mut paint = painter.begin(&mut pass)?;
        for (text, origin) in texts {
            paint.draw_text(text, TextDraw::new(*origin).color([0.7, 0.85, 1., 1.]))?;
        }
        record = us(start.elapsed());
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
    let total = us(start_total.elapsed());
    let gpu = if let Some(t) = timer {
        t.read(g, index)?
    } else {
        wait(g, index)?;
        None
    };
    Ok((record, total, gpu))
}
// Readback outside timing verifies visible output, including the final grid row/column.
fn check_pixels(g: &GraphicsContext, target: &mut Framebuffer, grid: bool) -> Result<()> {
    let texture = target.color_texture()?.clone();
    let pitch = SIZE[0] * 4; // 7680 is aligned to wgpu's 256-byte row requirement.
    let buffer = g.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("Visible text verification"),
        size: u64::from(pitch) * u64::from(SIZE[1]),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
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
    let pixels = mapped(g, &buffer, frame.finish()?)?;
    let filled = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] != 0)
        .count();
    assert!(
        filled > 10000,
        "workload did not produce substantial visible text"
    );
    if grid {
        for i in [0, LABELS - 1] {
            let [x, y] = origin(i);
            assert!(
                (y as usize..y as usize + 30).any(|y| (x as usize..x as usize + 90)
                    .any(|x| pixels[(y * SIZE[0] as usize + x) * 4 + 3] != 0)),
                "grid endpoint is blank"
            );
        }
    }
    eprintln!("readback_nonzero_alpha_pixels={filled}");
    Ok(())
}
fn rendering(g: &GraphicsContext, fonts: &mut TextSystem, timer: Option<&Timer>) -> Result<()> {
    let supported = g
        .create_framebuffer(FramebufferOptions::new(1, 1))?
        .supported_sample_counts()
        .to_vec();
    let mut painter = Painter::new(g);
    let paragraph = (0..32)
        .map(|_| "office AV e\u{301} العربية 123 repeated words. ".repeat(4))
        .collect::<Vec<_>>()
        .join("\n");
    for samples in [1, 4] {
        if !supported.contains(&samples) {
            continue;
        }
        let mut target = g.create_framebuffer(
            FramebufferOptions::new(SIZE[0], SIZE[1])
                .sample_count(samples)
                .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
        )?;
        painter.prepare(&target.render_format())?;
        for case in [
            "empty",
            "paragraph",
            "labels_600",
            "mixed_color_600",
            "update_60_of_600",
        ] {
            let mut buffers = Vec::new();
            let mut texts = Vec::new();
            if case == "paragraph" {
                let p = painter.prepare_text(
                    layout(fonts, &paragraph, 18., "Source Sans 3", Some(1888.))?.as_ref(),
                    TextRasterOptions::new(),
                )?;
                visible(&p, [16., 16.]);
                texts.push((p, [16., 16.]));
            } else if case == "mixed_color_600" {
                let p = painter.prepare_text(
                    layout(fonts, "M😀M😁M", 18., "Astrelis Test Color", None)?.as_ref(),
                    TextRasterOptions::new(),
                )?;
                for i in 0..LABELS {
                    visible(&p, origin(i));
                    texts.push((p.clone(), origin(i)));
                }
            } else if case != "empty" {
                for i in 0..LABELS {
                    let mut b = TextBuffer::new();
                    b.set_text(
                        &format!("Item {i:05}"),
                        TextStyle::new()
                            .family("Source Sans 3")
                            .font_size(14.)
                            .line_height(21.),
                    )?;
                    let p = painter
                        .prepare_text(b.layout(fonts)?.as_ref(), TextRasterOptions::new())?;
                    visible(&p, origin(i));
                    texts.push((p, origin(i)));
                    buffers.push(b);
                }
            }
            wait(g, g.queue().submit([]))?;
            let label = format!("{case}_{samples}x");
            let glyphs: usize = texts.iter().map(|(t, _)| t.glyph_count()).sum();
            let mut records = Vec::new();
            let mut totals = Vec::new();
            let mut gpus = Vec::new();
            let mut updates = Vec::new();
            let mut layouts = Vec::new();
            let mut preparations = Vec::new();
            let mut batches = 0;
            for sample in 0..WARMUP + SAMPLES {
                if case == "update_60_of_600" {
                    let before_update = painter.text().stats();
                    let start = Instant::now();
                    let mut layout_time = Duration::ZERO;
                    let mut preparation_time = Duration::ZERO;
                    for j in 0..60 {
                        let layout_start = Instant::now();
                        let i = (sample * 60 + j) % LABELS;
                        buffers[i].set_text(
                            &format!("Item {:05}", 10000 + sample * 60 + j),
                            TextStyle::new()
                                .family("Source Sans 3")
                                .font_size(14.)
                                .line_height(21.),
                        )?;
                        let layout = buffers[i].layout(fonts)?;
                        layout_time += layout_start.elapsed();
                        let preparation_start = Instant::now();
                        let next =
                            painter.prepare_text(layout.as_ref(), TextRasterOptions::new())?;
                        preparation_time += preparation_start.elapsed();
                        visible(&next, origin(i));
                        texts[i].0 = next;
                    }
                    let elapsed = us(start.elapsed());
                    let after_update = painter.text().stats();
                    if sample >= WARMUP {
                        assert_eq!(before_update.cache_misses, after_update.cache_misses);
                        assert_eq!(before_update.uploaded_bytes, after_update.uploaded_bytes);
                        assert_eq!(
                            before_update.geometry_buffer_allocations,
                            after_update.geometry_buffer_allocations
                        );
                        assert_eq!(
                            after_update.geometry_buffer_reuses
                                - before_update.geometry_buffer_reuses,
                            60
                        );
                        updates.push(elapsed);
                        layouts.push(us(layout_time));
                        preparations.push(us(preparation_time));
                    }
                    wait(g, g.queue().submit([]))?;
                }
                let before = painter.text().stats();
                let (record, total, gpu) = frame(g, &mut target, &mut painter, &texts, timer)?;
                let after = painter.text().stats();
                assert_eq!(before.cache_misses, after.cache_misses);
                assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
                assert_eq!(before.geometry_bytes, after.geometry_bytes);
                assert_eq!(
                    after.parameter_bytes - before.parameter_bytes,
                    texts.len() as u64 * 48
                );
                batches = after.draw_calls - before.draw_calls;
                if sample >= WARMUP {
                    records.push(record);
                    totals.push(total);
                    if let Some(gpu) = gpu {
                        gpus.push(gpu);
                    }
                }
            }
            if timer.is_some() && gpus.len() != SAMPLES {
                eprintln!(
                    "case={label}; incomplete_gpu_timestamps={}/{SAMPLES}; GPU results withheld",
                    gpus.len()
                );
                gpus.clear();
            }
            eprintln!("case={label}; valid_measured_gpu_samples={}", gpus.len());
            print(&label, "record_cpu", records);
            print(&label, "frame_cpu", totals);
            print(&label, "pass_gpu", gpus);
            if case == "update_60_of_600" {
                print(&label, "content_layout_prepare_cpu", updates);
                print(&label, "content_layout_cpu", layouts);
                print(&label, "cached_glyph_geometry_cpu", preparations);
            }
            eprintln!(
                "case={label}; drawable_glyphs={glyphs}; text_calls={}; page_batches_per_frame={batches}; stats={:?}",
                texts.len(),
                painter.text().stats()
            );
            if case != "empty" {
                check_pixels(g, &mut target, case != "paragraph")?;
            }
        }
    }
    Ok(())
}
fn preparation(g: &GraphicsContext, fonts: &mut TextSystem) -> Result<()> {
    let mut unique = String::new();
    // Select distinct nominal glyphs actually present in the two bundled font faces.
    for (probe, family) in [("A", "Source Sans 3"), ("م", "Noto Sans Arabic")] {
        let probe = layout(fonts, probe, 20., family, None)?;
        let font = &probe.fonts()[0];
        let face = swash::FontRef::from_index(font.data(), font.face_index() as usize)
            .ok_or("invalid font")?;
        let mut ids = HashSet::new();
        for code in 0x21..0x3000 {
            if let Some(c) = char::from_u32(code)
                && !c.is_control()
                && !c.is_whitespace()
            {
                let id = face.charmap().map(c);
                if id != 0 && ids.insert(id) {
                    unique.push(c);
                    unique.push(' ');
                }
            }
        }
    }
    let unique_layout = layout(fonts, &unique, 20., "Source Sans 3", Some(1888.))?;
    let unique_keys: HashSet<_> = unique_layout
        .glyphs()
        .iter()
        .filter(|g| g.glyph_id != 0)
        .map(|g| (unique_layout.fonts()[g.font_index].id(), g.glyph_id))
        .collect();
    eprintln!(
        "unique_workload_scalars={}; distinct_shaped_face_glyph_pairs={}",
        unique.chars().count(),
        unique_keys.len()
    );
    for case in ["cold_unique", "cached_unique", "raster_size_pressure"] {
        let mut renderer = TextRenderer::with_options(
            g,
            if case == "raster_size_pressure" {
                TextRendererOptions {
                    page_size: 256,
                    max_pages: 2,
                    ..Default::default()
                }
            } else {
                Default::default()
            },
        )?;
        let small = layout(
            fonts,
            "office AV e\u{301} العربية 123",
            16.,
            "Source Sans 3",
            None,
        )?;
        let selected = if case == "raster_size_pressure" {
            &small
        } else {
            &unique_layout
        };
        if case == "cached_unique" {
            drop(renderer.prepare_text(selected, TextRasterOptions::new())?);
            wait(g, g.queue().submit([]))?;
        }
        let mut durations = Vec::new();
        let mut max_pages = 0;
        let mut max_bytes = 0;
        let mut misses = 0;
        let mut upload = 0;
        let mut geometry = 0;
        let mut retries = 0;
        let mut measured_retries = 0;
        for sample in 0..WARMUP + SAMPLES {
            if case == "cold_unique" {
                renderer.clear_cache();
            }
            let before = renderer.stats();
            let raster = TextRasterOptions::new().raster_scale(if case == "raster_size_pressure" {
                1. + sample as f32 / 16.
            } else {
                1.
            });
            let start = Instant::now();
            let prepared = match renderer.prepare_text(selected, raster) {
                Err(TextRenderError::AtlasFull) if case == "raster_size_pressure" => {
                    // Append-only packing can strand free space across pages. No
                    // previous PreparedText or recording is retained in this case.
                    // Explicitly release cached pages and retry without increasing
                    // the budget or hiding a completion wait in preparation.
                    let occupied = renderer.stats();
                    max_pages = max_pages.max(occupied.live_pages);
                    max_bytes = max_bytes.max(occupied.atlas_bytes);
                    retries += 1;
                    if sample >= WARMUP {
                        measured_retries += 1;
                    }
                    renderer.clear_cache();
                    renderer.prepare_text(selected, raster)?
                }
                result => result?,
            };
            let elapsed = us(start.elapsed());
            let after = renderer.stats();
            assert!(prepared.glyph_count() > 0);
            if case == "cached_unique" {
                assert_eq!(before.cache_misses, after.cache_misses);
                assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
            }
            if case == "raster_size_pressure" {
                assert!(after.live_pages <= 2);
            }
            max_pages = max_pages.max(after.live_pages);
            max_bytes = max_bytes.max(after.atlas_bytes);
            if sample >= WARMUP {
                durations.push(elapsed);
                misses += after.cache_misses - before.cache_misses;
                upload += after.uploaded_bytes - before.uploaded_bytes;
                geometry += after.geometry_bytes - before.geometry_bytes;
            }
            drop(prepared);
            wait(g, g.queue().submit([]))?;
        }
        print(case, "prepare_cpu", durations);
        eprintln!(
            "case={case}; measured_cache_misses={misses}; measured_atlas_payload_bytes={upload}; measured_geometry_bytes={geometry}; peak_live_pages={max_pages}; peak_atlas_bytes={max_bytes}; atlas_full_retries={retries}; measured_atlas_full_retries={measured_retries}; stats={:?}",
            renderer.stats()
        );
    }
    Ok(())
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("use cargo bench --bench text_workloads".into());
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .iter()
        .any(|a| a != "--no-timestamps" && a != "--bench")
    {
        return Err("only --no-timestamps is supported".into());
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let supported = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let timestamps = supported && !args.iter().any(|a| a == "--no-timestamps");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: if timestamps {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        },
        ..Default::default()
    }))?;
    let g = GraphicsContext::from_wgpu(instance, adapter, device, queue);
    let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
    eprintln!(
        "adapter={:?}; size={SIZE:?}; samples={SAMPLES}; warmup={WARMUP}; timestamps_supported={supported}; timestamps_enabled={timestamps}; timestamp_period_ns={}",
        g.adapter().get_info(),
        g.queue().get_timestamp_period()
    );
    let timer = timestamps.then(|| Timer::new(&g));
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/TestColor.ttf"))?;
    println!("case,stage,median_us,p95_us");
    rendering(&g, &mut fonts, timer.as_ref())?;
    preparation(&g, &mut fonts)?;
    if let Some(error) = pollster::block_on(errors.pop()) {
        return Err(error.into());
    }
    Ok(())
}
