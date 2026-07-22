//! Two independently hosted retained UI windows sharing one GPU device.

#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use std::io;

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::geometry::Size;
use astrelis_platform::{WindowAttributes, WindowEvent, WindowId};
use astrelis_text::FontDatabase;
use astrelis_ui_core::{Theme, Ui};
use astrelis_ui_host::{GraphicsContext, WindowHostOptions, WindowHosts};

struct MultiWindow {
    hosts: WindowHosts<()>,
}

impl MultiWindow {
    fn new() -> Self {
        Self {
            hosts: WindowHosts::new(GraphicsContext::new()),
        }
    }

    fn ui(title: &str, detail: &str) -> Ui<()> {
        let mut ui = Ui::new(FontDatabase::default(), Theme::dark());
        let root = ui.root();
        let column = ui.add_column(root).expect("add content column");
        ui.add_label(column, title).expect("add title");
        ui.add_label(column, detail).expect("add detail");
        ui
    }
}

impl App for MultiWindow {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if !self.hosts.is_empty() {
            return Ok(());
        }
        for (title, detail) in [
            ("Astrelis scene", "This window owns the primary view."),
            ("Astrelis tools", "This window owns an independent UI tree."),
        ] {
            let id = self
                .hosts
                .open(
                    context,
                    Self::ui(title, detail),
                    WindowHostOptions {
                        window: WindowAttributes {
                            title: title.into(),
                            inner_size: Some(Size::new(480.0, 260.0)),
                            ..WindowAttributes::default()
                        },
                        ..WindowHostOptions::default()
                    },
                )
                .map_err(io::Error::other)?;
            context.invalidate_window(id);
        }
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        let Some(update) = self
            .hosts
            .handle_event(id, &context.clipboard(), &event)
            .map_err(io::Error::other)?
        else {
            return Ok(());
        };
        if update.close_requested {
            self.hosts.close(context, id);
            if self.hosts.is_empty() {
                context.exit();
            }
        } else if update.redraw {
            context.invalidate_window(id);
        }
        Ok(())
    }

    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        id: WindowId,
    ) -> Result<(), Self::Error> {
        self.hosts.redraw(id).map_err(io::Error::other)?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), astrelis_app::RuntimeError<io::Error>> {
    Runtime::finish(astrelis_platform_winit::run_return(Runtime::new(
        MultiWindow::new(),
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
