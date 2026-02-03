# Performance Tuning

This guide explains how to profile and optimize your Astrelis application for production performance. Learn to identify bottlenecks and apply targeted optimizations.

## Overview

**Performance tuning** involves:

- Profiling CPU and GPU work
- Identifying bottlenecks
- Applying targeted optimizations
- Measuring impact
- Production performance checklist

**Key Tools:**
- Puffin profiler for CPU profiling
- GPU timestamp queries
- Frame timing analysis
- Memory profiling

**Comparison to Unity:** Similar to Unity Profiler but focused on Rust-specific patterns (Arc clones, allocations, GPU submissions).

## Profiling Setup

### Initializing Puffin

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend};

fn main() {
    // Initialize puffin HTTP server
    init_profiling(ProfilingBackend::PuffinHttp);

    // Your application code
    run_app(|ctx| {
        // ...
    });
}
```

**Access profiler:** Open browser to `http://127.0.0.1:8585`

### Profiling Scopes

```rust
use astrelis_core::profiling::profile_function;

fn update_physics(delta: f32) {
    profile_function!(); // Automatically named from function

    // Physics code...
}

fn custom_scope() {
    {
        puffin::profile_scope!("expensive_operation");
        // Code to profile
    }

    {
        puffin::profile_scope!("another_operation");
        // More code
    }
}
```

### Frame Markers

```rust
use astrelis_core::profiling::{new_frame, finish_frame};

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        new_frame(); // Start new profiler frame

        profile_function!();

        // Game update logic...

        finish_frame(); // End profiler frame
    }
}
```

## Identifying CPU Bottlenecks

### Reading Puffin Flamegraphs

**Flamegraph interpretation:**
- **Width**: Time spent in function
- **Height**: Call stack depth
- **Color**: Different functions
- **Hot spots**: Wide bars = expensive operations

**Common patterns:**
```
Frame (16.6ms @ 60 FPS)
├─ update() (8ms) ← Large, investigate
│  ├─ physics_update() (6ms) ← Major contributor
│  └─ ai_update() (2ms)
└─ render() (7ms)
   ├─ ui_rebuild() (5ms) ← Too expensive!
   └─ draw_calls() (2ms)
```

### Frame Budget Analysis

**60 FPS target:**
- Total budget: 16.6ms
- Update: ~8ms target
- Render: ~8ms target

**30 FPS target:**
- Total budget: 33.3ms
- More forgiving, but aim for 60 FPS

**Measuring frame time:**
```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        profile_function!();

        // Track frame times
        self.frame_times.push(time.delta.as_secs_f32() * 1000.0);

        // Log slow frames
        if time.delta.as_secs_f32() > 0.020 { // > 20ms
            warn!("Slow frame: {:.2}ms", time.delta.as_secs_f32() * 1000.0);
        }
    }
}
```

### Common CPU Bottlenecks

**1. Excessive UI Rebuilds**

```rust
// BAD: Full rebuild every frame (20ms)
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        ui.build(|root| {
            // Rebuilding entire tree every frame!
            root.text(&format!("FPS: {}", self.fps)).build();
        });
    }
}

// GOOD: Incremental update (<1ms)
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Only update changed text
        ui.update_text("fps_label", &format!("FPS: {}", self.fps));
    }
}
```

**Performance impact:**
- Full rebuild: ~20ms
- Incremental update: <1ms
- **Speedup: 20x**

**2. Text Shaping Every Frame**

```rust
// BAD: Text shaping every frame (5-10ms)
ui.build(|root| {
    root.text(&format!("Score: {}", self.score))
        .id("score")
        .build();
});

// GOOD: Use update_text (reuses shaped cache)
ui.update_text("score", &format!("Score: {}", self.score));
```

**Text shaping costs:**
- Complex text (Arabic, ligatures): ~10ms
- Simple ASCII text: ~1-2ms
- Cached text: <0.1ms

**3. Allocation Hotspots**

```rust
// BAD: Allocating every frame
fn update(&mut self) {
    let enemies: Vec<Enemy> = self.enemies.iter()
        .filter(|e| e.is_alive())
        .cloned()
        .collect(); // New allocation!
}

// GOOD: Reuse allocation
struct GameState {
    alive_enemies: Vec<Enemy>, // Reusable buffer
}

fn update(&mut self) {
    self.alive_enemies.clear();
    for enemy in &self.enemies {
        if enemy.is_alive() {
            self.alive_enemies.push(enemy.clone());
        }
    }
}
```

