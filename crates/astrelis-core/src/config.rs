pub use winit::event_loop::ControlFlow;

/// Configurations for the Astrelis Game Engine
#[derive(Debug)]
pub struct Config {
    pub benchmark: BenchmarkMode,
    pub control_flow: ControlFlow,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            benchmark: BenchmarkMode::Off,
            control_flow: ControlFlow::Poll,
        }
    }
}

#[derive(Debug)]
pub enum BenchmarkMode {
    /// Benchmarking is disabled
    Off,
    /// Benchmarking is enabled, and can be viewed using the built-in viewer
    On,
    /// Benchmarking is enabled, and can be viewed either using the built-in viewer or
    /// using external tools such as 'puffin_viewer'
    WithWebsever,
}
