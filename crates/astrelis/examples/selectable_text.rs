//! Selectable multilingual text with retained glyphs and CPU interaction geometry.
//! Owns its window/event loop; copy this file and its licensed fonts into a binary.
//! The window title reports cumulative atlas/geometry bytes; selection does not change them.

use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};

use astrelis::{
    FrameError, GraphicsContext, Painter, PreparedText, Rect, RenderTarget, SurfaceOptions,
    TextAffinity, TextBuffer, TextDraw, TextLayout, TextPosition, TextRasterOptions, TextStyle,
    TextSystem, Transform2D, wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};

struct State {
    window: Arc<Window>,
    graphics: GraphicsContext,
    target: RenderTarget<'static>,
    painter: Painter,
    fonts: TextSystem,
    buffer: TextBuffer,
    layout: Arc<TextLayout>,
    prepared: Option<PreparedText>,
    heading: PreparedText,
    anchor: TextPosition,
    focus: TextPosition,
    cursor: [f32; 2],
    dragging: bool,
    modifiers: ModifiersState,
}

impl State {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn StdError>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Astrelis selectable text")
                    .with_inner_size(LogicalSize::new(860., 620.)),
            )?,
        );
        let size = window.inner_size();
        let (graphics, target) = pollster::block_on(GraphicsContext::with_surface(
            window.clone(),
            SurfaceOptions::new(size.width, size.height),
        ))?;
        let mut fonts = TextSystem::new();
        // Native discovery supplies installed scripts/emoji beyond the bundled fixtures.
        // Applications can omit discovery and load their own deterministic font set.
        fonts.load_system_fonts();
        fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
        fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
        fonts.load_font(include_bytes!("../tests/fonts/TestColor.ttf"))?;
        let mut buffer = TextBuffer::new();
        buffer.set_text("Select this paragraph with a click and drag.

Office, ffi and AV exercise shaping and ligatures. Combining marks stay together: e\u{301}, a\u{301}.
Mixed directions: Hello العربية مرحبا 123 world.
Emoji are atomic graphemes: 😀 😁 👩‍👩‍👧‍👦 🇭🇰.

Shift-click extends selection. Left/right move visually; Shift extends. Up/down retain X. Command/Ctrl-A selects all.

Resize to reflow. Selection and caret changes reuse the retained layout and prepared glyphs. Rendering sleeps while idle.",TextStyle::new().family("Source Sans 3").font_size(22.).line_height(32.))?;
        let layout = buffer.layout(&mut fonts)?;
        let mut painter = Painter::new(&graphics);
        let mut title = TextBuffer::new();
        title.set_text(
            "Astrelis selectable text — click/drag, Shift, arrows, Ctrl/Cmd-A",
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(16.)
                .line_height(24.),
        )?;
        let title_layout = title.layout(&mut fonts)?;
        let heading = painter.prepare_text(
            &title_layout,
            TextRasterOptions::new().raster_scale(window.scale_factor() as f32),
        )?;
        let mut state = Self {
            window,
            graphics,
            target,
            painter,
            fonts,
            buffer,
            layout,
            prepared: None,
            heading,
            anchor: TextPosition::new(0),
            focus: TextPosition::new(0),
            cursor: [0.; 2],
            dragging: false,
            modifiers: ModifiersState::empty(),
        };
        state.prepare()?;
        state.window.request_redraw();
        Ok(state)
    }
    fn prepare(&mut self) -> Result<(), Box<dyn StdError>> {
        let scale = self.window.scale_factor() as f32;
        self.buffer
            .set_width(Some((self.target.size()[0] as f32 / scale - 64.).max(0.)))?;
        let layout = self.buffer.layout(&mut self.fonts)?;
        let changed = !Arc::ptr_eq(&layout, &self.layout)
            || self
                .prepared
                .as_ref()
                .is_none_or(|p| p.raster_scale() != scale);
        self.layout = layout;
        self.layout.prepare_interaction()?;
        if changed {
            self.prepared = None;
            self.prepared = Some(
                self.painter
                    .prepare_text(&self.layout, TextRasterOptions::new().raster_scale(scale))?,
            );
        }
        if self.heading.raster_scale() != scale {
            let mut title = TextBuffer::new();
            title.set_text(
                "Astrelis selectable text — click/drag, Shift, arrows, Ctrl/Cmd-A",
                TextStyle::new()
                    .family("Source Sans 3")
                    .font_size(16.)
                    .line_height(24.),
            )?;
            let layout = title.layout(&mut self.fonts)?;
            self.heading = self
                .painter
                .prepare_text(&layout, TextRasterOptions::new().raster_scale(scale))?;
        }
        self.painter.prepare_for_target(&self.target)?;
        Ok(())
    }
    fn select(&mut self, position: TextPosition, extend: bool) {
        self.focus = position;
        if !extend {
            self.anchor = position;
        }
        let stats = self.painter.text().stats();
        self.window.set_title(&format!(
            "Selection {}..{} — atlas {} B, geometry {} B",
            self.anchor.byte_offset.min(self.focus.byte_offset),
            self.anchor.byte_offset.max(self.focus.byte_offset),
            stats.uploaded_bytes,
            stats.geometry_bytes
        ));
        self.window.request_redraw();
    }
    fn hit(&self) -> Option<TextPosition> {
        self.layout
            .hit_test([self.cursor[0] - 32., self.cursor[1] - 64.])
    }
    fn key(&mut self, key: Key) {
        let shift = self.modifiers.shift_key();
        let position = match key {
            Key::Character(ref c)
                if (self.modifiers.control_key() || self.modifiers.super_key())
                    && c.eq_ignore_ascii_case("a") =>
            {
                self.anchor = TextPosition::new(0);
                Some(TextPosition::new(self.layout.text().len()).affinity(TextAffinity::Upstream))
            }
            Key::Named(NamedKey::ArrowLeft) => self.layout.visual_neighbor(self.focus, false),
            Key::Named(NamedKey::ArrowRight) => self.layout.visual_neighbor(self.focus, true),
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => {
                self.layout.caret(self.focus).and_then(|c| {
                    let direction = if key == Key::Named(NamedKey::ArrowUp) {
                        -1.
                    } else {
                        1.
                    };
                    self.layout
                        .hit_test([c.origin[0], c.origin[1] + c.height * (0.5 + direction)])
                })
            }
            _ => None,
        };
        if let Some(p) = position {
            self.select(
                p,
                shift || self.modifiers.control_key() || self.modifiers.super_key(),
            );
        }
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
            let scale = state.window.scale_factor() as f32;
            let mut pass = frame
                .render_pass()
                .clear_color(wgpu::Color {
                    r: 0.015,
                    g: 0.025,
                    b: 0.04,
                    a: 1.,
                })
                .begin()?;
            let mut paint = state.painter.begin(&mut pass)?;
            let mut logical = paint.transformed(Transform2D::scale(scale, scale))?;
            logical.draw_text(
                &state.heading,
                TextDraw::new([32., 20.]).color([0.6, 0.8, 1., 1.]),
            )?;
            let mut local = logical.transformed(Transform2D::translation(32., 64.))?;
            let range = state.anchor.byte_offset.min(state.focus.byte_offset)
                ..state.anchor.byte_offset.max(state.focus.byte_offset);
            for rect in state.layout.selection_rects(range)? {
                local.fill_rect(rect, [0.08, 0.22, 0.5, 0.8])?;
            }
            if let Some(prepared) = &state.prepared {
                local.draw_text(prepared, TextDraw::default().color([0.85, 0.9, 1., 1.]))?;
            }
            if let Some(c) = state.layout.caret(state.focus) {
                local.fill_rect(
                    Rect::new(c.origin[0], c.origin[1], 1. / scale, c.height),
                    [1., 0.5, 0.12, 1.],
                )?;
            }
        }
        frame.finish()?;
        self.retry_at = None;
        Ok(())
    }

    fn recreate_surface(&mut self) -> Result<(), Box<dyn StdError>> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let size = state.window.inner_size();
        let count = state.target.sample_count();
        state.target = state.graphics.create_surface(
            state.window.clone(),
            SurfaceOptions::new(size.width, size.height).sample_count(count),
        )?;
        state.prepare()?;
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
            WindowEvent::Resized(size) => {
                if let Err(error) = state.target.resize(size.width, size.height) {
                    self.fail(event_loop, error);
                    return;
                }
                if size.width != 0 && size.height != 0 {
                    if let Err(error) = state.prepare() {
                        self.fail(event_loop, error);
                        return;
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Err(error) = state.prepare() {
                    self.fail(event_loop, error);
                } else {
                    state.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => state.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let s = state.window.scale_factor() as f32;
                state.cursor = [position.x as f32 / s, position.y as f32 / s];
                if state.dragging
                    && let Some(p) = state.hit()
                {
                    state.select(p, true);
                }
            }
            WindowEvent::MouseInput {
                state: pressed,
                button: MouseButton::Left,
                ..
            } => {
                state.dragging = pressed == ElementState::Pressed;
                if state.dragging
                    && let Some(p) = state.hit()
                {
                    state.select(p, state.modifiers.shift_key());
                }
            }
            WindowEvent::Focused(false) => state.dragging = false,
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                state.key(event.logical_key)
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
        if self.retry_at.is_some_and(|deadline| now >= deadline) {
            self.retry_at = None;
            if let Some(state) = &self.state {
                state.window.request_redraw()
            }
        }
        let wake_at = self.retry_at;
        event_loop.set_control_flow(wake_at.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
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