**4. Excessive Arc Cloning**

```rust
// BAD: Cloning Arc in tight loop
for _ in 0..10_000 {
    let graphics = ctx.engine.get::<Arc<GraphicsContext>>().unwrap();
    // Use graphics (Arc cloned 10k times)
}

// GOOD: Clone once outside loop
let graphics = ctx.engine.get::<Arc<GraphicsContext>>().unwrap();
for _ in 0..10_000 {
    // Use graphics (single Arc clone)
}
```

**Arc clone cost:** ~1ns per clone, but adds up in loops.

## Identifying GPU Bottlenecks

### GPU Timestamp Queries

```rust
use astrelis_render::query::QuerySet;

// Create timestamp query set
let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
    label: Some("Timestamps"),
    ty: wgpu::QueryType::Timestamp,
    count: 2,
});

// Record timestamps
{
    let mut pass = RenderPassBuilder::new()
        .target(RenderTarget::Surface)
        .build(&mut frame);

    pass.write_timestamp(&query_set, 0); // Start

    // Render operations
    ui.render(pass.wgpu_pass());

    pass.write_timestamp(&query_set, 1); // End
}

// Read back results (async)
let elapsed_ns = read_timestamp_results(&query_set, 0, 1).await;
println!("GPU time: {:.2}ms", elapsed_ns as f32 / 1_000_000.0);
```

### Draw Call Analysis

**Counting draw calls:**
```rust
pub struct RenderStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub instances: u32,
}

impl UiRenderer {
    pub fn stats(&self) -> RenderStats {
        RenderStats {
            draw_calls: self.draw_call_count,
            triangles: self.triangle_count,
            instances: self.instance_count,
        }
    }
}

// Monitor draw calls
let stats = ui_renderer.stats();
if stats.draw_calls > 100 {
    warn!("Too many draw calls: {}", stats.draw_calls);
}
```

**Draw call budget:**
- Modern GPU: ~10,000 draw calls @ 60 FPS
- Mobile GPU: ~500-1000 draw calls @ 60 FPS
- **Target: <100 draw calls for UI**

### Common GPU Bottlenecks

**1. Too Many Draw Calls**

```rust
// BAD: One draw call per widget (1000+ calls)
for widget in widgets {
    render_pass.set_vertex_buffer(0, widget.buffer.slice(..));
    render_pass.draw(0..6, 0..1); // 1000 draw calls!
}

// GOOD: Instanced rendering (1 draw call)
render_pass.set_vertex_buffer(0, unit_quad.slice(..));
render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
render_pass.draw(0..6, 0..1000); // 1 draw call
```

**2. Texture Switching**

```rust
// BAD: Texture switch per sprite
for sprite in sprites {
    render_pass.set_bind_group(0, &sprite.texture_bind_group, &[]);
    render_pass.draw(0..6, 0..1);
}

// GOOD: Texture atlas (no switches)
render_pass.set_bind_group(0, &texture_atlas_bind_group, &[]);
render_pass.draw(0..6, 0..sprite_count);
```

**3. Inefficient Shaders**

```wgsl
// BAD: Complex per-pixel computation
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec4<f32>(0.0);

    // Expensive loop per pixel!
    for (var i = 0; i < 100; i++) {
        color += texture_sample(i);
    }

    return color;
}

// GOOD: Precompute or use compute shader
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple lookup
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
```

## Memory Profiling

### Tracking Allocations

**Using system allocator stats:**
```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

pub struct TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

// Check memory usage
pub fn allocated_bytes() -> usize {
    ALLOCATED.load(Ordering::Relaxed)
}
```

### Memory Leak Detection

**Detecting leaked resources:**
```rust
impl Drop for MyResource {
    fn drop(&mut self) {
        debug!("MyResource dropped");
    }
}

// If you never see "MyResource dropped", you have a leak
```

**Common leak sources:**
- Circular Arc references
- Forgotten event listeners
- GPU buffers not dropped
- Texture handles held indefinitely

### GPU Memory Usage

