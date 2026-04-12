//! Backend traits and winit-based event loop implementation.

use std::collections::HashMap;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};

use crate::builder::WindowAttributes;
use crate::capabilities::build_capabilities;
use crate::capability::Capabilities;
use crate::control_flow::ControlFlow;
use crate::convert;
use crate::error::WindowError;
use crate::lifecycle::AppLifecycle;
use crate::monitor::MonitorInfo;
use crate::window::Window;
use crate::winit_window::WinitWindow;
use crate::window_id::WindowId;

/// Handler trait that users implement to receive events from the window backend.
///
/// Follows a callback pattern (similar to winit's `ApplicationHandler`) rather
/// than an iterator, because the backend controls the event loop lifetime on
/// most platforms.
pub trait AppHandler {
    /// Called when the application lifecycle state changes.
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle);

    /// Called for each window event.
    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: crate::event::WindowEvent,
    );

    /// Called for device-level events not tied to a specific window.
    ///
    /// The most important event here is [`DeviceEvent::MouseMotion`](crate::event::DeviceEvent::MouseMotion),
    /// which provides raw mouse deltas when the cursor is locked — essential for
    /// first-person camera controls.
    ///
    /// Default implementation does nothing.
    fn on_device_event(
        &mut self,
        _ctx: &mut dyn EventLoopContext,
        _event: crate::event::DeviceEvent,
    ) {
    }

    /// Called once per iteration after all pending events have been dispatched.
    ///
    /// This is the place to request redraws, update game state, set control
    /// flow, etc.
    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext);
}

/// Context provided to the [`AppHandler`] during event dispatch.
///
/// Allows the handler to create/destroy windows and control the event loop
/// without a direct reference to the backend.
pub trait EventLoopContext {
    /// Creates a new window with the given attributes.
    fn create_window(&mut self, attrs: WindowAttributes) -> Result<WindowId, WindowError>;

    /// Returns an immutable reference to a window by its ID.
    fn window(&self, id: WindowId) -> Option<&dyn Window>;

    /// Returns a mutable reference to a window by its ID.
    fn window_mut(&mut self, id: WindowId) -> Option<&mut dyn Window>;

    /// Destroys a window.
    fn destroy_window(&mut self, id: WindowId) -> Result<(), WindowError>;

    /// Sets the control flow mode for the event loop.
    fn set_control_flow(&mut self, flow: ControlFlow);

    /// Returns the current control flow mode.
    fn control_flow(&self) -> ControlFlow;

    /// Requests the event loop to exit after the current iteration completes.
    fn exit(&mut self);

    /// Returns the capabilities of this backend/platform.
    fn capabilities(&self) -> &Capabilities;

    /// Returns information about all connected monitors.
    fn monitors(&self) -> Vec<MonitorInfo>;

    /// Returns the primary monitor, if one can be determined.
    fn primary_monitor(&self) -> Option<MonitorInfo>;
}

/// Creates a winit event loop and runs the given handler.
///
/// This is the main entry point for the windowing system. It creates a winit
/// event loop and drives the given [`AppHandler`] with converted events.
///
/// On most platforms this does not return (it exits the process). On platforms
/// where it does return, it returns `Ok(())` on clean exit.
///
/// # Errors
///
/// Returns [`WindowError::BackendInitFailed`] if the event loop cannot be
/// created, or [`WindowError::EventLoopError`] if the event loop encounters
/// an error while running.
pub fn run(handler: &mut dyn AppHandler) -> Result<(), WindowError> {
    // NOTE: Do NOT use profile_function!() here — run_app() never returns,
    // so the scope guard would stay open for the entire process
    // lifetime and anchor every frame's root span under it.
    let event_loop = EventLoop::new()
        .map_err(|e| WindowError::BackendInitFailed(e.to_string()))?;
    let mut bridge = WinitBridge {
        handler,
        windows: HashMap::new(),
        winit_to_astrelis: HashMap::new(),
        astrelis_to_winit: HashMap::new(),
        next_id: 1,
        control_flow: ControlFlow::Poll,
        exit_requested: false,
        capabilities: build_capabilities(),
    };
    event_loop
        .run_app(&mut bridge)
        .map_err(|e| WindowError::EventLoopError(e.to_string()))
}

/// Bridge between winit's ApplicationHandler and our AppHandler.
struct WinitBridge<'a> {
    handler: &'a mut dyn AppHandler,
    windows: HashMap<winit::window::WindowId, WinitWindow>,
    winit_to_astrelis: HashMap<winit::window::WindowId, WindowId>,
    astrelis_to_winit: HashMap<WindowId, winit::window::WindowId>,
    next_id: u64,
    control_flow: ControlFlow,
    exit_requested: bool,
    capabilities: Capabilities,
}

