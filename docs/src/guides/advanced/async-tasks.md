# Async Tasks

This guide explains how to use asynchronous tasks in Astrelis for background work, parallel asset loading, and non-blocking operations without freezing the main thread.

## Overview

**Async tasks** enable:

- Background asset loading
- Parallel computations
- Non-blocking file I/O
- Network requests
- Long-running operations without frame drops

**Key Concepts:**
- TaskPool for background work
- AsyncRuntimePlugin setup
- Spawning tasks with `spawn()`
- Handling task results
- Async/await patterns
- Thread pool configuration

**Comparison to Unity:** Similar to Unity Coroutines but using Rust's async/await with actual parallelism.

## TaskPool Overview

### AsyncRuntimePlugin Setup

```rust
use astrelis::{Engine, Plugin};
use astrelis_core::async_runtime::{AsyncRuntimePlugin, TaskPool};

fn main() {
    let engine = Engine::builder()
        .add_plugin(AsyncRuntimePlugin::default())
        .build();

    // TaskPool is now available as a resource
    let task_pool = engine.get::<TaskPool>().unwrap();
}
```

**Configuration:**
```rust
let engine = Engine::builder()
    .add_plugin(AsyncRuntimePlugin {
        num_threads: 4, // Thread pool size
    })
    .build();
```

### Spawning Background Tasks

**Basic task spawning:**
```rust
use astrelis_core::async_runtime::TaskPool;

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        let task_pool = ctx.engine.get::<TaskPool>().unwrap();

        // Spawn background task
        let handle = task_pool.spawn(async {
            // This runs on background thread
            expensive_computation()
        });

        // Store handle for later
        self.pending_tasks.push(handle);
    }
}
```

**Checking task completion:**
```rust
use futures::future::Future;

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Check completed tasks
        self.pending_tasks.retain(|handle| {
            if handle.is_ready() {
                let result = handle.now_or_never().unwrap();
                info!("Task completed: {:?}", result);
                false // Remove from list
            } else {
                true // Keep in list
            }
        });
    }
}
```

## Parallel Asset Loading

### Background Asset Loading

**Loading multiple assets in parallel:**
```rust
use astrelis_assets::{AssetServer, Handle};
use futures::future::join_all;

impl App for MyGame {
    async fn load_level_assets(&self, level: &str) -> Vec<Handle<Texture>> {
        let asset_server = self.engine.get::<AssetServer>().unwrap();

        let texture_paths = vec![
            format!("levels/{}/background.png", level),
            format!("levels/{}/tileset.png", level),
            format!("levels/{}/characters.png", level),
        ];

        // Load all textures in parallel
        let load_futures: Vec<_> = texture_paths
            .iter()
            .map(|path| async move {
                asset_server.load::<Texture>(path).await
            })
            .collect();

        // Wait for all loads to complete
        join_all(load_futures).await
    }
}
```

**Integration with game loop:**
```rust
impl App for MyGame {
    fn on_level_start(&mut self, ctx: &mut AppCtx, level: &str) {
        let task_pool = ctx.engine.get::<TaskPool>().unwrap();

        // Spawn loading task
        let level_name = level.to_string();
        let handle = task_pool.spawn(async move {
            self.load_level_assets(&level_name).await
        });

        self.loading_task = Some(handle);
        self.game_state = GameState::Loading;
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        if let Some(handle) = &self.loading_task {
            if handle.is_ready() {
                let textures = handle.now_or_never().unwrap();
                info!("Loaded {} textures", textures.len());

                self.textures = textures;
                self.game_state = GameState::Playing;
                self.loading_task = None;
            }
        }
    }
}
```

### Progress Tracking

**Tracking async load progress:**
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct LoadingProgress {
    total: usize,
    loaded: Arc<AtomicUsize>,
}

impl LoadingProgress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            loaded: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn increment(&self) {
        self.loaded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn progress(&self) -> f32 {
        let loaded = self.loaded.load(Ordering::Relaxed);
        loaded as f32 / self.total as f32
    }
}

