//! Application builder and runner.

use std::collections::HashMap;

use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::window_id::WindowId;

use crate::events::Events;
use crate::phase::Phase;
use crate::plugin::Plugin;
use crate::resources::Resources;
use crate::time::Time;

/// Type-erased system function.
type SystemFn = Box<dyn Fn(&Resources)>;

/// Type-erased event swap function (calls `Events<T>::swap()`).
type EventSwapFn = Box<dyn Fn(&Resources)>;

/// Type-erased startup function that receives mutable resources and
/// the event loop context for window/GPU initialization.
type StartupFn = Box<dyn FnOnce(&mut Resources, &mut dyn EventLoopContext)>;

/// The application builder and runner.
///
/// Users configure plugins, resources, and systems, then call [`run`](App::run)
/// to enter the event loop.
///
/// # Example
///
/// ```ignore
/// use astrelis_app::{App, Phase, Plugin};
///
/// struct MyGame;
///
/// impl Plugin for MyGame {
///     fn build(&self, app: &mut App) {
///         app.add_system(Phase::Update, |res| {
///             // game logic
///         });
///     }
/// }
///
/// App::new()
///     .add_default_plugins()
///     .add_plugin(MyGame)
///     .run();
/// ```
pub struct App {
    resources: Resources,
    startup_fns: Vec<StartupFn>,
    systems: HashMap<Phase, Vec<SystemFn>>,
    event_swaps: Vec<EventSwapFn>,
    startup_done: bool,
    primary_window_id: Option<WindowId>,
}

impl App {
    /// Creates a new application with no plugins or systems.
    pub fn new() -> Self {
        let mut systems = HashMap::new();
        for &phase in Phase::frame_phases() {
            systems.insert(phase, Vec::new());
        }
        Self {
            resources: Resources::new(),
            startup_fns: Vec::new(),
            systems,
            event_swaps: Vec::new(),
            startup_done: false,
            primary_window_id: None,
        }
    }

    /// Registers all default engine plugins.
    ///
    /// This adds: [`WindowPlugin`](crate::plugins::window::WindowPlugin),
    /// [`GpuPlugin`](crate::plugins::gpu::GpuPlugin),
    /// [`InputPlugin`](crate::plugins::input::InputPlugin),
    /// [`AssetPlugin`](crate::plugins::asset::AssetPlugin),
    /// [`TimePlugin`](crate::plugins::time::TimePlugin), and
    /// [`ProfilingPlugin`](crate::plugins::profiling::ProfilingPlugin).
    pub fn add_default_plugins(self) -> Self {
        self.add_plugin(crate::plugins::window::WindowPlugin::default())
            .add_plugin(crate::plugins::gpu::GpuPlugin)
            .add_plugin(crate::plugins::input::InputPlugin)
            .add_plugin(crate::plugins::asset::AssetPlugin::default())
            .add_plugin(crate::plugins::time::TimePlugin)
            .add_plugin(crate::plugins::profiling::ProfilingPlugin)
    }

    /// Registers a plugin.
    pub fn add_plugin(mut self, plugin: impl Plugin) -> Self {
        plugin.build(&mut self);
        self
    }

