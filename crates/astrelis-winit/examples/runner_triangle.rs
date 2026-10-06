//! Copy this file into a desktop binary depending on astrelis-winit.
//! M toggles supported MSAA, Escape closes. No pass is supplied by the runner.
use astrelis_winit::{
    AppContext, Handler, PrepareAction, Runner, SurfaceSettings, WindowInfo,
    astrelis::{Frame, Mesh, MeshRenderer, Vertex, wgpu},
    winit::{
        dpi::LogicalSize,
        event::{ElementState, WindowEvent},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    },
};
use std::error::Error;

#[derive(Default)]
struct Triangle {
    window: Option<WindowId>,
    renderer: Option<MeshRenderer>,
    mesh: Option<Mesh>,
}
impl Handler for Triangle {
    type Message = ();
    type Error = Box<dyn Error>;
    fn resumed(&mut self, cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        if self.window.is_none() {
            self.window = Some(
                cx.create_window(
                    Window::default_attributes()
                        .with_title("Astrelis runner — M: MSAA, Escape: close")
                        .with_inner_size(LogicalSize::new(800., 600.)),
                    SurfaceSettings::new(),
                )?,
            );
        }
        Ok(())
    }
    fn window_created(
        &mut self,
        cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        let window = cx.window(id).unwrap();
        self.renderer = Some(MeshRenderer::new(window.graphics()));
        self.mesh = Some(window.graphics().create_mesh(
            &[
                Vertex::new([0., 0.75, 0.], [1., 0.15, 0.15, 1.]),
                Vertex::new([-0.75, -0.65, 0.], [0.15, 1., 0.15, 1.]),
                Vertex::new([0.75, -0.65, 0.], [0.15, 0.35, 1., 1.]),
            ],
            &[0, 1, 2],
        )?);
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
        _window: WindowInfo<'_>,
        frame: &mut Frame<'_, 'static>,
    ) -> Result<(), Self::Error> {
        let mut pass = frame
            .render_pass()
            .clear_color(wgpu::Color {
                r: 0.015,
                g: 0.02,
                b: 0.035,
                a: 1.,
            })
            .begin()?;
        self.renderer
            .as_mut()
            .unwrap()
            .draw(&mut pass, self.mesh.as_ref().unwrap())?;
        Ok(())
    }
    fn window_closed(
        &mut self,
        _cx: &mut AppContext<'_, ()>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        if self.window == Some(id) {
            self.window = None;
            self.mesh = None;
            self.renderer = None;
        }
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    Runner::new()?.run(&mut Triangle::default())?;
    Ok(())
}
