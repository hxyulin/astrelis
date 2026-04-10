//! egui integration for the `astrelis-gpu-wgpu` backend.
//!
//! Provides [`EguiIntegration`], which bridges astrelis-window events and
//! wgpu-based rendering with egui's immediate-mode UI.
//!
//! # Usage
//!
//! ```ignore
//! // During initialization:
//! let egui = EguiIntegration::new(&wgpu_device, surface_format);
//!
//! // In your event handler:
//! let consumed = egui.handle_window_event(&event);
//!
//! // In your render loop:
//! egui.begin_frame(&window);
//! egui::Window::new("Demo").show(egui.context(), |ui| {
//!     ui.label("Hello!");
//! });
//! egui.end_frame_and_render(&wgpu_device, &mut encoder, &view, screen_desc, Some(&window));
//! ```

#![warn(missing_docs)]

mod convert;

use astrelis_window::event::{ElementState, ImeEvent, KeyEvent, WindowEvent};
use astrelis_window::keyboard::Key;
use astrelis_window::mouse::MouseScrollDelta;
use astrelis_window::window::Window;

use astrelis_gpu_wgpu::WgpuDevice;

pub use egui;
pub use egui_wgpu;

/// Integrates egui with the astrelis windowing and wgpu rendering backends.
///
/// Manages the egui context, translates window events to egui input, and
/// renders egui output into a wgpu render pass.
///
/// # Clipboard
///
/// Clipboard paste is supported via [`set_clipboard_text`](Self::set_clipboard_text).
/// Call it each frame (or when the system clipboard changes) to supply the
/// current clipboard contents. Without it, `Ctrl+V` / `Cmd+V` will fall
/// through as a regular key event.
pub struct EguiIntegration {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    raw_input: egui::RawInput,
    pointer_pos: Option<egui::Pos2>,
    current_cursor_icon: egui::CursorIcon,
    scale_factor: f32,
    clipboard_text: Option<String>,
}

impl EguiIntegration {
    /// Creates a new egui integration.
    ///
    /// - `device`: The wgpu device, used to create the egui-wgpu renderer.
    /// - `surface_format`: The texture format of the render target surface.
    ///   Use the format returned by [`GpuSurface::preferred_format()`](astrelis_gpu_wgpu::convert::types::texture_format).
    pub fn new(device: &WgpuDevice, surface_format: wgpu::TextureFormat) -> Self {
        astrelis_profiling::profile_function!();
        let context = egui::Context::default();
        context.set_visuals(egui::Visuals::dark());

        let renderer = egui_wgpu::Renderer::new(
            device.wgpu_device(),
            surface_format,
            egui_wgpu::RendererOptions::default(),
        );

        Self {
            context,
            renderer,
            raw_input: egui::RawInput::default(),
            pointer_pos: None,
            current_cursor_icon: egui::CursorIcon::Default,
            scale_factor: 1.0,
            clipboard_text: None,
        }
    }

    /// Returns a reference to the egui context.
    ///
    /// Use this between [`begin_frame`](Self::begin_frame) and
    /// [`end_frame_and_render`](Self::end_frame_and_render) to build your UI.
    pub fn context(&self) -> &egui::Context {
        &self.context
    }

    /// Returns a mutable reference to the underlying [`egui_wgpu::Renderer`].
    ///
    /// Useful for registering custom textures via
    /// [`Renderer::register_native_texture`](egui_wgpu::Renderer::register_native_texture).
    pub fn renderer_mut(&mut self) -> &mut egui_wgpu::Renderer {
        &mut self.renderer
    }

    /// Sets the clipboard text to be pasted on the next `Ctrl+V` / `Cmd+V`.
    ///
    /// Call this each frame or whenever the system clipboard changes. The text
    /// is consumed on the next paste event.
    pub fn set_clipboard_text(&mut self, text: String) {
        self.clipboard_text = Some(text);
    }

