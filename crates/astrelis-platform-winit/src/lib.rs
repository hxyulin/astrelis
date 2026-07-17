//! Winit desktop backend for the Astrelis platform API.

#![warn(missing_docs)]

mod convert;
mod window;

use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

use astrelis_platform::{
    Application, Clipboard, ControlFlow, DeviceId, EventLoopClosed, EventLoopProxy,
    PlatformContext, PlatformError, WindowId, backend,
};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, EventLoop},
};

use crate::window::WinitWindow;

/// Runs an application on winit's portable owned event loop.
#[cfg(not(target_arch = "wasm32"))]
pub fn run<A: Application>(app: A) -> Result<(), PlatformError> {
    run_return(app).map(|_| ())
}

/// Runs an application and returns it after the event loop terminates.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_return<A: Application>(app: A) -> Result<A, PlatformError> {
    let event_loop = EventLoop::<A::UserEvent>::with_user_event()
        .build()
        .map_err(|error| PlatformError::new(error.to_string()))?;
    let proxy = event_loop.create_proxy();
    let mut adapter = Adapter {
        app,
        proxy,
        clipboard: Clipboard::from_backend(Arc::new(WinitClipboard::default())),
        windows: HashMap::new(),
        next_window_id: AtomicU64::new(1),
    };
    event_loop
        .run_app(&mut adapter)
        .map_err(|error| PlatformError::new(error.to_string()))?;
    Ok(adapter.app)
}

/// Browser-specific Winit integration.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use super::*;
    use winit::platform::web::EventLoopExtWebSys;

    /// Starts an application using an existing page canvas.
    ///
    /// This returns after scheduling Winit's browser event loop. The initial
    /// vertical slice supports one Astrelis window for the supplied canvas.
    pub fn spawn_on_canvas<A: Application>(
        app: A,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), PlatformError> {
        let event_loop = EventLoop::<A::UserEvent>::with_user_event()
            .build()
            .map_err(|error| PlatformError::new(error.to_string()))?;
        let proxy = event_loop.create_proxy();
        let adapter = Adapter {
            app,
            proxy,
            clipboard: Clipboard::from_backend(Arc::new(UnsupportedWebClipboard)),
            canvas: Some(canvas),
            windows: HashMap::new(),
            next_window_id: AtomicU64::new(1),
        };
        event_loop.spawn_app(adapter);
        Ok(())
    }
}

struct Adapter<A: Application> {
    app: A,
    proxy: winit::event_loop::EventLoopProxy<A::UserEvent>,
    clipboard: Clipboard,
    #[cfg(target_arch = "wasm32")]
    canvas: Option<web_sys::HtmlCanvasElement>,
    windows: HashMap<winit::window::WindowId, (WindowId, Weak<WinitWindow>)>,
    next_window_id: AtomicU64,
}

