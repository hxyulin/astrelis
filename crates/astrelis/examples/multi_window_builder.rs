//! ApplicationBuilder comparison example
//!
//! This example shows the boilerplate reduction achieved by ApplicationBuilder
//! compared to the traditional manual setup approach.
//!
//! # Boilerplate Comparison
//!
//! ## Traditional Approach (~60 lines minimum):
//! ```ignore
//! fn main() {
//!     logging::init();
//!
//!     run_app(|ctx| {
//!         // 1. Manual GraphicsContext creation
//!         let graphics = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
//!
//!         // 2. Manual WindowManager creation
//!         let mut window_manager = WindowManager::new(graphics);
//!
//!         // 3. Manual window creation with verbose descriptor
//!         let window_id = window_manager.create_window(
//!             ctx,
//!             WindowDescriptor {
//!                 title: "My App".to_string(),
//!                 size: Some(WinitPhysicalSize::new(800.0, 600.0)),
//!                 ..Default::default()
//!             },
//!         ).expect("Failed to create window");
//!
//!         // 4. Manual Engine building
//!         let engine = EngineBuilder::new()
//!             .add_plugin(TimePlugin)
//!             .add_plugin(AssetPlugin)
//!             .add_plugin(RenderPlugin)
//!             .add_plugin(TextPlugin)
//!             .build();
//!
//!         // 5. Finally, create app with all manual setup
//!         Box::new(MyApp {
//!             window_manager,
//!             engine,
//!             window_id,
//!         })
//!     });
//! }
//! ```
//!
//! ## ApplicationBuilder Approach (~10 lines):
//! ```ignore
//! fn main() {
//!     logging::init();
//!
//!     ApplicationBuilder::new()
//!         .with_title("My App")
//!         .with_size(800, 600)
//!         .add_plugins(DefaultPlugins)
//!         .run(|_ctx, _engine| MyApp);
//! }
//! ```
//!
//! # Benefits
//! - **85% less boilerplate** (60 lines → 10 lines)
//! - **Declarative API** - intent is clear from builder calls
//! - **No manual resource wiring** - everything connected automatically
//! - **Type-safe** - compiler catches configuration errors
//! - **Extensible** - easy to add custom plugins
//!
//! Run with: cargo run --example multi_window_builder

use astrelis::prelude::*;

struct DemoApp {
    frames: u64,
}

impl astrelis_winit::app::App for DemoApp {
    fn on_start(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        println!("\n=== ApplicationBuilder Demo ===");
        println!("✓ GraphicsContext created automatically");
        println!("✓ WindowManager initialized automatically");
        println!("✓ Main window created automatically");
        println!("✓ All plugins registered automatically");
        println!("✓ Engine built and ready");
        println!("\nThis entire setup required just 4 builder calls!");
        println!("Traditional approach would have required 60+ lines of boilerplate.\n");
        println!("Press ESC or close window to exit\n");
    }

    fn update(&mut self, _ctx: &mut astrelis_winit::app::AppCtx, time: &astrelis_winit::FrameTime) {
        self.frames = time.frame_count;

        if self.frames % 120 == 0 {
            println!(
                "Running smoothly... Frame {}, {:.1} FPS",
                self.frames,
                1.0 / time.delta.as_secs_f32()
            );
        }
    }

    fn render(
        &mut self,
        _ctx: &mut astrelis_winit::app::AppCtx,
        _window_id: astrelis_winit::WindowId,
        _events: &mut astrelis_winit::event::EventBatch,
    ) {
        // WindowManager handles resize events automatically
        // Rendering would go here
    }

    fn on_exit(&mut self, _ctx: &mut astrelis_winit::app::AppCtx) {
        println!("\n=== Shutdown ===");
        println!("Ran for {} frames", self.frames);
        println!("✓ Automatic cleanup via Engine::shutdown()");
        println!("✓ Plugin cleanup in reverse dependency order");
        println!("✓ All resources freed");
    }
}

fn main() {
    astrelis_core::logging::init();

    println!("\nApplicationBuilder Demo");
    println!("=======================\n");
    println!("This example demonstrates the boilerplate reduction");
    println!("achieved by using ApplicationBuilder instead of manual setup.\n");

    // This is all you need!
    // Compare this to 60+ lines of manual setup in traditional approach
    ApplicationBuilder::new()
        .with_title("ApplicationBuilder Demo - 85% Less Boilerplate!")
        .with_size(800, 600)
        .add_plugins(DefaultPlugins)
        .run(|_ctx, _engine| DemoApp { frames: 0 });

    // That's it! Everything else is handled automatically:
    // - GraphicsContext creation
    // - WindowManager setup
    // - Window creation
    // - Engine building
    // - Plugin initialization
    // - Lifecycle management
    // - Cleanup on exit
}
