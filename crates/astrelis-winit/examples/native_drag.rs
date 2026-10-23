//! Two real windows measuring source event delivery and mixed-DPI mapping.
//! No native targeting host is installed: this intentionally cannot dock/drop.
//! Pass --smoke to require a submission from both windows and exit automatically.
//! This requires an interactive desktop; occluded surfaces fail the smoke assertion.
use astrelis_winit::{
    AppContext, Handler, Runner, SurfaceSettings, WindowInfo,
    astrelis::{Frame, wgpu},
    drag::{DragCancelReason, DragGeometry, DragTracker, DragUpdate, NativeDragCapabilities},
    winit::{
        dpi::{LogicalPosition, LogicalSize},
        event::{ElementState, MouseButton, WindowEvent},
        window::{Window, WindowId},
    },
};
use std::{collections::HashSet, error::Error};

#[derive(Default)]
struct Demo {
    windows: Vec<WindowId>,
    drag: Option<DragTracker>,
    smoke: bool,
    submitted: HashSet<WindowId>,
}

impl Demo {
    fn cancel(&mut self, reason: DragCancelReason) {
        if let Some(mut drag) = self.drag.take() {
            eprintln!("{:?}", drag.cancel(reason));
        }
    }
}

impl Handler for Demo {
    type Message = ();
    type Error = Box<dyn Error>;

    fn resumed(&mut self, cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        if self.windows.is_empty() {
            for i in 0..2 {
                self.windows.push(
                    cx.create_window(
                        Window::default_attributes()
                            .with_title(
                                "Hold left mouse; Escape cancels — native targeting unavailable",
                            )
                            .with_position(LogicalPosition::new(120. + f64::from(i) * 400., 160.))
                            .with_inner_size(LogicalSize::new(480., 320.)),
                        SurfaceSettings::new(),
                    )?,
                );
            }
            for id in &self.windows {
                let window = cx.window(*id).expect("registered window");
                eprintln!(
                    "window {id:?}: metrics {:?}, geometry {:?}",
                    window.metrics(),
                    DragGeometry::from_window(window.window())
                );
            }
            if self.smoke {
                let proxy = cx.proxy();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let _ = proxy.send_event(());
                });
            }
        }
        Ok(())
    }

    fn window_event(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if self.smoke {
            eprintln!(
                "native drag smoke event {id:?}: {event:?}; can_present={:?}",
                cx.window(id).map(|w| w.can_present())
            );
        }
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            }
        ) && self.drag.is_none()
        {
            self.drag = Some(DragTracker::new(id, MouseButton::Left));
            eprintln!("Tracking {id:?}; OS capture and authoritative routing unavailable");
        }
        let update = self.drag.as_mut().and_then(|drag| drag.observe(id, &event));
        match update {
            Some(DragUpdate::Moved(position)) => {
                let point = cx
                    .window(id)
                    .and_then(|source| DragGeometry::from_window(source.window()))
                    .and_then(|geometry| geometry.to_desktop(position));
                for &other in &self.windows {
                    if let Some(destination) = cx.window(other) {
                        let mapped = point.and_then(|point| {
                            DragGeometry::from_window(destination.window())?.to_logical(point)
                        });
                        let title = if let Some(local) = mapped {
                            format!(
                                "Mapped ({:.1}, {:.1}) — geometry only; target unknown",
                                local.x, local.y
                            )
                        } else {
                            "Global geometry unavailable — target unknown".into()
                        };
                        destination.window().set_title(&title);
                    }
                }
            }
            Some(terminal) => {
                eprintln!("{terminal:?}; no native destination established, no transfer");
                self.drag = None;
                for &window in &self.windows {
                    if let Some(window) = cx.window(window) {
                        window.window().set_title(
                            "Drag ended — no native target/transfer; hold left to retry",
                        );
                    }
                }
            }
            None => {}
        }
        Ok(())
    }

    fn suspended(&mut self, _cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        self.cancel(DragCancelReason::Suspended);
        Ok(())
    }

    fn submitted(
        &mut self,
        _cx: &mut AppContext<'_, ()>,
        id: WindowId,
        _submission: wgpu::SubmissionIndex,
    ) -> Result<(), Self::Error> {
        self.submitted.insert(id);
        Ok(())
    }

    fn user_event(&mut self, cx: &mut AppContext<'_, ()>, _: ()) -> Result<(), Self::Error> {
        if self.smoke {
            if self.submitted.len() != 2 {
                return Err("smoke probe did not submit frames for both windows".into());
            }
            eprintln!(
                "native drag smoke: two windows initialized and submitted; targeting/capture NOT validated"
            );
            cx.exit();
        }
        Ok(())
    }

    fn window_closed(
        &mut self,
        _cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        if self.drag.as_ref().is_some_and(|drag| drag.source() == id) {
            self.cancel(DragCancelReason::SourceClosed);
        }
        self.windows.retain(|window| *window != id);
        Ok(())
    }

    fn exiting(&mut self, _cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        self.cancel(DragCancelReason::Explicit);
        Ok(())
    }

    fn render(
        &mut self,
        window: WindowInfo<'_>,
        frame: &mut Frame<'_, 'static>,
    ) -> Result<(), Self::Error> {
        let first = self.windows.first() == Some(&window.id());
        let color = if first {
            wgpu::Color {
                r: 0.035,
                g: 0.10,
                b: 0.18,
                a: 1.,
            }
        } else {
            wgpu::Color {
                r: 0.15,
                g: 0.055,
                b: 0.08,
                a: 1.,
            }
        };
        let _pass = frame.render_pass().clear_color(color).begin()?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    eprintln!(
        "winit-only capabilities: {:?}",
        NativeDragCapabilities::WINIT_ONLY
    );
    eprintln!("Move the windows to different-DPI monitors; overlap them and a foreign app.");
    eprintln!(
        "Titles show coordinate mapping even when occluded; they NEVER establish a native target."
    );
    Runner::new()?.run(&mut Demo {
        smoke: std::env::args().any(|arg| arg == "--smoke"),
        ..Default::default()
    })?;
    Ok(())
}
