pub mod app;
pub mod event;
pub mod time;
pub mod window;

// Re-export WindowId for convenience
pub use winit::window::WindowId;

// Re-export FrameTime for convenience
pub use time::FrameTime;
