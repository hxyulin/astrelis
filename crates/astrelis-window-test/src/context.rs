//! Mock event loop context.

use std::collections::HashMap;

use astrelis_core::geometry::{Physical, Point, Size};
use astrelis_window::backend::EventLoopContext;
use astrelis_window::builder::WindowAttributes;
use astrelis_window::capability::{Capabilities, Capability};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::error::WindowError;
use astrelis_window::monitor::{MonitorId, MonitorInfo, VideoMode};
use astrelis_window::window::Window;
use astrelis_window::window_id::WindowId;

use crate::window::MockWindow;

/// Mock implementation of [`EventLoopContext`] that tracks all state changes.
pub(crate) struct MockEventLoopContext {
    pub(crate) windows: HashMap<WindowId, MockWindow>,
    pub(crate) next_id: u64,
    pub(crate) control_flow: ControlFlow,
    pub(crate) exit_requested: bool,
    pub(crate) capabilities: Capabilities,
    pub(crate) created_ids: Vec<WindowId>,
    pub(crate) destroyed_ids: Vec<WindowId>,
}

impl MockEventLoopContext {
    pub(crate) fn new(
        initial_windows: HashMap<WindowId, MockWindow>,
        next_id: u64,
    ) -> Self {
        let mut capabilities = Capabilities::default();
        // Mock backend supports everything.
        for cap in [
            Capability::WindowOpacity,
            Capability::WindowLevel,
            Capability::Decorations,
            Capability::Minimize,
            Capability::Maximize,
            Capability::FullscreenBorderless,
            Capability::FullscreenExclusive,
            Capability::SizeConstraints,
            Capability::AspectRatio,
            Capability::CursorConfine,
            Capability::CursorLock,
            Capability::CustomCursor,
            Capability::DragWindow,
            Capability::DragResizeWindow,
            Capability::WindowIcon,
            Capability::ThemeDetection,
            Capability::TransparentBackground,
            Capability::TouchInput,
            Capability::ContentProtection,
            Capability::Ime,
        ] {
            capabilities.insert(cap);
        }

        Self {
            windows: initial_windows,
            next_id,
            control_flow: ControlFlow::Poll,
            exit_requested: false,
            capabilities,
            created_ids: Vec::new(),
            destroyed_ids: Vec::new(),
        }
    }
}

impl EventLoopContext for MockEventLoopContext {
    fn create_window(&mut self, attrs: WindowAttributes) -> Result<WindowId, WindowError> {
        let id = WindowId::new(self.next_id);
        self.next_id += 1;

        let logical = attrs.inner_size.logical();
        let window = MockWindow::new(
            id,
            attrs.title,
            Size::<Physical>::new(logical.width, logical.height),
        );
        self.windows.insert(id, window);
        self.created_ids.push(id);
        Ok(id)
    }

    fn window(&self, id: WindowId) -> Option<&dyn Window> {
        self.windows.get(&id).map(|w| w as &dyn Window)
    }

    fn window_mut(&mut self, id: WindowId) -> Option<&mut dyn Window> {
        self.windows.get_mut(&id).map(|w| w as &mut dyn Window)
    }

    fn destroy_window(&mut self, id: WindowId) -> Result<(), WindowError> {
        self.windows
            .remove(&id)
            .ok_or(WindowError::InvalidWindowId(id))?;
        self.destroyed_ids.push(id);
        Ok(())
    }

    fn set_control_flow(&mut self, flow: ControlFlow) {
        self.control_flow = flow;
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow
    }

    fn exit(&mut self) {
        self.exit_requested = true;
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            id: MonitorId::from_raw(0),
            name: Some("Mock Monitor".to_string()),
            position: Point::<Physical>::new(0.0, 0.0),
            size: Size::<Physical>::new(1920.0, 1080.0),
            scale_factor: 1.0,
            video_modes: vec![VideoMode {
                width: 1920,
                height: 1080,
                refresh_rate_millihertz: 60000,
                bit_depth: 8,
            }],
        }]
    }

    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.monitors().into_iter().next()
    }
}