impl App for MyGame {
    async fn load_assets_with_progress(&self, paths: Vec<String>) -> Vec<Handle<Texture>> {
        let progress = LoadingProgress::new(paths.len());
        self.loading_progress = Some(progress.clone());

        let asset_server = self.engine.get::<AssetServer>().unwrap();

        let mut handles = Vec::new();
        for path in paths {
            let handle = asset_server.load::<Texture>(&path).await?;
            handles.push(handle);

            progress.increment();
        }

        handles
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        if let Some(progress) = &self.loading_progress {
            let percent = progress.progress() * 100.0;
            self.ui.update_text("progress", &format!("Loading: {:.0}%", percent));
        }
    }
}
```

## Non-Blocking File I/O

### Async File Reading

**Reading files without blocking:**
```rust
use tokio::fs::File;
use tokio::io::AsyncReadExt;

impl App for MyGame {
    async fn load_config_file(&self, path: &str) -> Result<String, std::io::Error> {
        let mut file = File::open(path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        Ok(contents)
    }

    fn on_start(&mut self, ctx: &mut AppCtx) {
        let task_pool = ctx.engine.get::<TaskPool>().unwrap();

        let handle = task_pool.spawn(async move {
            self.load_config_file("config.json").await
        });

        self.config_loading = Some(handle);
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        if let Some(handle) = &self.config_loading {
            if handle.is_ready() {
                match handle.now_or_never().unwrap() {
                    Ok(contents) => {
                        self.parse_config(&contents);
                    }
                    Err(e) => {
                        error!("Failed to load config: {:?}", e);
                    }
                }
                self.config_loading = None;
            }
        }
    }
}
```

### Async File Writing

**Saving files without blocking:**
```rust
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

impl App for MyGame {
    async fn save_game(&self, path: &str, data: &GameSaveData) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(data)?;

        let mut file = File::create(path).await?;
        file.write_all(json.as_bytes()).await?;
        file.sync_all().await?;

        Ok(())
    }

    fn on_save_requested(&mut self, ctx: &mut AppCtx) {
        let task_pool = ctx.engine.get::<TaskPool>().unwrap();

        let save_data = self.create_save_data();
        let handle = task_pool.spawn(async move {
            self.save_game("save.json", &save_data).await
        });

        self.saving_task = Some(handle);
        self.show_notification("Saving...");
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        if let Some(handle) = &self.saving_task {
            if handle.is_ready() {
                match handle.now_or_never().unwrap() {
                    Ok(_) => {
                        self.show_notification("Saved successfully!");
                    }
                    Err(e) => {
                        error!("Save failed: {:?}", e);
                        self.show_notification("Save failed!");
                    }
                }
                self.saving_task = None;
            }
        }
    }
}
```

## Parallel Computations

### Background Processing

**Offloading expensive computations:**
```rust
use rayon::prelude::*;

impl App for MyGame {
    async fn process_particles(&self, particles: Vec<Particle>) -> Vec<Particle> {
        // Process in parallel on thread pool
        tokio::task::spawn_blocking(move || {
            particles.into_par_iter()
                .map(|mut particle| {
                    particle.update(0.016); // Expensive physics
                    particle
                })
                .collect()
        }).await.unwrap()
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        if self.particle_task.is_none() && !self.particles.is_empty() {
            let task_pool = ctx.engine.get::<TaskPool>().unwrap();

            let particles = self.particles.clone();
            let handle = task_pool.spawn(async move {
                self.process_particles(particles).await
            });

            self.particle_task = Some(handle);
        }

        // Apply results when ready
        if let Some(handle) = &self.particle_task {
            if handle.is_ready() {
                self.particles = handle.now_or_never().unwrap();
                self.particle_task = None;
            }
        }
    }
}
```

### Chunked Processing

**Spreading work across multiple frames:**
```rust
pub struct ChunkedProcessor<T> {
    items: Vec<T>,
    chunk_size: usize,
    current_index: usize,
}

impl<T> ChunkedProcessor<T> {
    pub fn new(items: Vec<T>, chunk_size: usize) -> Self {
        Self {
            items,
            chunk_size,
            current_index: 0,
        }
    }

    pub fn process_chunk<F>(&mut self, mut process_fn: F) -> bool
    where
        F: FnMut(&mut T),
    {
        let end = (self.current_index + self.chunk_size).min(self.items.len());

        for i in self.current_index..end {
            process_fn(&mut self.items[i]);
        }

        self.current_index = end;
        self.is_complete()
    }