**Tracking GPU allocations:**
```rust
pub struct GpuMemoryTracker {
    buffers: Vec<(String, u64)>,
    textures: Vec<(String, u64)>,
}

impl GpuMemoryTracker {
    pub fn track_buffer(&mut self, label: &str, size: u64) {
        self.buffers.push((label.to_string(), size));
    }

    pub fn total_buffer_memory(&self) -> u64 {
        self.buffers.iter().map(|(_, size)| size).sum()
    }

    pub fn report(&self) {
        info!("GPU Memory Usage:");
        info!("  Buffers: {:.2} MB", self.total_buffer_memory() as f32 / 1_000_000.0);
        info!("  Textures: {:.2} MB", self.total_texture_memory() as f32 / 1_000_000.0);
    }
}
```

## Optimization Techniques

### UI Dirty Flags Tuning

**Understanding dirty flags:**
```rust
pub struct DirtyFlags: u32 {
    const COLOR_ONLY     = 0b0001; // <1ms: Just color changed
    const TEXT_SHAPING   = 0b0010; // 5-10ms: Content changed, reshape needed
    const LAYOUT         = 0b0100; // 10-20ms: Size/position changed
    const GEOMETRY       = 0b1000; // Border/radius changed
}
```

**Optimization strategy:**
```rust
// If only changing color (fastest)
ui.update_color("label", Color::RED); // Marks COLOR_ONLY

// If changing text (medium)
ui.update_text("label", "New text"); // Marks TEXT_SHAPING

// If changing size (slowest)
ui.build(|root| {
    root.text("Resize").width(Length::px(200)).build();
}); // Marks LAYOUT
```

**Performance comparison:**
| Operation | Dirty Flag | Time |
|-----------|-----------|------|
| Color change | COLOR_ONLY | <1ms |
| Text update | TEXT_SHAPING | 5-10ms |
| Layout change | LAYOUT | 10-20ms |
| Full rebuild | All flags | 20-50ms |

### Text Shaping Cache

**Reusing shaped text:**
```rust
pub struct ShapedTextCache {
    cache: HashMap<u64, Arc<ShapedTextData>>,
}

impl Widget {
    fn get_or_shape_text(&mut self, text: &str) -> Arc<ShapedTextData> {
        let hash = calculate_hash(text);

        if let Some(shaped) = self.text_cache.get(&hash) {
            // Cache hit: <0.1ms
            return shaped.clone();
        }

        // Cache miss: shape and store (5-10ms)
        let shaped = Arc::new(shape_text(text));
        self.text_cache.insert(hash, shaped.clone());
        shaped
    }
}
```

**Cache effectiveness:**
- Cache hit: ~0.1ms
- Cache miss: ~5-10ms
- **Target hit rate: >95%**

### GPU Instancing

**Batching similar objects:**
```rust
// BAD: 1000 draw calls
for sprite in &sprites {
    render_pass.set_push_constants(0, bytemuck::bytes_of(&sprite.transform));
    render_pass.draw(0..6, 0..1);
}

// GOOD: 1 draw call with instancing
let instance_data: Vec<SpriteInstance> = sprites.iter()
    .map(|s| SpriteInstance {
        transform: s.transform,
        color: s.color,
        uv_offset: s.uv_offset,
    })
    .collect();

upload_to_instance_buffer(&instance_data);
render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
render_pass.draw(0..6, 0..sprite_count);
```

**Performance impact:**
- 1000 individual draws: ~5ms
- 1 instanced draw: ~0.5ms
- **Speedup: 10x**

### Batch Size Tuning

**Finding optimal batch size:**
```rust
pub struct BatchConfig {
    pub max_quads_per_batch: u32,
    pub max_instances_per_draw: u32,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_quads_per_batch: 10_000,   // Desktop default
            max_instances_per_draw: 1_000, // Desktop default
        }
    }
}

// Mobile configuration
impl BatchConfig {
    pub fn mobile() -> Self {
        Self {
            max_quads_per_batch: 2_000,
            max_instances_per_draw: 500,
        }
    }
}
```

**Tuning strategy:**
1. Start with default settings
2. Profile GPU time with timestamp queries
3. Increase batch size if GPU is idle
4. Decrease if hitting GPU limits

### Buffer Pooling

**Reusing allocations:**
```rust
use astrelis_render::buffer_pool::BufferPool;

pub struct Renderer {
    buffer_pool: BufferPool,
}

impl Renderer {
    pub fn render_frame(&mut self) {
        // Allocate from pool (fast, no allocation)
        let vertex_buffer = self.buffer_pool.alloc(
            size,
            wgpu::BufferUsages::VERTEX,
        );

        // Use buffer...

        // Automatically returned to pool when dropped
    }
}
```