impl<A: Application> Adapter<A> {
    fn with_context(
        &mut self,
        event_loop: &ActiveEventLoop,
        callback: impl FnOnce(&mut A, &mut PlatformContext<'_, A::UserEvent>),
    ) {
        let app = &mut self.app;
        let mut context = Context {
            event_loop,
            proxy: self.proxy.clone(),
            clipboard: self.clipboard.clone(),
            #[cfg(target_arch = "wasm32")]
            canvas: &mut self.canvas,
            windows: &mut self.windows,
            next_window_id: &self.next_window_id,
        };
        callback(app, &mut PlatformContext::from_backend(&mut context));
    }
}

impl<A: Application> ApplicationHandler<A::UserEvent> for Adapter<A> {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        astrelis_profiling::profile_scope!("platform.new_events");
        self.with_context(event_loop, |app, context| {
            app.new_events(context, convert::start_cause(cause))
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.with_context(event_loop, |app, context| app.resumed(context));
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.with_context(event_loop, |app, context| app.suspended(context));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        native_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        astrelis_profiling::profile_scope!("platform.window_event");
        let Some((id, weak)) = self.windows.get(&native_id).cloned() else {
            return;
        };
        let destroyed = matches!(event, winit::event::WindowEvent::Destroyed);
        if !destroyed && weak.upgrade().is_none() {
            self.windows.remove(&native_id);
            return;
        }
        let inner_size = weak.upgrade().map(|window| window.native.inner_size());
        if let Some(event) = convert::window_event(event, inner_size) {
            self.with_context(event_loop, |app, context| {
                app.window_event(context, id, event)
            });
        }
        if destroyed || weak.upgrade().is_none() {
            self.windows.remove(&native_id);
        }
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        astrelis_profiling::profile_scope!("platform.device_event");
        if let Some(event) = convert::device_event(event) {
            let id = DeviceId(convert::hash_id(device_id));
            self.with_context(event_loop, |app, context| {
                app.device_event(context, id, event)
            });
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: A::UserEvent) {
        self.with_context(event_loop, |app, context| app.user_event(context, event));
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        astrelis_profiling::profile_scope!("platform.about_to_wait");
        self.windows.retain(|_, (_, weak)| weak.strong_count() > 0);
        self.with_context(event_loop, |app, context| app.about_to_wait(context));
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.with_context(event_loop, |app, context| app.exiting(context));
    }
}

struct Context<'a, T: 'static> {
    event_loop: &'a ActiveEventLoop,
    proxy: winit::event_loop::EventLoopProxy<T>,
    clipboard: Clipboard,
    #[cfg(target_arch = "wasm32")]
    canvas: &'a mut Option<web_sys::HtmlCanvasElement>,
    windows: &'a mut HashMap<winit::window::WindowId, (WindowId, Weak<WinitWindow>)>,
    next_window_id: &'a AtomicU64,
}

impl<T: Send + 'static> backend::ActiveContext<T> for Context<'_, T> {
    fn create_window(
        &mut self,
        attributes: astrelis_platform::WindowAttributes,
    ) -> Result<astrelis_platform::Window, PlatformError> {
        astrelis_profiling::profile_scope!("platform.create_window");
        let attributes = window::attributes(attributes);
        #[cfg(target_arch = "wasm32")]
        let attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;

            let canvas = self.canvas.take().ok_or_else(|| {
                PlatformError::new("the browser runner supports one canvas window")
            })?;
            attributes
                .with_canvas(Some(canvas))
                .with_focusable(true)
                .with_prevent_default(true)
        };
        let native = self
            .event_loop
            .create_window(attributes)
            .map_err(|error| PlatformError::new(error.to_string()))?;
        let id = WindowId(self.next_window_id.fetch_add(1, Ordering::Relaxed));
        let backend = Arc::new(WinitWindow {
            id,
            native: Arc::new(native),
        });
        self.windows
            .insert(backend.native.id(), (id, Arc::downgrade(&backend)));
        Ok(astrelis_platform::Window::from_backend(backend))
    }

    fn set_control_flow(&mut self, flow: ControlFlow) {
        astrelis_profiling::profile_scope!("platform.wait_transition");
        self.event_loop.set_control_flow(match flow {
            ControlFlow::Poll => winit::event_loop::ControlFlow::Poll,
            ControlFlow::Wait => winit::event_loop::ControlFlow::Wait,
            ControlFlow::WaitUntil(deadline) => winit::event_loop::ControlFlow::WaitUntil(deadline),
        });
    }

    fn control_flow(&self) -> ControlFlow {
        match self.event_loop.control_flow() {
            winit::event_loop::ControlFlow::Poll => ControlFlow::Poll,
            winit::event_loop::ControlFlow::Wait => ControlFlow::Wait,
            winit::event_loop::ControlFlow::WaitUntil(deadline) => ControlFlow::WaitUntil(deadline),
        }
    }

    fn available_monitors(&self) -> Vec<astrelis_platform::Monitor> {
        self.event_loop
            .available_monitors()
            .map(convert::monitor)
            .collect()
    }
    fn primary_monitor(&self) -> Option<astrelis_platform::Monitor> {
        self.event_loop.primary_monitor().map(convert::monitor)
    }
    fn event_loop_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::from_backend(Arc::new(WinitProxy(self.proxy.clone())))
    }
    fn clipboard(&self) -> astrelis_platform::Clipboard {
        self.clipboard.clone()
    }
    fn exit(&mut self) {
        self.event_loop.exit();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct WinitClipboard {
    inner: Mutex<Option<arboard::Clipboard>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for WinitClipboard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WinitClipboard")
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WinitClipboard {
    fn with_clipboard<T>(
        &self,
        operation: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
    ) -> Result<T, PlatformError> {
        let mut clipboard = self
            .inner
            .lock()
            .map_err(|_| PlatformError::new("clipboard lock was poisoned"))?;
        if clipboard.is_none() {
            *clipboard = Some(
                arboard::Clipboard::new().map_err(|error| PlatformError::new(error.to_string()))?,
            );
        }
        operation(clipboard.as_mut().expect("clipboard initialized"))
            .map_err(|error| PlatformError::new(error.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl backend::Clipboard for WinitClipboard {
    fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
        astrelis_platform::ClipboardCapabilities {
            read_text: true,
            write_text: true,
        }
    }

    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        self.with_clipboard(|clipboard| match clipboard.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(error) => Err(error),
        })
    }

    fn write_text(&self, text: String) -> Result<(), PlatformError> {
        self.with_clipboard(|clipboard| clipboard.set_text(text))
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct UnsupportedWebClipboard;

#[cfg(target_arch = "wasm32")]
impl backend::Clipboard for UnsupportedWebClipboard {
    fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
        astrelis_platform::ClipboardCapabilities::default()
    }

    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        Err(PlatformError::new(
            "synchronous clipboard reads are unavailable in browsers",
        ))
    }

    fn write_text(&self, _text: String) -> Result<(), PlatformError> {
        Err(PlatformError::new(
            "synchronous clipboard writes are unavailable in browsers",
        ))
    }
}

struct WinitProxy<T: 'static>(winit::event_loop::EventLoopProxy<T>);
impl<T> std::fmt::Debug for WinitProxy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WinitProxy")
    }
}
impl<T: Send + 'static> backend::EventLoopProxy<T> for WinitProxy<T> {
    fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.0
            .send_event(event)
            .map_err(|error| EventLoopClosed(error.0))
    }
}