    pub fn is_complete(&self) -> bool {
        self.current_index >= self.items.len()
    }

    pub fn progress(&self) -> f32 {
        self.current_index as f32 / self.items.len() as f32
    }
}

// Usage
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        if let Some(processor) = &mut self.chunked_processor {
            // Process 100 items per frame
            processor.process_chunk(|item| {
                expensive_operation(item);
            });

            // Update progress
            self.ui.update_text("progress", &format!(
                "Processing: {:.0}%",
                processor.progress() * 100.0
            ));

            // Complete?
            if processor.is_complete() {
                info!("Chunked processing complete");
                self.chunked_processor = None;
            }
        }
    }
}
```

## Frame Timing Considerations

### Budget Management

**Keeping frame time under budget:**
```rust
use std::time::{Instant, Duration};

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        let frame_start = Instant::now();
        let frame_budget = Duration::from_millis(16); // 60 FPS

        // Process as many items as fit in budget
        while let Some(task) = self.background_tasks.pop() {
            task.execute();

            // Check if we're running out of time
            if frame_start.elapsed() > frame_budget * 0.8 {
                info!("Frame budget 80% used, deferring remaining tasks");
                break;
            }
        }
    }
}
```

### Async Task Priority

**Prioritizing important tasks:**
```rust
use std::cmp::Ordering;

pub struct PrioritizedTask {
    priority: u32,
    task: Box<dyn Future<Output = ()>>,
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first
        other.priority.cmp(&self.priority)
    }
}

impl App for MyGame {
    fn spawn_with_priority(&mut self, priority: u32, task: impl Future<Output = ()> + 'static) {
        self.task_queue.push(PrioritizedTask {
            priority,
            task: Box::new(task),
        });

        // Sort by priority
        self.task_queue.sort();
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Process highest priority tasks first
        while let Some(task) = self.task_queue.pop() {
            task_pool.spawn(task.task);

            if frame_budget_exceeded() {
                break;
            }
        }
    }
}
```

## Thread Pool Configuration

### Sizing Thread Pool

**Determining optimal thread count:**
```rust
use astrelis_core::async_runtime::AsyncRuntimePlugin;

fn create_engine() -> Engine {
    let num_threads = determine_thread_count();

    Engine::builder()
        .add_plugin(AsyncRuntimePlugin {
            num_threads,
        })
        .build()
}

fn determine_thread_count() -> usize {
    let cpu_count = num_cpus::get();

    // Common strategies:
    // 1. All cores: cpu_count
    // 2. Leave one for main thread: cpu_count - 1
    // 3. Half cores for background: cpu_count / 2
    // 4. Fixed count: 4

    (cpu_count - 1).max(1) // Leave one core for main thread
}
```

### Per-Task Thread Control

**Running tasks on specific threads:**
```rust
impl App for MyGame {
    fn spawn_io_task(&mut self) {
        // I/O tasks: single thread
        let handle = tokio::task::spawn(async {
            load_file("data.json").await
        });
    }

    fn spawn_compute_task(&mut self) {
        // Compute tasks: blocking thread pool
        let handle = tokio::task::spawn_blocking(|| {
            expensive_computation()
        });
    }
}
```

## Complete Async Example

Full example with parallel asset loading and progress tracking:

```rust
use astrelis::*;
use astrelis_core::async_runtime::{AsyncRuntimePlugin, TaskPool};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct AsyncDemo {
    graphics: Arc<GraphicsContext>,
    renderable: RenderableWindow,
    ui: UiSystem,
    loading_task: Option<TaskHandle<Vec<Handle<Texture>>>>,
    textures: Vec<Handle<Texture>>,
    loading_progress: Arc<AtomicUsize>,
}

