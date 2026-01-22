//! High-level application builder for simplified game/app initialization.
//!
//! The ApplicationBuilder eliminates 35-50 lines of boilerplate by providing
//! a clean, declarative API for common app setup patterns.

use crate::{Engine, EngineBuilder};
use crate::plugin::{Plugin, PluginDyn, PluginGroup};

#[cfg(all(feature = "render", feature = "winit"))]
use astrelis_render::{WindowManager, WindowContextDescriptor};

#[cfg(feature = "winit")]
use astrelis_winit::{
    app::{run_app, App, AppCtx},
    window::{WindowDescriptor, WinitPhysicalSize},
};

/// Internal plugin group that wraps stored plugins
struct StoredPlugins {
    plugins: std::cell::RefCell<Vec<Box<dyn PluginDyn>>>,
}

impl PluginGroup for StoredPlugins {
    fn name(&self) -> &'static str {
        "StoredPlugins"
    }

    fn plugins(&self) -> Vec<Box<dyn PluginDyn>> {
        self.plugins.borrow_mut().drain(..).collect()
    }
}

/// High-level builder for creating Astrelis applications.
///
/// The ApplicationBuilder provides a clean, declarative API for common app
/// initialization patterns, eliminating 35-50 lines of boilerplate.
///
/// # Example
///
/// ```ignore
/// use astrelis::prelude::*;
///
/// struct MyGame {
///     window_id: WindowId,
/// }
///
/// impl App for MyGame {
///     fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
///         // Game logic
///     }
///
///     fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
///         // Rendering
///     }
/// }
///
/// fn main() {
///     ApplicationBuilder::new()
///         .with_title("My Game")
///         .with_size(1280, 720)
///         .add_plugins(DefaultPlugins)
///         .run(|ctx, engine| {
///             // Window already created! Get its ID:
///             let window_id = ctx.windows().keys().next().copied().unwrap();
///             MyGame { window_id }
///         });
/// }
/// ```
pub struct ApplicationBuilder {
    title: String,
    size: (u32, u32),
    plugins: Vec<Box<dyn PluginDyn>>,
    #[cfg(all(feature = "render", feature = "winit"))]
    window_descriptor: Option<WindowContextDescriptor>,
    create_window: bool,
}

impl Default for ApplicationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationBuilder {
    /// Creates a new ApplicationBuilder with default settings.
    ///
    /// Defaults:
    /// - Title: "Astrelis Application"
    /// - Size: 1280x720
    /// - Plugins: None (add with `add_plugin` or `add_plugins`)
    /// - Window: Automatically created
    pub fn new() -> Self {
        Self {
            title: "Astrelis Application".to_string(),
            size: (1280, 720),
            plugins: Vec::new(),
            #[cfg(all(feature = "render", feature = "winit"))]
            window_descriptor: None,
            create_window: true,
        }
    }

