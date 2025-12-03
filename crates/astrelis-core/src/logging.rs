pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter("trace,wgpu_core=info,winit=info,cosmic_text=info,naga=info,wgpu_hal=info")
        .init();
}
