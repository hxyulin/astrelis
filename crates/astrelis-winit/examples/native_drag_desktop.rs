//! Two native windows exercising the optional AppKit/Win32 drag adapter.
//! No GPU needed. Titles/logs report native targets; this transfers no UI panels.
use astrelis_winit::{
    drag::{CapturedDrag, DragCancelReason, DragUpdate, desktop::DesktopDragHost},
    winit::{
        application::ApplicationHandler,
        dpi::{LogicalPosition, LogicalSize},
        event::{ElementState, MouseButton, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId},
    },
};
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};

struct Demo<'a> {
    host: Option<&'a mut DesktopDragHost>,
    drag: Option<CapturedDrag<'a, DesktopDragHost>>,
    windows: Vec<Arc<Window>>,
    smoke: Option<Instant>,
}
impl Demo<'_> {
    fn finish(&mut self, reason: Option<DragCancelReason>) {
        if let Some(mut drag) = self.drag.take() {
            if let Some(reason) = reason {
                eprintln!("{:?}", drag.cancel(reason));
            }
            self.host = Some(drag.into_host());
        }
    }
    fn poll(&mut self) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        for update in drag.poll_native() {
            eprintln!("native drag: {update:?}");
            for window in &self.windows {
                window.set_title(&format!(
                    "Native drag: {:?} → {:?}",
                    update.update, update.target
                ));
            }
            if !matches!(update.update, DragUpdate::Moved(_)) {
                break;
            }
        }
        if !drag.is_active() {
            self.finish(None);
        }
    }
}
impl ApplicationHandler for Demo<'_> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }
        for i in 0..2 {
            let window = Arc::new(
                el.create_window(
                    Window::default_attributes()
                        .with_title(
                            "Hold primary in content and drag to the other window; Escape cancels",
                        )
                        .with_inner_size(LogicalSize::new(420., 260.))
                        .with_position(LogicalPosition::new(120. + f64::from(i) * 450., 160.)),
                )
                .expect("create native window"),
            );
            self.host
                .as_mut()
                .expect("idle host")
                .register(&window)
                .expect("register native window");
            eprintln!(
                "registered {:?}: scale={}, inner={:?}",
                window.id(),
                window.scale_factor(),
                window.inner_size()
            );
            self.windows.push(window);
        }
        if self.smoke.is_some() {
            self.smoke = Some(Instant::now() + Duration::from_secs(2));
        }
    }
    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Native hooks have already sampled this event. Poll before accepting a
        // subsequent native press, so an old lease cannot consume the new one.
        self.poll();
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            }
        ) && self.drag.is_none()
        {
            let host = self.host.take().expect("idle host");
            // A failed acquisition ends this probe; no native capture is leaked.
            self.drag = Some(
                CapturedDrag::begin(host, id, MouseButton::Left)
                    .expect("verified native primary gesture"),
            );
            eprintln!("native capture started for {id:?}");
        }
        let cancellation = match event {
            WindowEvent::Focused(false) => Some(DragCancelReason::FocusLost),
            WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                Some(DragCancelReason::SourceClosed)
            }
            WindowEvent::KeyboardInput { event: ref key, .. }
                if key.state == ElementState::Pressed
                    && key.physical_key
                        == astrelis_winit::winit::keyboard::PhysicalKey::Code(
                            astrelis_winit::winit::keyboard::KeyCode::Escape,
                        ) =>
            {
                Some(DragCancelReason::Escape)
            }
            _ => None,
        };
        if self.drag.as_ref().is_some_and(|d| d.source() == id)
            && let Some(reason) = cancellation
        {
            self.finish(Some(reason));
        }
        if matches!(event, WindowEvent::CloseRequested) {
            self.finish(Some(DragCancelReason::Explicit));
            self.host
                .as_mut()
                .expect("idle host")
                .unregister(id)
                .expect("unregister");
            self.windows.retain(|w| w.id() != id);
            if self.windows.is_empty() {
                el.exit();
            }
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.poll();
        if self
            .smoke
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            eprintln!(
                "smoke: two windows registered; live target={:?}; gesture input NOT measured",
                self.host
                    .as_mut()
                    .expect("idle smoke host")
                    .target_at_pointer()
            );
            el.exit();
            return;
        }
        if self.drag.is_some() || self.smoke.is_some() {
            el.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(8),
            ));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.finish(Some(DragCancelReason::Suspended));
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.finish(Some(DragCancelReason::Explicit));
        for w in &self.windows {
            let _ = self.host.as_mut().expect("idle host").unregister(w.id());
        }
    }
}
use astrelis_winit::drag::NativeDragHost;
fn main() -> Result<(), Box<dyn Error>> {
    let el = EventLoop::new()?;
    let mut host = DesktopDragHost::new()?;
    let smoke = std::env::args().any(|a| a == "--smoke").then(Instant::now);
    el.run_app(&mut Demo {
        host: Some(&mut host),
        drag: None,
        windows: Vec::new(),
        smoke,
    })?;
    Ok(())
}
