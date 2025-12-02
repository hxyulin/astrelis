pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter("trace,wgpu-core=info,winit=info")
        .init();
}
