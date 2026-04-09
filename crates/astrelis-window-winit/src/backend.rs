//! WinitBackend — WindowBackend implementation using winit.

use std::collections::HashMap;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};

use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::builder::WindowAttributes;
use astrelis_window::capability::Capabilities;
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::error::WindowError;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::monitor::MonitorInfo;
use astrelis_window::window::Window;
use astrelis_window::window_id::WindowId;

use crate::capabilities::build_capabilities;
use crate::convert;
use crate::window::WinitWindow;

/// winit-based windowing backend.
pub struct WinitBackend {
    event_loop: EventLoop<()>,
}

impl WindowBackend for WinitBackend {
    fn new() -> Result<Self, WindowError> {
        let event_loop = EventLoop::new()
            .map_err(|e| WindowError::BackendInitFailed(e.to_string()))?;
        Ok(Self { event_loop })
    }

    fn run(self, handler: &mut dyn AppHandler) -> Result<(), WindowError> {
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
        self.event_loop
            .run_app(&mut bridge)
            .map_err(|e| WindowError::EventLoopError(e.to_string()))
    }
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
        let Some(&astrelis_id) = self.winit_to_astrelis.get(&window_id) else {
            return;
        };

        let Some(converted) = convert::event::convert_window_event(event) else {
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
        let converted = match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                astrelis_window::event::DeviceEvent::MouseMotion {
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
                astrelis_window::event::DeviceEvent::MouseWheel {
                    delta_x: x,
                    delta_y: y,
                }
            }
            winit::event::DeviceEvent::Button { button, state } => {
                let state = match state {
                    winit::event::ElementState::Pressed => {
                        astrelis_window::event::ElementState::Pressed
                    }
                    winit::event::ElementState::Released => {
                        astrelis_window::event::ElementState::Released
                    }
                };
                astrelis_window::event::DeviceEvent::Button { button, state }
            }
            winit::event::DeviceEvent::Key(raw) => {
                // RawKeyEvent only has physical_key + state (no logical key or location).
                let key_event = astrelis_window::event::KeyEvent {
                    key_code: convert::keyboard::convert_key_code(raw.physical_key),
                    key: astrelis_window::keyboard::Key::Unidentified,
                    state: match raw.state {
                        winit::event::ElementState::Pressed => {
                            astrelis_window::event::ElementState::Pressed
                        }
                        winit::event::ElementState::Released => {
                            astrelis_window::event::ElementState::Released
                        }
                    },
                    location: astrelis_window::keyboard::KeyLocation::Standard,
                    repeat: false,
                };
                astrelis_window::event::DeviceEvent::Key(key_event)
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

/// EventLoopContext implementation for the winit backend.
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