impl WinitBridge<'_> {
    #[allow(clippy::too_many_arguments)]
    fn make_context<'a>(
        event_loop: &'a ActiveEventLoop,
        windows: &'a mut HashMap<winit::window::WindowId, WinitWindow>,
        winit_to_astrelis: &'a mut HashMap<winit::window::WindowId, WindowId>,
        astrelis_to_winit: &'a mut HashMap<WindowId, winit::window::WindowId>,
        next_id: &'a mut u64,
        control_flow: &'a mut ControlFlow,
        exit_requested: &'a mut bool,
        capabilities: &'a Capabilities,
    ) -> WinitEventLoopContext<'a> {
        WinitEventLoopContext {
            event_loop,
            windows,
            winit_to_astrelis,
            astrelis_to_winit,
            next_id,
            control_flow,
            exit_requested,
            capabilities,
        }
    }
}

impl ApplicationHandler for WinitBridge<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        astrelis_profiling::profile_function!();
        let mut ctx = Self::make_context(
            event_loop,
            &mut self.windows,
            &mut self.winit_to_astrelis,
            &mut self.astrelis_to_winit,
            &mut self.next_id,
            &mut self.control_flow,
            &mut self.exit_requested,
            &self.capabilities,
        );
        self.handler.on_lifecycle(&mut ctx, AppLifecycle::Resumed);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        astrelis_profiling::profile_function!();
        let mut ctx = Self::make_context(
            event_loop,
            &mut self.windows,
            &mut self.winit_to_astrelis,
            &mut self.astrelis_to_winit,
            &mut self.next_id,
            &mut self.control_flow,
            &mut self.exit_requested,
            &self.capabilities,
        );
        self.handler.on_lifecycle(&mut ctx, AppLifecycle::Suspended);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        astrelis_profiling::profile_function!();
        let Some(&astrelis_id) = self.winit_to_astrelis.get(&window_id) else {
            return;
        };

        let Some(converted) = ({
            astrelis_profiling::profile_scope!("convert_event");
            convert::event::convert_window_event(event)
        }) else {
            return;
        };

        let mut ctx = Self::make_context(
            event_loop,
            &mut self.windows,
            &mut self.winit_to_astrelis,
            &mut self.astrelis_to_winit,
            &mut self.next_id,
            &mut self.control_flow,
            &mut self.exit_requested,
            &self.capabilities,
        );
        self.handler
            .on_window_event(&mut ctx, astrelis_id, converted);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        astrelis_profiling::profile_function!();
        let converted = match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                crate::event::DeviceEvent::MouseMotion {
                    delta_x: delta.0,
                    delta_y: delta.1,
                }
            }
            winit::event::DeviceEvent::MouseWheel { delta } => {
                let (x, y) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x, y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        (pos.x as f32, pos.y as f32)
                    }
                };
                crate::event::DeviceEvent::MouseWheel {
                    delta_x: x,
                    delta_y: y,
                }
            }
            winit::event::DeviceEvent::Button { button, state } => {
                let state = match state {
                    winit::event::ElementState::Pressed => {
                        crate::event::ElementState::Pressed
                    }
                    winit::event::ElementState::Released => {
                        crate::event::ElementState::Released
                    }
                };
                crate::event::DeviceEvent::Button { button, state }
            }
            winit::event::DeviceEvent::Key(raw) => {
                let key_event = crate::event::KeyEvent {
                    key_code: convert::keyboard::convert_key_code(raw.physical_key),
                    key: crate::keyboard::Key::Unidentified,
                    state: match raw.state {
                        winit::event::ElementState::Pressed => {
                            crate::event::ElementState::Pressed
                        }
                        winit::event::ElementState::Released => {
                            crate::event::ElementState::Released
                        }
                    },
                    location: crate::keyboard::KeyLocation::Standard,
                    repeat: false,
                };
                crate::event::DeviceEvent::Key(key_event)
            }
            _ => return,
        };

        let mut ctx = Self::make_context(
            event_loop,
            &mut self.windows,
            &mut self.winit_to_astrelis,
            &mut self.astrelis_to_winit,
            &mut self.next_id,
            &mut self.control_flow,
            &mut self.exit_requested,
            &self.capabilities,
        );
        self.handler.on_device_event(&mut ctx, converted);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        astrelis_profiling::profile_function!();
        let mut ctx = Self::make_context(
            event_loop,
            &mut self.windows,
            &mut self.winit_to_astrelis,
            &mut self.astrelis_to_winit,
            &mut self.next_id,
            &mut self.control_flow,
            &mut self.exit_requested,
            &self.capabilities,
        );
        self.handler.on_events_cleared(&mut ctx);

        // Apply control flow.
        astrelis_profiling::profile_scope!("apply_control_flow");
        match self.control_flow {
            ControlFlow::Poll => {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
            }
            ControlFlow::Wait => {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
            }
            ControlFlow::WaitUntil(duration) => {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                    std::time::Instant::now() + duration,
                ));
            }
        }

        if self.exit_requested {
            event_loop.exit();
        }
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        astrelis_profiling::profile_function!();
        let mut ctx = Self::make_context(
            event_loop,
            &mut self.windows,
            &mut self.winit_to_astrelis,
            &mut self.astrelis_to_winit,
            &mut self.next_id,
            &mut self.control_flow,
            &mut self.exit_requested,
            &self.capabilities,
        );
        self.handler.on_lifecycle(&mut ctx, AppLifecycle::Exiting);
    }
}