    /// Processes an astrelis [`WindowEvent`] and feeds it to egui.
    ///
    /// Returns `true` if egui consumed the event (i.e., it hit an egui widget
    /// and the application should not process it further).
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        astrelis_profiling::profile_function!();
        match event {
            WindowEvent::KeyboardInput(key_event) => self.on_keyboard_input(key_event),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.raw_input.modifiers = egui::Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control,
                    shift: modifiers.shift,
                    mac_cmd: cfg!(target_os = "macos") && modifiers.meta,
                    command: if cfg!(target_os = "macos") {
                        modifiers.meta
                    } else {
                        modifiers.control
                    },
                };
                false
            }
            WindowEvent::CursorMoved(pos) => {
                let pos_in_points = egui::pos2(
                    pos.x / self.pixels_per_point(),
                    pos.y / self.pixels_per_point(),
                );
                self.pointer_pos = Some(pos_in_points);
                self.raw_input
                    .events
                    .push(egui::Event::PointerMoved(pos_in_points));
                self.context.egui_is_using_pointer()
            }
            WindowEvent::CursorLeft => {
                self.pointer_pos = None;
                self.raw_input.events.push(egui::Event::PointerGone);
                false
            }
            WindowEvent::MouseButtonInput { button, state } => {
                if let Some(pos) = self.pointer_pos
                    && let Some(button) = convert::translate_mouse_button(*button)
                {
                    let pressed = *state == ElementState::Pressed;
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers: self.raw_input.modifiers,
                    });
                }
                self.context.egui_wants_pointer_input()
            }
            WindowEvent::MouseWheel(delta) => {
                self.on_mouse_wheel(*delta);
                self.context.egui_wants_pointer_input()
            }
            WindowEvent::Focused(focused) => {
                self.raw_input.focused = *focused;
                self.raw_input
                    .events
                    .push(egui::Event::WindowFocused(*focused));
                false
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = *scale_factor;
                self.raw_input
                    .viewports
                    .entry(egui::ViewportId::ROOT)
                    .or_default()
                    .native_pixels_per_point = Some(*scale_factor);
                false
            }
            WindowEvent::Ime(ime) => {
                match ime {
                    ImeEvent::Commit(text) => {
                        self.raw_input
                            .events
                            .push(egui::Event::Text(text.clone()));
                    }
                    ImeEvent::Preedit(text, _cursor) => {
                        self.raw_input.events.push(egui::Event::Ime(
                            egui::ImeEvent::Preedit(text.clone()),
                        ));
                    }
                    ImeEvent::Enabled => {
                        self.raw_input
                            .events
                            .push(egui::Event::Ime(egui::ImeEvent::Enabled));
                    }
                    ImeEvent::Disabled => {
                        self.raw_input
                            .events
                            .push(egui::Event::Ime(egui::ImeEvent::Disabled));
                    }
                }
                self.context.egui_wants_keyboard_input()
            }
            WindowEvent::Touch(touch) => {
                let pos = egui::pos2(
                    touch.position.x / self.pixels_per_point(),
                    touch.position.y / self.pixels_per_point(),
                );
                let phase = match touch.phase {
                    astrelis_window::event::TouchPhase::Started => egui::TouchPhase::Start,
                    astrelis_window::event::TouchPhase::Moved => egui::TouchPhase::Move,
                    astrelis_window::event::TouchPhase::Ended => egui::TouchPhase::End,
                    astrelis_window::event::TouchPhase::Cancelled => egui::TouchPhase::Cancel,
                };
                // All touch events are attributed to a single device. Multi-digitizer
                // disambiguation (e.g., Apple Pencil vs finger) is not yet supported.
                self.raw_input.events.push(egui::Event::Touch {
                    device_id: egui::TouchDeviceId(0),
                    id: egui::TouchId(touch.id.0),
                    phase,
                    pos,
                    force: None,
                });
                self.context.egui_is_using_pointer()
            }
            // Events handled elsewhere or not relevant to egui.
            WindowEvent::CursorEntered
            | WindowEvent::Resized(_)
            | WindowEvent::Moved(_)
            | WindowEvent::RedrawRequested
            | WindowEvent::CloseRequested
            | WindowEvent::Destroyed
            | WindowEvent::ThemeChanged(_)
            | WindowEvent::Occluded(_)
            | WindowEvent::DroppedFileHovered(_)
            | WindowEvent::DroppedFile(_)
            | WindowEvent::DroppedFileCancelled
            | WindowEvent::Minimized
            | WindowEvent::Restored
            | WindowEvent::Maximized
            | WindowEvent::Unmaximized => false,
            // WindowEvent is #[non_exhaustive]; default to not consumed for unknown variants.
            _ => false,
        }
    }

    /// Begins a new egui frame.
    ///
    /// Call this once per frame before building your UI via [`context()`](Self::context).
    pub fn begin_frame(&mut self, window: &dyn Window) {
        astrelis_profiling::profile_function!();
        let size = window.inner_size();
        let phys = size.physical();
        let ppp = self.pixels_per_point();

        let screen_size = egui::vec2(phys.width / ppp, phys.height / ppp);
        self.raw_input.screen_rect =
            (screen_size.x > 0.0 && screen_size.y > 0.0)
                .then(|| egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size));

        self.scale_factor = window.scale_factor();
        self.raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale_factor);

        self.context.begin_pass(self.raw_input.take());
    }

    /// Ends the egui frame, tessellates the output, and renders it.
    ///
    /// - `device`: The wgpu device for buffer/texture updates.
    /// - `encoder`: A command encoder to record the render pass into.
    /// - `render_target`: The texture view to render egui onto (typically the surface view).
    /// - `screen_descriptor`: Screen size and DPI information for egui-wgpu.
    /// - `window`: If provided, cursor icon changes are applied to this window.
    pub fn end_frame_and_render(
        &mut self,
        device: &WgpuDevice,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
        window: Option<&dyn Window>,
    ) {
        astrelis_profiling::profile_function!();
        let full_output = self.context.end_pass();

        // Apply cursor icon.
        let cursor_icon = full_output.platform_output.cursor_icon;
        if cursor_icon != self.current_cursor_icon {
            self.current_cursor_icon = cursor_icon;
            if let Some(window) = window {
                if cursor_icon == egui::CursorIcon::None {
                    window.set_cursor_visible(false);
                } else {
                    window.set_cursor_visible(true);
                    window.set_cursor_icon(convert::translate_cursor_icon(cursor_icon));
                }
            }
        }

        // Tessellate.
        let clipped_primitives = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let wgpu_device = device.wgpu_device();
        let wgpu_queue = device.wgpu_queue();

        // Update textures.
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(wgpu_device, wgpu_queue, *id, image_delta);
        }

        // Update buffers.
        self.renderer.update_buffers(
            wgpu_device,
            wgpu_queue,
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        // Render egui into a render pass.
        {
            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render_target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();

            self.renderer
                .render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        // Free textures.
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    /// Registers a wgpu texture with egui for use in image widgets.
    ///
    /// Returns a [`egui::TextureId`] that can be used with [`egui::Image`] or
    /// [`egui::Ui::image`].
    pub fn register_texture(
        &mut self,
        device: &WgpuDevice,
        texture_view: &wgpu::TextureView,
        filter: wgpu::FilterMode,
    ) -> egui::TextureId {
        self.renderer
            .register_native_texture(device.wgpu_device(), texture_view, filter)
    }

    /// Unregisters a previously registered texture.
    pub fn unregister_texture(&mut self, id: egui::TextureId) {
        self.renderer.free_texture(&id);
    }

    // --- Private helpers ---

    fn pixels_per_point(&self) -> f32 {
        self.scale_factor * self.context.zoom_factor()
    }

    fn on_keyboard_input(&mut self, event: &KeyEvent) -> bool {
        let pressed = event.state == ElementState::Pressed;

        let physical_key = convert::translate_key_code(event.key_code);
        let logical_key = convert::translate_key(&event.key);

        if let Some(active_key) = logical_key.or(physical_key) {
            // Handle clipboard shortcuts.
            if pressed {
                if is_cut_command(self.raw_input.modifiers, active_key) {
                    self.raw_input.events.push(egui::Event::Cut);
                    return self.context.egui_wants_keyboard_input();
                } else if is_copy_command(self.raw_input.modifiers, active_key) {
                    self.raw_input.events.push(egui::Event::Copy);
                    return self.context.egui_wants_keyboard_input();
                } else if is_paste_command(self.raw_input.modifiers, active_key) {
                    // Paste requires clipboard contents. If a provider is set,
                    // emit the event; otherwise fall through so the key event
                    // still reaches egui (and the application can supply
                    // clipboard text via `egui::RawInput::events`).
                    if let Some(text) = self.clipboard_text.take() {
                        self.raw_input.events.push(egui::Event::Paste(text));
                        return self.context.egui_wants_keyboard_input();
                    }
                }
            }

            self.raw_input.events.push(egui::Event::Key {
                key: active_key,
                physical_key,
                pressed,
                repeat: event.repeat,
                modifiers: self.raw_input.modifiers,
            });
        }

        // Emit text event for printable characters on press.
        if pressed
            && let Key::Character(text) = &event.key
        {
            let is_cmd = self.raw_input.modifiers.ctrl
                || self.raw_input.modifiers.command
                || self.raw_input.modifiers.mac_cmd;
            if !is_cmd && !text.is_empty() && text.chars().all(is_printable_char) {
                self.raw_input
                    .events
                    .push(egui::Event::Text(text.clone()));
            }
        }

        self.context.egui_wants_keyboard_input()
            || matches!(
                event.key,
                Key::Named(astrelis_window::keyboard::NamedKey::Tab)
            )
    }

    fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let ppp = self.pixels_per_point();
        let (unit, delta) = match delta {
            MouseScrollDelta::LineDelta { x, y } => {
                (egui::MouseWheelUnit::Line, egui::vec2(x, y))
            }
            MouseScrollDelta::PixelDelta { x, y } => {
                (egui::MouseWheelUnit::Point, egui::vec2(x / ppp, y / ppp))
            }
        };
        self.raw_input.events.push(egui::Event::MouseWheel {
            unit,
            delta,
            modifiers: self.raw_input.modifiers,
            phase: egui::TouchPhase::Move,
        });
    }
}

fn is_printable_char(chr: char) -> bool {
    let is_private_use = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);
    !is_private_use && !chr.is_ascii_control()
}

fn is_cut_command(modifiers: egui::Modifiers, key: egui::Key) -> bool {
    key == egui::Key::Cut
        || (modifiers.command && key == egui::Key::X)
        || (cfg!(target_os = "windows") && modifiers.shift && key == egui::Key::Delete)
}

fn is_copy_command(modifiers: egui::Modifiers, key: egui::Key) -> bool {
    key == egui::Key::Copy
        || (modifiers.command && key == egui::Key::C)
        || (cfg!(target_os = "windows") && modifiers.ctrl && key == egui::Key::Insert)
}

fn is_paste_command(modifiers: egui::Modifiers, key: egui::Key) -> bool {
    key == egui::Key::Paste
        || (modifiers.command && key == egui::Key::V)
        || (cfg!(target_os = "windows") && modifiers.shift && key == egui::Key::Insert)
}
