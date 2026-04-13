//! Window creation and management plugin.

use astrelis_window::control_flow::ControlFlow;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::WindowBuilder;

use crate::app::PrimaryWindowId;
use crate::plugin::Plugin;

/// Plugin that creates and manages the primary application window.
///
/// Configurable via its public fields. The window is created during
/// startup when the event loop becomes active.
pub struct WindowPlugin {
    /// Window title.
    pub title: String,
    /// Logical width in pixels.
    pub width: f32,
    /// Logical height in pixels.
    pub height: f32,
    /// Whether the window is resizable.
    pub resizable: bool,
}

impl Default for WindowPlugin {
    fn default() -> Self {
        Self {
            title: "Astrelis".to_string(),
            width: 1280.0_f32,
            height: 720.0_f32,
            resizable: true,
        }
    }
}

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut crate::app::App) {
        let title = self.title.clone();
        let width = self.width;
        let height = self.height;
        let resizable = self.resizable;

        app.add_startup(move |resources, ctx| {
            let attrs = WindowBuilder::new()
                .with_title(&title)
                .with_inner_size(LogicalInnerSize::new(width, height))
                .with_resizable(resizable)
                .build();

            let win_id = ctx.create_window(attrs).expect("failed to create window");
            ctx.set_control_flow(ControlFlow::Poll);

            tracing::info!(?win_id, "Primary window created");
            resources.insert(PrimaryWindowId(win_id));
        });
    }
}