/// EventLoopContext implementation backed by winit.
struct WinitEventLoopContext<'a> {
    event_loop: &'a ActiveEventLoop,
    windows: &'a mut HashMap<winit::window::WindowId, WinitWindow>,
    winit_to_astrelis: &'a mut HashMap<winit::window::WindowId, WindowId>,
    astrelis_to_winit: &'a mut HashMap<WindowId, winit::window::WindowId>,
    next_id: &'a mut u64,
    control_flow: &'a mut ControlFlow,
    exit_requested: &'a mut bool,
    capabilities: &'a Capabilities,
}

impl EventLoopContext for WinitEventLoopContext<'_> {
    fn create_window(&mut self, attrs: WindowAttributes) -> Result<WindowId, WindowError> {
        astrelis_profiling::profile_function!();
        let inner_size = attrs.inner_size.logical();
        let mut winit_attrs = winit::window::WindowAttributes::default()
            .with_title(&attrs.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                inner_size.width,
                inner_size.height,
            ))
            .with_resizable(attrs.resizable)
            .with_decorations(attrs.decorations)
            .with_visible(attrs.visible)
            .with_transparent(attrs.transparent)
            .with_maximized(attrs.maximized);

        if let Some(min) = attrs.min_inner_size {
            let s = min.logical();
            winit_attrs =
                winit_attrs.with_min_inner_size(winit::dpi::LogicalSize::new(s.width, s.height));
        }
        if let Some(max) = attrs.max_inner_size {
            let s = max.logical();
            winit_attrs =
                winit_attrs.with_max_inner_size(winit::dpi::LogicalSize::new(s.width, s.height));
        }
        if let Some(pos) = attrs.position {
            let p = pos.logical();
            winit_attrs = winit_attrs
                .with_position(winit::dpi::LogicalPosition::new(p.x, p.y));
        }

        let window = self
            .event_loop
            .create_window(winit_attrs)
            .map_err(|e| WindowError::WindowCreationFailed(e.to_string()))?;

        let astrelis_id = WindowId::new(*self.next_id);
        *self.next_id += 1;

        let winit_id = window.id();

        let winit_window = WinitWindow {
            inner: window,
            astrelis_id,
            title: attrs.title,
        };

        self.windows.insert(winit_id, winit_window);
        self.winit_to_astrelis.insert(winit_id, astrelis_id);
        self.astrelis_to_winit.insert(astrelis_id, winit_id);

        Ok(astrelis_id)
    }

    fn window(&self, id: WindowId) -> Option<&dyn Window> {
        let winit_id = self.astrelis_to_winit.get(&id)?;
        self.windows.get(winit_id).map(|w| w as &dyn Window)
    }

    fn window_mut(&mut self, id: WindowId) -> Option<&mut dyn Window> {
        let winit_id = self.astrelis_to_winit.get(&id)?;
        self.windows.get_mut(winit_id).map(|w| w as &mut dyn Window)
    }

    fn destroy_window(&mut self, id: WindowId) -> Result<(), WindowError> {
        let winit_id = self
            .astrelis_to_winit
            .remove(&id)
            .ok_or(WindowError::InvalidWindowId(id))?;
        self.winit_to_astrelis.remove(&winit_id);
        self.windows.remove(&winit_id);
        Ok(())
    }

    fn set_control_flow(&mut self, flow: ControlFlow) {
        *self.control_flow = flow;
    }

    fn control_flow(&self) -> ControlFlow {
        *self.control_flow
    }

    fn exit(&mut self) {
        *self.exit_requested = true;
    }

    fn capabilities(&self) -> &Capabilities {
        self.capabilities
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        self.event_loop
            .available_monitors()
            .enumerate()
            .map(|(i, h)| convert::monitor::convert_monitor(&h, i as u64))
            .collect()
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.event_loop
            .primary_monitor()
            .map(|h| convert::monitor::convert_monitor(&h, 0))
    }
}