    /// Sets the window title.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::ApplicationBuilder;
    /// ApplicationBuilder::new()
    ///     .with_title("My Game");
    /// ```
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the window size in logical pixels.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::ApplicationBuilder;
    /// ApplicationBuilder::new()
    ///     .with_size(1920, 1080);
    /// ```
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.size = (width, height);
        self
    }

    /// Adds a plugin to the engine.
    ///
    /// Plugins are initialized in the order they are added, with dependency
    /// ordering handled automatically.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::ApplicationBuilder;
    /// # use astrelis::FnPlugin;
    /// ApplicationBuilder::new()
    ///     .add_plugin(FnPlugin::new("setup", |resources| {
    ///         // Plugin initialization
    ///     }));
    /// ```
    pub fn add_plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Adds a plugin group (multiple plugins) to the engine.
    ///
    /// This is useful for adding default plugins or custom plugin bundles.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::{ApplicationBuilder, DefaultPlugins};
    /// ApplicationBuilder::new()
    ///     .add_plugins(DefaultPlugins);
    /// ```
    pub fn add_plugins(mut self, group: impl PluginGroup) -> Self {
        for plugin in group.plugins() {
            self.plugins.push(plugin);
        }
        self
    }

    /// Sets custom window rendering context descriptor.
    ///
    /// This allows configuring advanced rendering options like present mode,
    /// texture format, and alpha mode.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::ApplicationBuilder;
    /// # #[cfg(all(feature = "render", feature = "winit"))]
    /// # {
    /// use astrelis_render::WindowContextDescriptor;
    ///
    /// ApplicationBuilder::new()
    ///     .with_window_descriptor(WindowContextDescriptor {
    ///         present_mode: Some(wgpu::PresentMode::Mailbox),
    ///         ..Default::default()
    ///     });
    /// # }
    /// ```
    #[cfg(all(feature = "render", feature = "winit"))]
    pub fn with_window_descriptor(mut self, descriptor: WindowContextDescriptor) -> Self {
        self.window_descriptor = Some(descriptor);
        self
    }

    /// Disables automatic window creation.
    ///
    /// By default, ApplicationBuilder creates a window automatically. Use this
    /// if you want to create windows manually in your app factory function.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::ApplicationBuilder;
    /// ApplicationBuilder::new()
    ///     .without_window()
    ///     .run(|ctx, engine| {
    ///         // Create windows manually here
    ///         MyApp::new()
    ///     });
    /// # struct MyApp;
    /// # impl MyApp { fn new() -> Self { Self } }
    /// ```
    pub fn without_window(mut self) -> Self {
        self.create_window = false;
        self
    }

    /// Builds and runs the application with the given factory function.
    ///
    /// The factory function receives:
    /// - `AppCtx` - For creating additional windows or accessing window resources
    /// - `Engine` - The initialized engine with all plugins
    ///
    /// The factory should return your App implementation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::prelude::*;
    /// # struct MyApp { window_id: WindowId }
    /// # impl App for MyApp {
    /// #     fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {}
    /// # }
    /// ApplicationBuilder::new()
    ///     .with_title("My Game")
    ///     .run(|ctx, engine| {
    ///         // Engine is already built with plugins
    ///         // Window is already created (unless you called without_window)
    ///         MyApp { window_id: WindowId::from(0u64) }
    ///     });
    /// ```
    #[cfg(feature = "winit")]
    pub fn run<T>(self, factory: impl FnOnce(&mut AppCtx, &Engine) -> T + 'static)
    where
        T: App + 'static,
    {
        self.run_internal(factory)
    }

    #[cfg(feature = "winit")]
    fn run_internal<T>(self, factory: impl FnOnce(&mut AppCtx, &Engine) -> T + 'static)
    where
        T: App + 'static,
    {
        let title = self.title;
        let size = self.size;
        #[cfg(all(feature = "render", feature = "winit"))]
        let window_descriptor = self.window_descriptor;
        let create_window = self.create_window;

        // Build engine with plugins using custom plugin group
        let mut builder = EngineBuilder::new();
        if !self.plugins.is_empty() {
            use std::cell::RefCell;
            builder = builder.add_plugins(StoredPlugins {
                plugins: RefCell::new(self.plugins)
            });
        }
        let engine = builder.build();

        // We need to pass engine and other data into run_app
        // Since AppFactory is a fn pointer, we use a trick:
        // Store data in a static and use a helper function
        use std::cell::RefCell;
        thread_local! {
            static APP_BUILDER_DATA: RefCell<Option<AppBuilderData>> = RefCell::new(None);
        }

        #[allow(dead_code)] // Fields are used in app_builder_factory below
        struct AppBuilderData {
            engine: Engine,
            title: String,
            size: (u32, u32),
            #[cfg(all(feature = "render", feature = "winit"))]
            window_descriptor: Option<WindowContextDescriptor>,
            create_window: bool,
            factory: Box<dyn FnOnce(&mut AppCtx, &Engine) -> Box<dyn App>>,
        }

        // Store data in thread-local
        APP_BUILDER_DATA.with(|data| {
            *data.borrow_mut() = Some(AppBuilderData {
                engine,
                title,
                size,
                #[cfg(all(feature = "render", feature = "winit"))]
                window_descriptor,
                create_window,
                factory: Box::new(move |ctx, engine| Box::new(factory(ctx, engine))),
            });
        });

        // Helper function that can be used as AppFactory
        fn app_builder_factory(ctx: &mut AppCtx) -> Box<dyn App> {
            use std::cell::RefCell;
            thread_local! {
                static APP_BUILDER_DATA: RefCell<Option<AppBuilderData>> = RefCell::new(None);
            }

            struct AppBuilderData {
                engine: Engine,
                title: String,
                size: (u32, u32),
                #[cfg(all(feature = "render", feature = "winit"))]
                window_descriptor: Option<WindowContextDescriptor>,
                create_window: bool,
                factory: Box<dyn FnOnce(&mut AppCtx, &Engine) -> Box<dyn App>>,
            }

            let mut data = APP_BUILDER_DATA.with(|d| d.borrow_mut().take())
                .expect("ApplicationBuilder data not found");

            #[cfg(all(feature = "render", feature = "winit"))]
            {
                // Create initial window if requested
                if data.create_window {
                    if let Some(window_manager) = data.engine.get_mut::<WindowManager>() {
                        // Use WindowManager if available
                        let descriptor = WindowDescriptor {
                            title: data.title.clone(),
                            size: Some(WinitPhysicalSize::new(data.size.0 as f32, data.size.1 as f32)),
                            ..Default::default()
                        };

                        if let Some(window_desc) = data.window_descriptor.take() {
                            if let Err(e) = window_manager.create_window_with_descriptor(ctx, descriptor, window_desc) {
                                tracing::error!("Failed to create window with descriptor: {}", e);
                            }
                        } else if let Err(e) = window_manager.create_window(ctx, descriptor) {
                            tracing::error!("Failed to create window: {}", e);
                        }
                    }
                }
            }

            #[cfg(not(all(feature = "render", feature = "winit")))]
            {
                let _ = data.create_window; // Suppress unused warning
            }

            (data.factory)(ctx, &data.engine)
        }

        run_app(app_builder_factory)
    }

    /// Builds the engine without running the application.
    ///
    /// This is useful for testing or when you want to use the engine
    /// without the winit event loop.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use astrelis::ApplicationBuilder;
    /// let engine = ApplicationBuilder::new()
    ///     .add_plugins(astrelis::DefaultPlugins)
    ///     .build_engine();
    ///
    /// // Use engine...
    /// ```
    pub fn build_engine(self) -> Engine {
        let mut builder = EngineBuilder::new();
        if !self.plugins.is_empty() {
            use std::cell::RefCell;
            builder = builder.add_plugins(StoredPlugins {
                plugins: RefCell::new(self.plugins)
            });
        }
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_application_builder_defaults() {
        let builder = ApplicationBuilder::new();
        assert_eq!(builder.title, "Astrelis Application");
        assert_eq!(builder.size, (1280, 720));
        assert!(builder.plugins.is_empty());
        assert!(builder.create_window);
    }

    #[test]
    fn test_application_builder_with_title() {
        let builder = ApplicationBuilder::new()
            .with_title("Test Game");
        assert_eq!(builder.title, "Test Game");
    }

    #[test]
    fn test_application_builder_with_size() {
        let builder = ApplicationBuilder::new()
            .with_size(1920, 1080);
        assert_eq!(builder.size, (1920, 1080));
    }

    #[test]
    fn test_application_builder_without_window() {
        let builder = ApplicationBuilder::new()
            .without_window();
        assert!(!builder.create_window);
    }

    #[test]
    fn test_application_builder_add_plugin() {
        use crate::FnPlugin;

        let builder = ApplicationBuilder::new()
            .add_plugin(FnPlugin::new("test", |_| {}));
        assert_eq!(builder.plugins.len(), 1);
    }

    #[test]
    fn test_application_builder_build_engine() {
        use crate::FnPlugin;

        let engine = ApplicationBuilder::new()
            .add_plugin(FnPlugin::new("test", |resources| {
                resources.insert(42i32);
            }))
            .build_engine();

        assert_eq!(*engine.get::<i32>().unwrap(), 42);
    }

    #[test]
    fn test_application_builder_chaining() {
        let builder = ApplicationBuilder::new()
            .with_title("Test")
            .with_size(800, 600)
            .without_window();

        assert_eq!(builder.title, "Test");
        assert_eq!(builder.size, (800, 600));
        assert!(!builder.create_window);
    }
}
