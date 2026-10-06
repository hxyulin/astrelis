//! Optional window/lifecycle integration for Astrelis applications using winit.
//!
//! [`WindowContext`] integrates surface ownership with an application-owned event
//! loop. On desktop, `Runner` supplies optional lifecycle and redraw scheduling
//! through `Handler`. Applications retain their models and rendering resources.
//!
//! The lifecycle separates input, preparation, recording, and presentation.
//! Preparation runs before frame
//! acquisition; rendering receives immutable window information and a borrowed
//! Astrelis frame. Pass construction and custom GPU work remain application choices.
//! A low-level window context also supports applications with their own event
//! loop. Native winit events, identifiers, attributes, windows, and message proxies
//! remain accessible to platform integrations such as IME and accessibility.
//!
//! Window geometry uses physical pixels for surface attachments and logical units
//! for layout. A metric snapshot derives both from one physical size/scale factor,
//! so consumers need not query the operating system in each drawing operation.
//!
//! ```
//! use astrelis_winit::{SurfaceSettings, WindowMetrics, winit::dpi::PhysicalSize};
//! let metrics = WindowMetrics::new(PhysicalSize::new(1600, 1200), 2.)?;
//! assert_eq!(metrics.logical_size().width, 800.);
//! let options = SurfaceSettings::new().sample_count(4)
//!     .surface_options(metrics.physical_size());
//! assert_eq!(options.size, [1600, 1200]);
//! assert_eq!(options.sample_count, 4);
//! # Ok::<(), astrelis_winit::InvalidWindowMetrics>(())
//! ```
//!
//! Native convenience driver (macOS, Windows, Linux):
//!
//! ```no_run
//! # #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use astrelis_winit::{AppContext, Handler, Runner, SurfaceSettings, WindowInfo,
//!     astrelis::{Frame, wgpu}, winit::window::{Window, WindowId}};
//! #[derive(Default)]
//! struct App { window: Option<WindowId> }
//! impl Handler for App {
//!     type Message = ();
//!     type Error = Box<dyn std::error::Error>;
//!     fn resumed(&mut self, cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
//!         if self.window.is_none() {
//!             self.window = Some(cx.create_window(
//!                 Window::default_attributes().with_title("Astrelis"),
//!                 SurfaceSettings::new())?);
//!         }
//!         Ok(())
//!     }
//!     fn render(&mut self, _window: WindowInfo<'_>, frame: &mut Frame<'_, 'static>)
//!         -> Result<(), Self::Error> {
//!         let _pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
//!         Ok(())
//!     }
//! }
//! Runner::new()?.run(&mut App::default())?;
//! # Ok(())
//! # }
//! # #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
//! # fn main() {}
//! ```
//!
//! A created window stays hidden until `Handler::window_created` returns, so
//! native accessibility adapters can be installed before showing it. The first
//! managed window may block on one-time GPU initialization. For application-owned
//! async initialization, use [`WindowContext::new`] and desktop adoption.
//! [`WindowContext::set_visible`] keeps native visibility and redraw policy together.
//! Accessing the native window still allows title, cursor, IME, and platform APIs.
//!
//! OnDemand is the default; Continuous and one-shot redraw deadlines are per window.
//! Retryable acquisition is paced; known occlusion, zero size, and suspension wait
//! for availability. Renderers and settings belong to the application and targets,
//! respectively. The runner introduces no default pass, clear color, or explicit GPU completion wait.
//! Device loss and asynchronous initialization cancellation remain application policy.

mod error;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod handler;
mod policy;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod runner;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod scheduler;
#[cfg(all(
    test,
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
mod scheduler_tests;
mod window;

pub use astrelis;
pub use error::{Callback, RunError, UnknownWindow, WindowError};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub use handler::{Handler, PrepareAction};
pub use policy::{CloseResponse, InvalidWindowMetrics, RedrawMode, SurfaceSettings, WindowMetrics};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub use runner::{AppContext, Runner, RunnerOptions};
pub use window::{WindowContext, WindowInfo, WindowUpdate};
pub use winit;
