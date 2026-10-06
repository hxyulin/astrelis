//! Streaming charts with retained point buffers and optional application-side min/max reduction.
//! 1/2/3 selects 10K/100K/1M samples. Wheel zooms; drag pans; F follows the tail.
//! D toggles explicit reduction, M markers, P pause, A coverage, Space supported MSAA.
//! Run in release mode. This file owns its window, data policy, and event loop.
//! Copy into a binary with astrelis, winit 0.30, and pollster 0.4.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    EdgeAntialiasing, Error, FrameError, GraphicsContext, LineJoin, MarkerDraw, Painter, Point2D,
    PointBuffer, PointBufferOptions, PolylineDraw, PreparedText, Rect, RenderTarget,
    SurfaceOptions, TextBuffer, TextDraw, TextRasterOptions, TextStyle, TextSystem, Transform2D,
    wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

// Small leaves bound partial-range scans while appending still rebuilds only
// affected leaves and their ancestors. The application chooses this tradeoff.
const BLOCK: usize = 32;
const NONE: usize = usize::MAX;
#[derive(Clone, Copy)]
struct Summary {
    first: usize,
    last: usize,
    min: usize,
    max: usize,
    gap: bool,
}
impl Default for Summary {
    fn default() -> Self {
        Self {
            first: NONE,
            last: NONE,
            min: NONE,
            max: NONE,
            gap: false,
        }
    }
}
// Chart-owned CPU data and a hierarchy over physical blocks. No reduction policy
// is imposed by Astrelis. Block summaries update only where incoming samples land.
struct History {
    data: Vec<Point2D>,
    head: usize,
    len: usize,
    next: u64,
    tree: Vec<Summary>,
    leaves: usize,
    changed: Vec<usize>,
}
impl History {
    fn new(capacity: usize) -> Self {
        let leaves = capacity.div_ceil(BLOCK).next_power_of_two();
        Self {
            data: vec![Point2D::gap(); capacity],
            head: 0,
            len: 0,
            next: 0,
            tree: vec![Summary::default(); leaves * 2],
            leaves,
            changed: Vec::with_capacity(capacity.div_ceil(BLOCK) + 1),
        }
    }
    fn combine(&self, a: Summary, b: Summary) -> Summary {
        let mut result = if a.first == NONE {
            b
        } else if b.first == NONE {
            a
        } else {
            Summary {
                first: a.first,
                last: b.last,
                min: if self.y(a.min) <= self.y(b.min) {
                    a.min
                } else {
                    b.min
                },
                max: if self.y(a.max) >= self.y(b.max) {
                    a.max
                } else {
                    b.max
                },
                gap: false,
            }
        };
        result.gap = a.gap || b.gap;
        result
    }
    fn y(&self, i: usize) -> f32 {
        self.data[i].position().unwrap()[1]
    }
    fn scan(&self, range: std::ops::Range<usize>) -> Summary {
        range.fold(Summary::default(), |s, i| {
            self.combine(
                s,
                if self.data[i].position().is_some() {
                    Summary {
                        first: i,
                        last: i,
                        min: i,
                        max: i,
                        gap: false,
                    }
                } else {
                    Summary {
                        gap: true,
                        ..Summary::default()
                    }
                },
            )
        })
    }
    fn rebuild(&mut self, block: usize) {
        let start = block * BLOCK;
        let end = (start + BLOCK).min(self.data.len());
        let mut i = self.leaves + block;
        self.tree[i] = self.scan(start..end);
        while i > 1 {
            i /= 2;
            self.tree[i] = self.combine(self.tree[i * 2], self.tree[i * 2 + 1]);
        }
    }
    fn append(&mut self, points: &[Point2D]) {
        self.changed.clear();
        for &p in points {
            let physical = (self.head + self.len) % self.data.len();
            self.data[physical] = p;
            if self.len == self.data.len() {
                self.head = (self.head + 1) % self.data.len();
            } else {
                self.len += 1;
            }
            if self.changed.last() != Some(&(physical / BLOCK)) {
                self.changed.push(physical / BLOCK);
            }
        }
        for i in 0..self.changed.len() {
            self.rebuild(self.changed[i]);
        }
    }
    fn generate(&mut self, count: usize, out: &mut Vec<Point2D>) {
        out.clear();
        for _ in 0..count {
            let x = self.next as f32;
            let y = (x * 0.003).sin() * 0.55 + (x * 0.031).sin() * 0.16 + (x * 0.113).sin() * 0.04;
            out.push(if self.next % 50003 == 25000 {
                Point2D::gap()
            } else {
                Point2D::new([x, if self.next % 9973 == 41 { 0.95 } else { y }])
            });
            self.next += 1;
        }
        self.append(out);
    }
    fn physical_query(&self, start: usize, end: usize) -> Summary {
        if start >= end {
            return Summary::default();
        }
        let full_start = start.div_ceil(BLOCK);
        let full_end = end / BLOCK;
        if full_start >= full_end {
            return self.scan(start..end);
        }
        let mut result = self.scan(start..full_start * BLOCK);
        let mut left = full_start + self.leaves;
        let mut right = full_end + self.leaves;
        let mut suffix = Summary::default();
        while left < right {
            if left % 2 == 1 {
                result = self.combine(result, self.tree[left]);
                left += 1;
            }
            if right % 2 == 1 {
                right -= 1;
                suffix = self.combine(self.tree[right], suffix);
            }
            left /= 2;
            right /= 2;
        }
        result = self.combine(result, suffix);
        self.combine(result, self.scan(full_end * BLOCK..end))
    }
    fn query(&self, start: usize, end: usize) -> Summary {
        let physical = (self.head + start) % self.data.len();
        let count = end - start;
        let split = count.min(self.data.len() - physical);
        self.combine(
            self.physical_query(physical, physical + split),
            self.physical_query(0, count - split),
        )
    }
    fn at(&self, logical: usize) -> Point2D {
        self.data[(self.head + logical) % self.data.len()]
    }
    fn reduce(&self, range: std::ops::Range<usize>, columns: usize, out: &mut Vec<Point2D>) {
        out.clear();
        let count = range.end - range.start;
        if count == 0 {
            return;
        }
        let columns = columns.max(1).min(count);
        let mut previous = NONE;
        for column in 0..columns {
            let start = range.start + column * count / columns;
            let end = range.start + (column + 1) * count / columns;
            let s = self.query(start, end);
            // Preserve exact gap placement; never connect across a missing sample.
            if s.gap {
                for i in start..end {
                    out.push(self.at(i));
                }
                previous = NONE;
                continue;
            }
            if s.first == NONE {
                continue;
            }
            let mut candidates = [s.first, s.min, s.max, s.last];
            candidates
                .sort_unstable_by_key(|i| (*i + self.data.len() - self.head) % self.data.len());
            for i in candidates {
                if i != previous {
                    out.push(self.data[i]);
                    previous = i;
                }
            }
        }
    }
}
struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    painter: Painter,
    points: PointBuffer,
    reduced: PointBuffer,
    history: History,
    scratch: Vec<Point2D>,
    reduction: Vec<Point2D>,
    labels: Vec<PreparedText>,
    coverage: bool,
    decimate: bool,
    markers: bool,
    paused: bool,
    dirty: bool,
    span: usize,
    offset: usize,
    cursor: f64,
    drag: Option<(f64, usize)>,
    next_update: Instant,
    next_title: Instant,
}
impl State {
    fn new(events: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            events.create_window(
                Window::default_attributes()
                    .with_title("Astrelis streaming charts")
                    .with_inner_size(PhysicalSize::new(1280, 720)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut painter = Painter::new(&graphics);
        painter.prepare_for_target(&target)?;
        painter.prepare_points(&target.render_format())?;
        let mut history = History::new(100_000);
        let mut scratch = Vec::with_capacity(100_000);
        history.generate(100_000, &mut scratch);
        let mut points = graphics.create_point_buffer(PointBufferOptions::new(100_000).ring())?;
        points.replace(&scratch)?;
        let reduced = graphics.create_point_buffer(PointBufferOptions::new(100_000))?;
        let mut fonts = TextSystem::new();
        let loaded = fonts.load_system_fonts();
        let mut labels = Vec::new();
        if !loaded.is_empty() {
            for text in [
                "1 / 2 / 3: 10K / 100K / 1M history    Wheel: zoom    Drag: pan    F: follow",
                "D: reduction    M: markers    P: pause    A: coverage    Space: MSAA",
                "1.0",
                "0.5",
                "0",
                "-0.5",
                "-1.0",
            ] {
                let mut buffer = TextBuffer::new();
                buffer.set_text(text, TextStyle::new().font_size(16.).line_height(22.))?;
                let layout = buffer.layout(&mut fonts)?;
                labels.push(painter.prepare_text(&layout, TextRasterOptions::new())?);
            }
        }
        let state = Self {
            window,
            graphics,
            target,
            painter,
            points,
            reduced,
            history,
            scratch,
            reduction: Vec::with_capacity(100_000),
            labels,
            coverage: true,
            decimate: true,
            markers: false,
            paused: false,
            dirty: true,
            span: 20_000,
            offset: 0,
            cursor: 0.,
            drag: None,
            next_update: Instant::now() + Duration::from_millis(16),
            next_title: Instant::now(),
        };
        state.window.request_redraw();
        Ok(state)
    }
    fn reset(&mut self, capacity: usize) -> Result<(), Error> {
        self.history = History::new(capacity);
        self.scratch = Vec::with_capacity(capacity);
        self.reduction = Vec::with_capacity(capacity);
        self.history.generate(capacity, &mut self.scratch);
        self.points = self
            .graphics
            .create_point_buffer(PointBufferOptions::new(capacity).ring())?;
        self.points.replace(&self.scratch)?;
        self.reduced = self
            .graphics
            .create_point_buffer(PointBufferOptions::new(capacity))?;
        self.span = capacity;
        self.offset = 0;
        self.dirty = true;
        self.next_update = Instant::now() + Duration::from_millis(16);
        Ok(())
    }
    fn toggle_msaa(&mut self) -> Result<(), Error> {
        if self.target.supported_sample_counts().contains(&4) {
            self.target
                .set_sample_count(if self.target.sample_count() == 1 {
                    4
                } else {
                    1
                })?;
            self.painter.prepare_for_target(&self.target)?;
            self.painter.prepare_points(&self.target.render_format())?;
        }
        Ok(())
    }
    fn plot(&self) -> Rect {
        let [w, h] = self.target.size();
        Rect::new(
            64.,
            72.,
            (w as f32 - 88.).max(1.),
            (h as f32 - 104.).max(1.),
        )
    }
    fn range(&self) -> std::ops::Range<usize> {
        let end = self.history.len - self.offset.min(self.history.len.saturating_sub(2));
        let start = end.saturating_sub(self.span.max(2));
        start..end
    }
    fn prepare_frame(&mut self) -> Result<(), Error> {
        let now = Instant::now();
        if !self.paused && now >= self.next_update {
            self.history.generate(256, &mut self.scratch);
            self.points.append(&self.scratch)?;
            if self.offset > 0 {
                self.offset = (self.offset + 256).min(self.history.len.saturating_sub(2));
            }
            self.next_update = now + Duration::from_millis(16);
            self.dirty = true;
        }
        if self.dirty {
            let range = self.range();
            let plot = self.plot();
            if self.decimate {
                self.history
                    .reduce(range.clone(), plot.width as usize, &mut self.reduction);
                self.reduced.replace(&self.reduction)?;
            }
            let rendered = if self.decimate {
                self.reduction.len()
            } else {
                range.len()
            };
            if now >= self.next_title || self.paused {
                self.next_title = now + Duration::from_secs(1);
                self.window.set_title(&format!(
                    "Astrelis — {} stored / {} visible / {} rendered — {}{}",
                    self.history.len,
                    range.len(),
                    rendered,
                    if self.decimate { "min/max" } else { "raw" },
                    if self.paused { " (paused)" } else { "" }
                ));
            }
            self.dirty = false;
        }
        Ok(())
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
    failure: Option<Box<dyn StdError>>,
    retry_at: Option<Instant>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn StdError>>) {
        self.failure = Some(error.into());
        event_loop.exit();
    }

    fn redraw(&mut self) -> Result<(), Box<dyn StdError>> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        if state.target.size().contains(&0) {
            return Ok(());
        }
        state.prepare_frame()?;
        let plot = state.plot();
        let range = state.range();
        let frame_size = state.target.size();
        let mut frame = match state.target.begin_frame() {
            Ok(frame) => frame,
            Err(FrameError::Retry) => {
                self.retry_at = Some(Instant::now() + Duration::from_millis(16));
                return Ok(());
            }
            Err(FrameError::Suspended) => {
                self.retry_at = None;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        {
            let domain_start =
                (state.history.next - state.history.len as u64 + range.start as u64) as f64;
            let sx = f64::from(plot.width) / (range.len().saturating_sub(1).max(1) as f64);
            let transform = Transform2D([
                sx as f32,
                0.,
                0.,
                -plot.height * 0.45,
                (f64::from(plot.x) - domain_start * sx) as f32,
                plot.y + plot.height * 0.5,
            ]);
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.012,
                    g: 0.02,
                    b: 0.035,
                    a: 1.,
                })
                .begin()?;
            let mut paint = state.painter.begin(&mut pass)?;
            paint.fill_rect(plot, [0.025, 0.04, 0.065, 1.])?;
            for i in 0..=4 {
                let y = plot.y + plot.height * (0.5 - (1. - i as f32 * 0.5) * 0.45);
                paint.fill_rect(Rect::new(plot.x, y, plot.width, 1.), [0.09, 0.13, 0.18, 1.])?;
                if let Some(label) = state.labels.get(i + 2) {
                    paint.draw_text(
                        label,
                        TextDraw::new([8., y - 10.]).color([0.65, 0.75, 0.85, 1.]),
                    )?;
                }
            }
            for i in 0..=8 {
                paint.fill_rect(
                    Rect::new(plot.x + plot.width * i as f32 / 8., plot.y, 1., plot.height),
                    [0.055, 0.08, 0.12, 1.],
                )?;
            }
            for (i, label) in state.labels.iter().take(2).enumerate() {
                paint.draw_text(
                    label,
                    TextDraw::new([16., 12. + i as f32 * 24.]).color([0.75, 0.83, 0.93, 1.]),
                )?;
            }
            let [w, h] = frame_size;
            let x = plot.x as u32;
            let y = plot.y as u32;
            paint.pass().set_scissor_rect(
                x.min(w),
                y.min(h),
                (plot.width as u32).min(w.saturating_sub(x)),
                (plot.height as u32).min(h.saturating_sub(y)),
            )?;
            let aa = if state.coverage {
                EdgeAntialiasing::Coverage
            } else {
                EdgeAntialiasing::None
            };
            let points = if state.decimate {
                &state.reduced
            } else {
                &state.points
            };
            let mut draw = PolylineDraw::new([0.1, 0.7, 1., 1.])
                .width_pixels(1.5)
                .join(LineJoin::Bevel)
                .transform(transform)
                .antialiasing(aa);
            if !state.decimate {
                draw = draw.range(range.clone());
            }
            paint.draw_polyline(points, draw)?;
            if state.markers {
                let mut draw = MarkerDraw::new([1., 0.4, 0.08, 0.65])
                    .radius_pixels(2.)
                    .transform(transform)
                    .antialiasing(aa);
                if !state.decimate {
                    draw = draw.range(range);
                }
                paint.draw_markers(points, draw)?;
            }
        }
        frame.finish()?;
        self.retry_at = None;
        Ok(())
    }

    fn recreate_surface(&mut self) -> Result<(), Error> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let size = state.window.inner_size();
        let count = state.target.sample_count();
        state.target = state.graphics.create_surface(
            state.window.clone(),
            SurfaceOptions::new(size.width, size.height).sample_count(count),
        )?;
        state.painter.prepare_for_target(&state.target)?;
        state
            .painter
            .prepare_points(&state.target.render_format())?;
        state.window.request_redraw();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match State::new(event_loop) {
            Ok(state) => {
                self.state = Some(state);
            }
            Err(error) => self.fail(event_loop, error),
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
        self.retry_at = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() {
            return;
        }
        let Some(state) = &mut self.state else { return };
        if state.window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && !event.repeat =>
            {
                let result = match event.logical_key {
                    Key::Named(NamedKey::Space) => state.toggle_msaa(),
                    Key::Character(ref k) => match k.to_ascii_lowercase().as_str() {
                        "1" => state.reset(10_000),
                        "2" => state.reset(100_000),
                        "3" => state.reset(1_000_000),
                        "a" => {
                            state.coverage = !state.coverage;
                            Ok(())
                        }
                        "d" => {
                            state.decimate = !state.decimate;
                            state.dirty = true;
                            Ok(())
                        }
                        "m" => {
                            state.markers = !state.markers;
                            Ok(())
                        }
                        "p" => {
                            state.paused = !state.paused;
                            state.dirty = true;
                            state.next_update = Instant::now();
                            Ok(())
                        }
                        "f" => {
                            state.offset = 0;
                            state.dirty = true;
                            Ok(())
                        }
                        _ => Ok(()),
                    },
                    _ => Ok(()),
                };
                if let Err(error) = result {
                    self.fail(event_loop, error);
                    return;
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 60.,
                };
                state.span = ((state.span as f64 * 0.8f64.powf(delta)) as usize)
                    .clamp(32, state.history.len.max(32));
                state.dirty = true;
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: button,
                button: MouseButton::Left,
                ..
            } => {
                state.drag = if button == ElementState::Pressed {
                    Some((state.cursor, state.offset))
                } else {
                    None
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor = position.x;
                if let Some((start, offset)) = state.drag {
                    let delta =
                        (position.x - start) * state.span as f64 / f64::from(state.plot().width);
                    state.offset = (offset as f64 + delta)
                        .clamp(0., state.history.len.saturating_sub(2) as f64)
                        as usize;
                    state.dirty = true;
                    state.window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                state.dirty = true;
                if let Err(error) = state.target.resize(size.width, size.height) {
                    self.fail(event_loop, error);
                    return;
                }
                state.window.request_redraw();
            }
            WindowEvent::Occluded(false) => state.window.request_redraw(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw() {
                    if error.downcast_ref::<FrameError>() == Some(&FrameError::SurfaceLost) {
                        if let Err(error) = self.recreate_surface() {
                            self.fail(event_loop, error)
                        }
                    } else {
                        self.fail(event_loop, error);
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let update = self.state.as_ref().and_then(|s| {
            (!s.paused && s.target.size()[0] > 0 && s.target.size()[1] > 0).then_some(s.next_update)
        });
        let wake = update.into_iter().chain(self.retry_at).min();
        if wake.is_some_and(|deadline| now >= deadline) {
            self.retry_at = None;
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
        }
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

fn main() -> Result<(), Box<dyn StdError>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;
    match app.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
