//! Sustained application-owned mixed UI recording with bounded submissions.
use astrelis::{
    CornerRadii, Framebuffer, FramebufferOptions, GraphicsContext, MarkerDraw, Painter, Path,
    PathDraw, PathOptions, Point2D, PointBuffer, PointBufferOptions, PolylineDraw, PreparedPath,
    PreparedText, Rect, TextBuffer, TextDraw, TextRasterOptions, TextStyle, TextSystem,
    TextureBinding, TextureBindingOptions, TextureDraw, TextureOptions, Transform2D, wgpu,
};
use std::{
    collections::VecDeque,
    sync::mpsc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SIZE: [u32; 2] = [1600, 900];
const CARDS: usize = 60;
const WAIT: Duration = Duration::from_secs(30);
fn wait(g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(WAIT),
    })?;
    Ok(())
}
fn origin(i: usize) -> [f32; 2] {
    [16. + (i % 12) as f32 * 130., 16. + (i / 12) as f32 * 172.]
}
struct Scene {
    painter: Painter,
    fonts: TextSystem,
    buffers: Vec<TextBuffer>,
    texts: Vec<PreparedText>,
    image: TextureBinding,
    path: PreparedPath,
    points: PointBuffer,
}
fn image(g: &GraphicsContext, painter: &mut Painter, side: u32) -> Result<TextureBinding> {
    let texture =
        g.create_texture(TextureOptions::new(side, side).format(wgpu::TextureFormat::Rgba8Unorm))?;
    texture.write(&[30, 210, 60, 255].repeat((side * side) as usize))?;
    Ok(painter
        .textures()
        .create_binding(texture.view(), TextureBindingOptions::new())?)
}
fn path(painter: &mut Painter, shift: f32) -> Result<PreparedPath> {
    let mut b = Path::builder();
    b.move_to([0., 0.])
        .line_to([24., 0.])
        .quadratic_to([24. + shift, 24.], [0., 24.])
        .close();
    Ok(painter.prepare_path(&b.build()?, PathOptions::new())?)
}
impl Scene {
    fn new(g: &GraphicsContext, target: &Framebuffer) -> Result<Self> {
        let mut painter = Painter::new(g);
        let mut fonts = TextSystem::new();
        fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
        let mut buffers = Vec::new();
        let mut texts = Vec::new();
        for i in 0..CARDS {
            let mut b = TextBuffer::new();
            b.set_text(
                &format!("Item {i:05}"),
                TextStyle::new()
                    .family("Source Sans 3")
                    .font_size(14.)
                    .line_height(20.),
            )?;
            texts.push(
                painter.prepare_text(b.layout(&mut fonts)?.as_ref(), TextRasterOptions::new())?,
            );
            buffers.push(b);
        }
        let image = image(g, &mut painter, 32)?;
        let path = path(&mut painter, 0.)?;
        let mut points = g.create_point_buffer(PointBufferOptions::new(64))?;
        points.replace(
            &(0..64)
                .map(|i| Point2D::new([i as f32 * 1.4, 8. + (i as f32 * 0.2).sin() * 6.]))
                .collect::<Vec<_>>(),
        )?;
        painter.prepare(&target.render_format())?;
        painter.prepare_image(&image, &target.render_format())?;
        Ok(Self {
            painter,
            fonts,
            buffers,
            texts,
            image,
            path,
            points,
        })
    }
    fn update(&mut self, g: &GraphicsContext, frame: usize) -> Result<()> {
        // Deterministic 10% label replacement; old resources survive until replacement succeeds.
        for i in (frame % 10..CARDS).step_by(10) {
            self.buffers[i].set_text(
                &format!("Item {:05}", (frame * CARDS + i) % 100000),
                TextStyle::new()
                    .family("Source Sans 3")
                    .font_size(14.)
                    .line_height(20.),
            )?;
            let next = self.painter.prepare_text(
                self.buffers[i].layout(&mut self.fonts)?.as_ref(),
                TextRasterOptions::new(),
            )?;
            self.texts[i] = next;
        }
        self.image = image(
            g,
            &mut self.painter,
            if frame.is_multiple_of(2) { 256 } else { 32 },
        )?;
        self.path = path(&mut self.painter, (frame % 8) as f32)?;
        self.points.replace(
            &(0..64)
                .map(|i| Point2D::new([i as f32 * 1.4, 8. + ((i + frame) as f32 * 0.2).sin() * 6.]))
                .collect::<Vec<_>>(),
        )?;
        Ok(())
    }
    fn record(&mut self, target: &mut Framebuffer, spike: bool) -> Result<wgpu::SubmissionIndex> {
        let mut frame = target.begin_frame()?;
        {
            let mut pass = frame.render_pass().begin()?;
            let mut paint = self.painter.begin(&mut pass)?;
            for i in 0..CARDS {
                let [x, y] = origin(i);
                let mut outer = paint
                    .clipped_rounded(Rect::new(x, y, 124., 160.), CornerRadii::uniform(14.))?;
                outer.fill_rect(Rect::new(x, y, 124., 160.), [0.15, 0.18, 0.22, 1.])?;
                let mut inner = outer.clipped_rounded(
                    Rect::new(x + 8., y + 8., 108., 144.),
                    CornerRadii::uniform(8.),
                )?;
                inner.draw_image(
                    &self.image,
                    TextureDraw::new(Rect::new(x + 16., y + 20., 24., 24.)),
                )?;
                inner.draw_path(
                    &self.path,
                    PathDraw::new([0.9, 0.1, 0.1, 1.])
                        .transform(Transform2D::translation(x + 64., y + 20.)),
                )?;
                inner.draw_text(&self.texts[i], TextDraw::new([x + 12., y + 60.]))?;
                let transform = Transform2D::translation(x + 16., y + 105.);
                inner.draw_polyline(
                    &self.points,
                    PolylineDraw::new([0.1, 0.4, 1., 1.])
                        .width_pixels(1.5)
                        .transform(transform),
                )?;
                inner.draw_markers(
                    &self.points,
                    MarkerDraw::new([0.1, 0.4, 1., 1.])
                        .radius_pixels(1.5)
                        .transform(transform),
                )?;
                // Deliberately cross the inner boundary: nested coverage must be active.
                inner.fill_rect(Rect::new(x + 100., y + 130., 40., 40.), [0.8, 0.5, 0.1, 1.])?;
            }
            if spike {
                let draws =
                    vec![astrelis::ShapeDraw::rect(Rect::new(1500., 880., 2., 2.), [1.; 4]); 12000];
                paint.draw_shapes(&draws)?;
            }
        }
        Ok(frame.finish()?)
    }
}
fn read(g: &GraphicsContext, target: &mut Framebuffer) -> Result<Vec<u8>> {
    let texture = target.color_texture()?.clone();
    let pitch = (SIZE[0] * 4).div_ceil(256) * 256;
    let buffer = g.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("mixed UI verification"),
        size: u64::from(pitch * SIZE[1]),
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
    let index = frame.finish()?;
    let (tx, rx) = mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    wait(g, index)?;
    rx.recv_timeout(WAIT)??;
    let bytes = buffer.slice(..).get_mapped_range()?.to_vec();
    buffer.unmap();
    for i in [0, CARDS - 1] {
        let [x, y] = origin(i).map(|v| v as usize);
        let pixel = |dx, dy| &bytes[(y + dy) * pitch as usize + (x + dx) * 4..][..4];
        assert_eq!(pixel(0, 0)[3], 0, "rounded outer corner");
        assert_eq!(pixel(20, 24), [30, 210, 60, 255], "image");
        assert!(pixel(68, 24)[0] > 200, "path");
        assert!(
            pixel(120, 140)[0] < 100,
            "inner clip rejected crossing shape"
        );
        assert!(
            (60..80).any(|dy| (12..100).any(|dx| pixel(dx, dy)[0] > 100)),
            "visible text"
        );
        assert!(
            (103..122).any(|dy| (16..104).any(|dx| pixel(dx, dy)[2] > 200)),
            "visible points"
        );
    }
    Ok(bytes)
}
fn row(phase: &str, stage: &str, values: &mut [f64]) {
    values.sort_by(f64::total_cmp);
    let q = |p: f64| values[((values.len() as f64 * p).ceil() as usize).saturating_sub(1)];
    println!(
        "{phase},{stage},{},{:.3},{:.3},{:.3},{:.3}",
        values.len(),
        q(0.5),
        q(0.95),
        q(0.99),
        values[values.len() - 1]
    );
}
fn main() -> Result<()> {
    let mut frames = 1800usize;
    let mut warmup = 60usize;
    let mut flight = 3usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--frames" => frames = args.next().ok_or("missing frames")?.parse()?,
            "--warmup" => warmup = args.next().ok_or("missing warmup")?.parse()?,
            "--in-flight" => flight = args.next().ok_or("missing in-flight")?.parse()?,
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if frames == 0 || warmup == 0 || !(2..=8).contains(&flight) {
        return Err("frames/warmup must be positive; in-flight must be 2..=8".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
    eprintln!(
        "adapter={:?}; limits={:?}; target={SIZE:?} RGBA8Unorm 1x; cards={CARDS}; points_per_card=64; seed=0; warmup={warmup}; frames_per_phase={frames}; max_in_flight={flight}; gpu_execution=not_measured; profile_debug={}; os={}; arch={}",
        g.adapter().get_info(),
        g.device().limits(),
        cfg!(debug_assertions),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let mut target = g.create_framebuffer(
        FramebufferOptions::new(SIZE[0], SIZE[1])
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )?;
    let mut scene = Scene::new(&g, &target)?;
    wait(&g, scene.record(&mut target, false)?)?;
    let reference = read(&g, &mut target)?;
    for _ in 0..warmup {
        wait(&g, scene.record(&mut target, false)?)?;
    }
    assert_eq!(
        reference,
        read(&g, &mut target)?,
        "warm reuse changed pixels"
    );
    eprintln!(
        "pixel_verification=passed; full_static_equality=true; region_probes=outer_clip,inner_clip,image,path,text,points"
    );
    println!("phase,cpu_stage,samples,p50_us,p95_us,p99_us,max_us");
    for phase in ["warm_static", "changing_churn", "recovery_static"] {
        let mut pending = VecDeque::new();
        let mut stages = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let before = g.upload_stats();
        eprintln!("phase={phase}; upload_before={before:?}");
        let phase_start = Instant::now();
        for frame in 0..frames {
            let start = Instant::now();
            if pending.len() == flight {
                wait(&g, pending.pop_front().unwrap())?;
            }
            let after_wait = Instant::now();
            if phase == "changing_churn" {
                scene.update(&g, frame)?;
            }
            let after_update = Instant::now();
            pending.push_back(scene.record(
                &mut target,
                phase == "changing_churn" && frame == frames - 1,
            )?);
            let end = Instant::now();
            for (times, duration) in stages.iter_mut().zip([
                after_wait - start,
                after_update - after_wait,
                end - after_update,
                end - start,
            ]) {
                times.push(duration.as_secs_f64() * 1e6);
            }
        }
        for index in pending {
            wait(&g, index)?;
        }
        let elapsed = phase_start.elapsed().as_secs_f64();
        for (stage, values) in [
            "backpressure",
            "update_prepare",
            "record_upload_submit",
            "frame_wall",
        ]
        .into_iter()
        .zip(&mut stages)
        {
            row(phase, stage, values);
        }
        eprintln!(
            "phase={phase}; completed_frames={frames}; drained_wall_s={elapsed:.6}; completed_frames_per_s={:.3}; text_stats={:?}",
            frames as f64 / elapsed,
            scene.painter.text().stats()
        );
        let after = g.upload_stats();
        eprintln!("phase={phase}; upload_after={after:?}");
        if phase == "warm_static" && warmup >= 4 {
            assert_eq!(
                before.pages_created, after.pages_created,
                "warm page allocation"
            );
            assert_eq!(
                before.clip_bindings_created, after.clip_bindings_created,
                "warm clip binding creation"
            );
        }
        if phase == "recovery_static" && frames > 121 {
            assert!(
                after.pages_released > before.pages_released,
                "spike pages did not expire"
            );
            assert!(
                after.idle_buffer_bytes < before.idle_buffer_bytes,
                "spike capacity did not recover"
            );
        }
        read(&g, &mut target)?;
    }
    wait(&g, g.queue().submit([]))?;
    if let Some(error) = pollster::block_on(errors.pop()) {
        return Err(error.into());
    }
    Ok(())
}