    /// Inserts a resource into the type-map.
    ///
    /// If a resource of the same type already exists, it is replaced.
    pub fn insert_resource<T: 'static>(&mut self, value: T) {
        self.resources.insert(value);
    }

    /// Registers a startup function that runs once when the event loop
    /// starts.
    ///
    /// Unlike regular systems, startup functions receive `&mut Resources`
    /// and `&mut dyn EventLoopContext`, enabling window creation, GPU
    /// initialization, and other one-time setup that needs both.
    pub fn add_startup<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Resources, &mut dyn EventLoopContext) + 'static,
    {
        self.startup_fns.push(Box::new(f));
    }

    /// Registers a system to run in the given phase.
    ///
    /// Systems within the same phase run in registration order.
    pub fn add_system<F>(&mut self, phase: Phase, system: F)
    where
        F: Fn(&Resources) + 'static,
    {
        self.systems
            .entry(phase)
            .or_default()
            .push(Box::new(system));
    }

    /// Registers a typed event channel.
    ///
    /// This inserts an [`Events<T>`] resource and registers its buffer
    /// swap to run at the start of each frame.
    pub fn add_event<T: 'static>(&mut self) {
        self.resources.insert(Events::<T>::new());
        let swap: EventSwapFn = Box::new(|resources: &Resources| {
            let mut events = resources.get_mut::<Events<T>>();
            events.swap();
        });
        self.event_swaps.push(swap);
    }

    /// Returns a reference to the resources container.
    ///
    /// Useful during plugin setup for checking existing resources.
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Returns a mutable reference to the resources container.
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    /// Enters the event loop.
    ///
    /// This consumes the builder, creates the window, and runs the
    /// application until exit. On most platforms this never returns.
    pub fn run(mut self) -> ! {
        astrelis_profiling::init();
        astrelis_profiling::set_thread_name("main");

        // Insert Time if not already present (TimePlugin inserts it, but
        // allow running without default plugins).
        if !self.resources.contains::<Time>() {
            self.resources.insert(Time::new());
        }

        astrelis_window::run(&mut self).expect("event loop error");

        // winit::run() typically doesn't return, but if it does:
        std::process::exit(0);
    }

    /// Runs all systems registered in a given phase.
    fn run_phase(&self, phase: Phase) {
        astrelis_profiling::profile_scope!("phase", format!("{phase:?}"));
        if let Some(systems) = self.systems.get(&phase) {
            for system in systems {
                system(&self.resources);
            }
        }
    }

    /// Swaps all event buffers. Called at the start of PreUpdate.
    fn swap_events(&self) {
        for swap in &self.event_swaps {
            swap(&self.resources);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                if !self.startup_done {
                    // Run all startup functions with full access to
                    // resources and the event loop context.
                    let startup_fns = std::mem::take(&mut self.startup_fns);
                    for f in startup_fns {
                        f(&mut self.resources, ctx);
                    }
                    self.startup_done = true;

                    // Resolve the primary window ID from the window plugin.
                    if let Some(id) = self.resources.remove::<PrimaryWindowId>() {
                        self.primary_window_id = Some(id.0);
                    }
                }
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {}
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        astrelis_profiling::profile_function!();

        // Feed events to the input system if present.
        if let Some(mut input) = self.resources.try_get_mut::<astrelis_input::InputState>() {
            input.handle_event(&event);
        }

        // Store window events in the event bus if registered.
        if let Some(mut events) = self.resources.try_get_mut::<Events<WindowEvent>>() {
            events.send(event.clone());
        }

        // Handle built-in window management.
        match &event {
            WindowEvent::CloseRequested => {
                ctx.exit();
            }
            WindowEvent::Resized(size) => {
                let phys = size.physical();
                let w = phys.width as u32;
                let h = phys.height as u32;
                if w > 0
                    && h > 0
                    && let Some(mut surface) =
                        self.resources.try_get_mut::<astrelis_gpu::Surface>()
                {
                    let config = astrelis_gpu::surface::SurfaceConfiguration {
                        format: surface.preferred_format(),
                        width: w,
                        height: h,
                        present_mode: astrelis_gpu::types::PresentMode::AutoVsync,
                        desired_maximum_frame_latency: 2,
                    };
                    surface.configure(&config);
                }
            }
            _ => {}
        }
    }

    fn on_device_event(
        &mut self,
        _ctx: &mut dyn EventLoopContext,
        event: astrelis_window::event::DeviceEvent,
    ) {
        astrelis_profiling::profile_function!();
        if let Some(mut input) = self.resources.try_get_mut::<astrelis_input::InputState>() {
            input.handle_device_event(&event);
        }
    }

    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();

        // Update time.
        {
            let mut time = self.resources.get_mut::<Time>();
            time.update();
        }

        // Swap event buffers.
        self.swap_events();

        // PreUpdate.
        self.run_phase(Phase::PreUpdate);

        // FixedUpdate: run 0..N times based on accumulator.
        {
            let mut steps = 0;
            loop {
                let should_step = {
                    let mut time = self.resources.get_mut::<Time>();
                    time.consume_fixed_step()
                };
                if !should_step {
                    break;
                }
                self.run_phase(Phase::FixedUpdate);
                steps += 1;
                // Safety valve: don't run more than 10 fixed steps per frame.
                if steps >= 10 {
                    tracing::warn!("FixedUpdate capped at 10 steps this frame");
                    break;
                }
            }
        }

        // Update.
        self.run_phase(Phase::Update);

        // PostUpdate.
        self.run_phase(Phase::PostUpdate);

        // Render.
        self.run_phase(Phase::Render);

        // Present.
        self.run_phase(Phase::Present);

        // Request redraw to keep the loop going.
        if let Some(win_id) = self.primary_window_id
            && let Some(win) = ctx.window(win_id)
        {
            win.request_redraw();
        }

        // Mark profiling frame.
        astrelis_profiling::new_frame();
    }
}

/// Resource temporarily holding the primary window ID during startup.
/// Moved into `App::primary_window_id` after startup completes.
pub struct PrimaryWindowId(pub WindowId);
