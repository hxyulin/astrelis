//! Two windows with application-owned shared-device recovery.
//! Press D to destroy the device and exercise notification/rebuild; Escape closes.
//! Policy: one replacement per run, synchronous initialization, exit on any failure.
use astrelis_winit::{
    AppContext, Handler, PrepareAction, Runner, SurfaceSettings, WindowInfo,
    astrelis::{Frame, GraphicsContext, Mesh, MeshRenderer, Vertex, wgpu},
    winit::{
        event::{ElementState, WindowEvent},
        event_loop::EventLoopProxy,
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    },
};
use std::error::Error;

enum Message {
    Lost {
        generation: u64,
        reason: wgpu::DeviceLostReason,
        message: String,
    },
}

#[derive(Default)]
struct Demo {
    windows: Vec<WindowId>,
    graphics: Option<GraphicsContext>,
    renderer: Option<MeshRenderer>,
    mesh: Option<Mesh>,
    generation: u64,
    replacements: u32,
}
impl Demo {
    fn watch(graphics: &GraphicsContext, proxy: EventLoopProxy<Message>, generation: u64) {
        // Do not capture graphics/device: the device owns this closure.
        graphics.set_device_lost_callback(move |reason, message| {
            let _ = proxy.send_event(Message::Lost {
                generation,
                reason,
                message,
            });
        });
    }
    fn rebuild(&mut self, graphics: &GraphicsContext) -> Result<(), Box<dyn Error>> {
        // This model stays in application memory. Textures, UI, framebuffers,
        // custom shaders and bindings would be recreated here by their owners too.
        self.mesh = Some(graphics.create_mesh(
            &[
                Vertex::new([0., 0.7, 0.], [1., 0., 0., 1.]),
                Vertex::new([-0.7, -0.7, 0.], [0., 1., 0., 1.]),
                Vertex::new([0.7, -0.7, 0.], [0., 0., 1., 1.]),
            ],
            &[0, 1, 2],
        )?);
        self.renderer = Some(MeshRenderer::new(graphics));
        Ok(())
    }
}
impl Handler for Demo {
    type Message = Message;
    type Error = Box<dyn Error>;
    fn resumed(&mut self, cx: &mut AppContext<'_, Message>) -> Result<(), Self::Error> {
        if self.windows.is_empty() {
            for _ in 0..2 {
                self.windows.push(
                    cx.create_window(
                        Window::default_attributes()
                            .with_title("D: destroy shared device; Escape: close"),
                        SurfaceSettings::new(),
                    )?,
                );
            }
        }
        Ok(())
    }
    fn window_created(
        &mut self,
        cx: &mut AppContext<'_, Message>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        if self.graphics.is_none() {
            let graphics = cx.window(id).unwrap().graphics().clone();
            Self::watch(&graphics, cx.proxy(), self.generation);
            self.rebuild(&graphics)?;
            self.graphics = Some(graphics);
        }
        Ok(())
    }
    fn user_event(
        &mut self,
        cx: &mut AppContext<'_, Message>,
        event: Message,
    ) -> Result<(), Self::Error> {
        let Message::Lost {
            generation,
            reason,
            message,
        } = event;
        if generation != self.generation {
            return Ok(());
        } // stale completion
        let Some(old) = self.graphics.take() else {
            return Ok(());
        };
        cx.invalidate_graphics(&old);
        self.mesh = None;
        self.renderer = None;
        eprintln!("Device {generation} lost ({reason:?}): {message}");
        if self.replacements >= 1 || self.windows.is_empty() {
            cx.exit();
            return Ok(());
        }
        self.replacements += 1;
        // This example chooses a bounded synchronous recovery. An application may
        // instead return now and forward an async initialization result later.
        // Initialize against a surviving window to select a compatible adapter.
        let window = cx.window(self.windows[0]).unwrap();
        let (replacement, temporary_target) = pollster::block_on(GraphicsContext::with_surface(
            window.window().clone(),
            window
                .settings()
                .surface_options(window.metrics().physical_size()),
        ))?;
        drop(temporary_target); // release before staging managed swapchains
        self.generation += 1;
        Self::watch(&replacement, cx.proxy(), self.generation);
        self.rebuild(&replacement)?;
        // All shared windows switch together, including the runner's default for
        // later windows. No rendering occurs until this callback returns.
        cx.replace_graphics(&old, &replacement)?;
        self.graphics = Some(replacement);
        Ok(())
    }
    fn window_event(
        &mut self,
        cx: &mut AppContext<'_, Message>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if let WindowEvent::KeyboardInput { event, .. } = event
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyD) => {
                    if let Some(graphics) = &self.graphics {
                        graphics.device().destroy();
                        // On-demand windows may have no further GPU work to drain
                        // a deferred loss callback. Wake recovery without a wait.
                        let _ = graphics.device().poll(wgpu::PollType::Poll);
                    }
                }
                PhysicalKey::Code(KeyCode::Escape) => cx.close_window(id)?,
                _ => {}
            }
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        cx: &mut AppContext<'_, Message>,
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
            .clear_color(wgpu::Color::BLACK)
            .begin()?;
        self.renderer
            .as_mut()
            .unwrap()
            .draw(&mut pass, self.mesh.as_ref().unwrap())?;
        Ok(())
    }
    fn window_closed(
        &mut self,
        _cx: &mut AppContext<'_, Message>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        self.windows.retain(|window| *window != id);
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    Runner::new()?.run(&mut Demo::default())?;
    Ok(())
}