impl App for AsyncDemo {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Build loading UI
        self.ui.build(|root| {
            root.column()
                .padding(Length::px(20))
                .child(|c| c.text("Loading Assets...").font_size(24.0).build())
                .child(|c| c.text("0%").id("progress").font_size(18.0).build())
                .child(|c| {
                    c.button("Start Loading")
                        .id("load_button")
                        .on_click(|| {
                            info!("Load button clicked");
                        })
                        .build()
                })
                .build();
        });
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Update progress
        if self.loading_task.is_some() {
            let progress = self.loading_progress.load(Ordering::Relaxed);
            let percent = (progress as f32 / 10.0) * 100.0;
            self.ui.update_text("progress", &format!("{:.0}%", percent));
        }

        // Check if loading complete
        if let Some(handle) = &self.loading_task {
            if handle.is_ready() {
                self.textures = handle.now_or_never().unwrap();
                info!("Loading complete: {} textures", self.textures.len());

                self.ui.update_text("progress", "Complete!");
                self.loading_task = None;
            }
        }
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Handle load button
        events.dispatch(|event| {
            if let Event::WidgetClicked { id } = event {
                if id == "load_button" {
                    self.start_loading(ctx);
                    return HandleStatus::consumed();
                }
            }
            HandleStatus::ignored()
        });

        // Render
        let mut frame = self.renderable.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb(30, 30, 40),
            |pass| {
                self.ui.render(pass.wgpu_pass());
            },
        );

        frame.finish();
    }
}

impl AsyncDemo {
    fn start_loading(&mut self, ctx: &mut AppCtx) {
        let task_pool = ctx.engine.get::<TaskPool>().unwrap();
        let progress = self.loading_progress.clone();

        let handle = task_pool.spawn(async move {
            let asset_server = ctx.engine.get::<AssetServer>().unwrap();
            let mut handles = Vec::new();

            for i in 0..10 {
                let path = format!("textures/asset_{}.png", i);
                let handle = asset_server.load::<Texture>(&path).await?;
                handles.push(handle);

                progress.fetch_add(1, Ordering::Relaxed);

                // Simulate loading delay
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            Ok(handles)
        });

        self.loading_task = Some(handle);
        self.loading_progress.store(0, Ordering::Relaxed);
    }
}

fn main() {
    let engine = Engine::builder()
        .add_plugin(AsyncRuntimePlugin::default())
        .build();

    run_app(|ctx| {
        // Create app...
        Box::new(AsyncDemo { /* ... */ })
    });
}
```

## Best Practices

### ✅ DO: Use Async for I/O

```rust
// GOOD: Async file loading
async fn load_file(path: &str) -> String {
    tokio::fs::read_to_string(path).await.unwrap()
}
```

### ✅ DO: Track Task Progress

```rust
// GOOD: Progress tracking
let progress = Arc::new(AtomicUsize::new(0));
// Update as work progresses
progress.fetch_add(1, Ordering::Relaxed);
```

### ✅ DO: Handle Task Errors

```rust
// GOOD: Error handling
match task_handle.now_or_never() {
    Some(Ok(result)) => { /* Use result */ }
    Some(Err(e)) => { error!("Task failed: {:?}", e); }
    None => { /* Still running */ }
}
```

### ❌ DON'T: Block Main Thread

```rust
// BAD: Blocking main thread
let result = std::fs::read_to_string("file.txt").unwrap(); // Blocks!

// GOOD: Async I/O
let result = tokio::fs::read_to_string("file.txt").await?;
```

### ❌ DON'T: Forget to Check Completion

```rust
// BAD: Spawning and forgetting
task_pool.spawn(async { expensive_work() });

// GOOD: Storing handle and checking
let handle = task_pool.spawn(async { expensive_work() });
self.tasks.push(handle);
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| Coroutines | async/await | True parallelism in Astrelis |
| StartCoroutine() | task_pool.spawn() | Spawning async work |
| yield return | .await | Suspension point |
| WWW/UnityWebRequest | tokio::fs, reqwest | I/O operations |

## Next Steps

- **Practice:** Add async asset loading to your game
- **Optimize:** Use parallel computations for heavy work
- **Monitor:** Track frame timing with async tasks
- **Examples:** `async_loading_demo`, `parallel_compute`

## See Also

- [Asset System](../asset-system/loading-assets.md) - Async asset loading
- [Performance Tuning](performance-tuning.md) - Frame timing
- API Reference: [`TaskPool`](../../api/astrelis-core/async_runtime/struct.TaskPool.html)