**Performance impact:**
- Allocating new buffer: ~0.5ms
- Pool allocation: ~0.01ms
- **Speedup: 50x**

## Frame Timing Analysis

### Measuring Frame Components

```rust
pub struct FrameTiming {
    pub update_time_ms: f32,
    pub render_time_ms: f32,
    pub present_time_ms: f32,
    pub total_time_ms: f32,
}

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        let start = std::time::Instant::now();

        // Update logic
        self.update_game_logic();

        self.timing.update_time_ms = start.elapsed().as_secs_f32() * 1000.0;
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        let start = std::time::Instant::now();

        // Render logic
        self.render_frame();

        self.timing.render_time_ms = start.elapsed().as_secs_f32() * 1000.0;
        self.timing.total_time_ms = self.timing.update_time_ms + self.timing.render_time_ms;
    }
}
```

### Frame Time Visualization

```rust
pub struct FrameTimeGraph {
    samples: VecDeque<f32>,
    max_samples: usize,
}

impl FrameTimeGraph {
    pub fn push(&mut self, frame_time_ms: f32) {
        self.samples.push_back(frame_time_ms);

        if self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    pub fn average(&self) -> f32 {
        self.samples.iter().sum::<f32>() / self.samples.len() as f32
    }

    pub fn percentile(&self, p: f32) -> f32 {
        let mut sorted: Vec<f32> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = (sorted.len() as f32 * p) as usize;
        sorted[index.min(sorted.len() - 1)]
    }
}

// Usage
let p99 = frame_graph.percentile(0.99);
println!("99th percentile: {:.2}ms", p99); // Worst 1% of frames
```

## Production Performance Checklist

### CPU Optimization Checklist

- [ ] UI uses incremental updates (`update_text`, `update_color`)
- [ ] No full UI rebuilds in update/render loop
- [ ] Text shaping cache hit rate >95%
- [ ] Allocations minimized in hot paths
- [ ] Arc cloning outside tight loops
- [ ] Profiler shows no functions >5ms
- [ ] Frame time average <16ms (60 FPS)
- [ ] 99th percentile frame time <20ms

### GPU Optimization Checklist

- [ ] Draw calls <100 per frame
- [ ] GPU instancing used for repeated objects
- [ ] Texture atlases reduce texture switches
- [ ] Shaders avoid complex per-pixel computation
- [ ] MSAA sample count appropriate (4x desktop, 1x mobile)
- [ ] Render passes use RAII (auto-drop)
- [ ] No GPU synchronization in render loop
- [ ] GPU timestamp queries show <8ms render time

### Memory Optimization Checklist

- [ ] No memory leaks detected
- [ ] Drop implementations clean up resources
- [ ] Buffer pool used for dynamic allocations
- [ ] Texture handles released when unused
- [ ] GPU memory usage <500MB (desktop), <200MB (mobile)
- [ ] No circular Arc references
- [ ] Event listeners properly unregistered

### Platform-Specific Checklist

**Desktop:**
- [ ] 60 FPS maintained at 1920x1080
- [ ] VSync properly configured
- [ ] Window resizing smooth
- [ ] Multi-monitor support tested

**Mobile:**
- [ ] 30 FPS maintained (60 FPS target if possible)
- [ ] Battery consumption acceptable
- [ ] Thermal throttling handled gracefully
- [ ] Touch input responsive

**Web (WASM):**
- [ ] Initial load time <3 seconds
- [ ] Frame time budget adjusted (33ms @ 30 FPS)
- [ ] Asset streaming implemented
- [ ] Browser compatibility tested

## Profiling Example

Complete profiling integration:

