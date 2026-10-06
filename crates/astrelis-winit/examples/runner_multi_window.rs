//! Two desktop windows sharing one device, mesh, and renderer.
//! Left animates at 30 Hz using one-shot deadlines; right sleeps until invalidated.
//! Space toggles animation, M toggles MSAA, Escape closes a window.
//! Copy this file into a binary depending on astrelis-winit.
use astrelis_winit::{
    AppContext, CloseResponse, Handler, PrepareAction, Runner, SurfaceSettings, WindowInfo,
    astrelis::{Frame, Mesh, MeshRenderer, Vertex, wgpu},
    winit::{
        dpi::{LogicalPosition, LogicalSize},
        event::{ElementState, WindowEvent},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    },
};
use std::{
    collections::HashMap,
    error::Error,
    time::{Duration, Instant},
};
struct View {
    animated: bool,
    close_armed: bool,
}
struct Demo {
    views: HashMap<WindowId, View>,
    renderer: Option<MeshRenderer>,
    mesh: Option<Mesh>,
    started: Instant,
}
impl Handler for Demo {
    type Message = ();
    type Error = Box<dyn Error>;
    fn resumed(&mut self, cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        if self.views.is_empty() {
            for (i, animated) in [true, false].into_iter().enumerate() {
                let id = cx.create_window(
                    Window::default_attributes()
                        .with_title(if animated {
                            "30 Hz — Space pauses, M: MSAA"
                        } else {
                            "On demand — Space animates, M: MSAA"
                        })
                        .with_inner_size(LogicalSize::new(520., 400.))
                        .with_position(LogicalPosition::new(80. + i as f64 * 560., 140.)),
                    SurfaceSettings::new().depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8),
                )?;
                self.views.insert(
                    id,
                    View {
                        animated,
                        close_armed: false,
                    },
                );
            }
        }
        Ok(())
    }
    fn window_created(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        // Both default-created surfaces use the first window's graphics device.
        if self.renderer.is_none() {
            let graphics = cx.window(id).unwrap().graphics();
            self.renderer = Some(MeshRenderer::new(graphics));
            self.mesh = Some(graphics.create_mesh(
                &[
                    Vertex::new([0., 0.75, 0.], [1., 0.15, 0.15, 1.]),
                    Vertex::new([-0.75, -0.65, 0.], [0.15, 1., 0.15, 1.]),
                    Vertex::new([0.75, -0.65, 0.], [0.15, 0.35, 1., 1.]),
                ],
                &[0, 1, 2],
            )?);
        }
        Ok(())
    }
    fn window_event(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if let WindowEvent::KeyboardInput { event, .. } = event
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => cx.close_window(id)?,
                PhysicalKey::Code(KeyCode::Space) => {
                    if let Some(view) = self.views.get_mut(&id) {
                        view.animated = !view.animated;
                        if !view.animated {
                            cx.cancel_redraw_at(id)?;
                        }
                        cx.request_redraw(id)?;
                    }
                }
                PhysicalKey::Code(KeyCode::KeyM) => {
                    if let Some(window) = cx.window_mut(id)
                        && let Some(target) = window.target()
                    {
                        let count = if target.sample_count() == 1
                            && target.supported_sample_counts().contains(&4)
                        {
                            4
                        } else {
                            1
                        };
                        window.set_sample_count(count)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<PrepareAction, Self::Error> {
        self.renderer
            .as_mut()
            .unwrap()
            .prepare(cx.window(id).unwrap().render_format().unwrap())?;
        Ok(PrepareAction::Render)
    }
    fn render(
        &mut self,
        window: WindowInfo<'_>,
        frame: &mut Frame<'_, 'static>,
    ) -> Result<(), Self::Error> {
        let pulse = if self.views[&window.id()].animated {
            (self.started.elapsed().as_secs_f64() * 2.).sin() * 0.5 + 0.5
        } else {
            0.25
        };
        let mut pass = frame
            .render_pass()
            .clear_color(wgpu::Color {
                r: 0.015,
                g: 0.02 + 0.07 * pulse,
                b: 0.05 + 0.1 * pulse,
                a: 1.,
            })
            .begin()?;
        self.renderer
            .as_mut()
            .unwrap()
            .draw(&mut pass, self.mesh.as_ref().unwrap())?;
        Ok(())
    }
    fn submitted(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
        _submission: wgpu::SubmissionIndex,
    ) -> Result<(), Self::Error> {
        if self.views[&id].animated {
            cx.request_redraw_at(id, Instant::now() + Duration::from_secs_f64(1. / 30.))?;
        }
        Ok(())
    }
    fn close_requested(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<CloseResponse, Self::Error> {
        let view = self.views.get_mut(&id).unwrap();
        if view.close_armed {
            Ok(CloseResponse::Close)
        } else {
            view.close_armed = true;
            cx.window(id)
                .unwrap()
                .window()
                .set_title("Close again to confirm, or Escape");
            Ok(CloseResponse::KeepOpen)
        }
    }
    fn window_closed(
        &mut self,
        _cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        self.views.remove(&id);
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut app = Demo {
        views: HashMap::new(),
        renderer: None,
        mesh: None,
        started: Instant::now(),
    };
    Runner::new()?.run(&mut app)?;
    assert!(app.views.is_empty()); // Handler remains inspectable after run returns.
    Ok(())
}