```rust
use astrelis::*;
use astrelis_core::profiling::*;
use std::collections::VecDeque;

struct ProfilingDemo {
    frame_times: VecDeque<f32>,
    ui: UiSystem,
}

impl App for ProfilingDemo {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        profile_function!();

        // Initialize profiling
        init_profiling(ProfilingBackend::PuffinHttp);

        // Build UI once
        self.ui.build(|root| {
            root.column()
                .padding(Length::px(20))
                .child(|c| c.text("Frame Time Graph").id("title").build())
                .child(|c| c.text("0.00ms").id("avg_frame_time").build())
                .child(|c| c.text("0.00ms").id("p99_frame_time").build())
                .build();
        });
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        profile_function!();
        new_frame();

        // Track frame time
        let frame_ms = time.delta.as_secs_f32() * 1000.0;
        self.frame_times.push_back(frame_ms);

        if self.frame_times.len() > 120 {
            self.frame_times.pop_front();
        }

        // Update UI with stats
        let avg = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        let mut sorted = self.frame_times.iter().copied().collect::<Vec<_>>();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p99 = sorted[(sorted.len() as f32 * 0.99) as usize];

        self.ui.update_text("avg_frame_time", &format!("Avg: {:.2}ms", avg));
        self.ui.update_text("p99_frame_time", &format!("P99: {:.2}ms", p99));

        finish_frame();
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        profile_function!();

        // Render frame
        let mut frame = self.renderable.begin_drawing();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb(30, 30, 40),
            |pass| {
                puffin::profile_scope!("ui_render");
                self.ui.render(pass.wgpu_pass());
            },
        );

        frame.finish();
    }
}

fn main() {
    run_app(|ctx| {
        // Setup...
        Box::new(ProfilingDemo {
            frame_times: VecDeque::new(),
            ui: UiSystem::new(graphics.clone()),
        })
    });
}
```

**Access profiler:** `http://127.0.0.1:8585`

## Troubleshooting Performance Issues

### Slow Frame Rate

**Symptoms:** FPS drops below 60

**Diagnosis:**
1. Check Puffin flamegraph for wide bars
2. Measure update vs render time
3. Check GPU timestamp queries

**Solutions:**
- If update is slow: Profile CPU, reduce allocations
- If render is slow: Profile GPU, reduce draw calls
- If both: Reduce workload or lower target FPS

### Stuttering/Jank

**Symptoms:** Occasional frame spikes

**Diagnosis:**
1. Look at 99th percentile frame time
2. Check for allocations in hot paths
3. Monitor garbage collection (if using GC)

**Solutions:**
- Preallocate buffers
- Use object pools
- Spread expensive work across frames

### High Memory Usage

**Symptoms:** Memory growth over time

**Diagnosis:**
1. Track allocations with TrackingAllocator
2. Check Drop implementations
3. Look for Arc reference cycles

**Solutions:**
- Implement Drop for cleanup
- Use Weak references to break cycles
- Release texture handles when unused

## Best Practices

### ✅ DO: Profile Before Optimizing

```rust
// Always profile first
init_profiling(ProfilingBackend::PuffinHttp);
profile_function!();
```

### ✅ DO: Measure Impact

```rust
// Measure before and after
let before = Instant::now();
optimized_function();
let after = before.elapsed();
println!("Improvement: {:.2}ms", after.as_secs_f32() * 1000.0);
```

### ✅ DO: Use Release Builds

```bash
# Always profile release builds
cargo build --release
cargo run --release --example my_game
```

### ❌ DON'T: Optimize Prematurely

```rust
// BAD: Optimizing before profiling
// Don't guess what's slow!

// GOOD: Profile, identify bottleneck, optimize
profile_function!();
// Then optimize the actual slow parts
```

### ❌ DON'T: Ignore 99th Percentile

```rust
// BAD: Only looking at average
println!("Avg: {:.2}ms", avg);

// GOOD: Track worst-case performance
println!("Avg: {:.2}ms, P99: {:.2}ms", avg, p99);
```

## Comparison to Unity

| Unity Profiler | Astrelis | Notes |
|----------------|----------|-------|
| CPU Timeline | Puffin Flamegraph | Similar visualization |
| GPU Profiler | Timestamp Queries | Manual implementation |
| Memory Profiler | TrackingAllocator | Custom tracking needed |
| Frame Debugger | Manual logging | No built-in frame capture |

## Next Steps

- **Practice:** Profile your application with Puffin
- **Optimize:** Apply techniques from this guide
- **Measure:** Verify improvements with benchmarks
- **Examples:** `performance_benchmark`, `profiling_demo`

## See Also

- [UI Performance Optimization](../ui/performance-optimization.md) - UI-specific optimizations
- [GPU Instancing](../rendering/gpu-instancing.md) - Rendering optimization
- API Reference: [`init_profiling()`](../../api/astrelis-core/profiling/fn.init_profiling.html)
