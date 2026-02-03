# GPU Profiling

Astrelis includes built-in GPU profiling via `puffin` (CPU profiling) and `wgpu_profiler` (GPU timestamps). This guide covers setup, usage, and interpreting profiling results.

## Overview

Profiling helps identify performance bottlenecks in:
- **CPU work** (layout, draw list generation, event processing)
- **GPU work** (shader execution, draw calls, buffer uploads)
- **Frame synchronization** (CPU/GPU bubbles, stalls)

## Profiling Backends

### PuffinHttp (Recommended)

The `PuffinHttp` backend streams profiling data to a web viewer:

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend};

fn main() {
    // Initialize profiling with HTTP server
    init_profiling(ProfilingBackend::PuffinHttp);

    // Your application code...
}
```

**Access the profiler:**
1. Run your application
2. Open browser to `http://127.0.0.1:8585`
3. The puffin viewer shows live profiling data

### PuffinLocalFile

For offline analysis, write profiling data to a file:

```rust
init_profiling(ProfilingBackend::PuffinLocalFile("profile.puffin"));

// When done profiling:
astrelis_core::profiling::shutdown_profiling();
```

Then open `profile.puffin` in the standalone puffin viewer.

### None (Disable Profiling)

For production builds:

```rust
init_profiling(ProfilingBackend::None);
```

Or conditionally enable:

```rust
#[cfg(debug_assertions)]
init_profiling(ProfilingBackend::PuffinHttp);

#[cfg(not(debug_assertions))]
init_profiling(ProfilingBackend::None);
```

## Manual Profiling Scopes

### Function Profiling

Profile entire functions:

```rust
use astrelis_core::profiling::profile_function;

fn update_game_state() {
    profile_function!();  // Automatically uses function name

    // Function body...
}
```

### Scope Profiling

Profile specific code sections:

```rust
use astrelis_core::profiling::profile_scope;

fn render_frame() {
    {
        profile_scope!("Layout computation");
        // Layout code...
    }

    {
        profile_scope!("Draw list generation");
        // Draw list code...
    }

    {
        profile_scope!("GPU rendering");
        // Rendering code...
    }
}
```

### Conditional Profiling

Only profile when enabled:

```rust
#[cfg(feature = "profiling")]
use astrelis_core::profiling::profile_scope;

fn expensive_operation() {
    #[cfg(feature = "profiling")]
    profile_scope!("Expensive operation");

    // Work...
}
```

## GPU Profiling

GPU profiling requires the `gpu-profiling` feature and is integrated into `FrameContext`:

```toml
[dependencies]
astrelis-render = { version = "0.1", features = ["gpu-profiling"] }
```

### Automatic GPU Profiling

GPU timestamps are automatically collected per render pass:

```rust
let mut frame = renderable_window.begin_drawing();

frame.clear_and_render(
    RenderTarget::Surface,
    Color::BLACK,
    |pass| {
        // This render pass is automatically profiled
        ui.render(pass.wgpu_pass());
    },
);

frame.finish();  // GPU timestamps are resolved here
```

### Manual GPU Scopes

For finer-grained GPU profiling:

```rust
#[cfg(feature = "gpu-profiling")]
{
    let mut pass = frame.begin_render_pass(/* ... */);

    {
        let _scope = frame.gpu_scope("UI rendering");
        ui.render(pass.wgpu_pass());
    }

    {
        let _scope = frame.gpu_scope("Post-processing");
        post_process.render(pass.wgpu_pass());
    }
}
```

## Viewing Profiling Results

### Puffin Viewer Interface

The web viewer at `http://127.0.0.1:8585` shows:

1. **Flame Graph** - Hierarchical view of profiling scopes
2. **Timeline** - Chronological view of events
3. **Statistics** - Min/max/average times per scope
4. **Frame Selector** - Scrub through captured frames

### Key Metrics

Look for:
- **Frame time** - Total time per frame (target: <16.67ms for 60fps)
- **CPU time** - Time spent in application code
- **GPU time** - Time spent in shader execution
- **Bubbles** - Gaps indicating CPU/GPU synchronization issues

## Interpreting Results

### High CPU Time

**Symptoms:**
- `update()` or `layout()` scopes take >10ms
- Frame time dominated by CPU work

**Solutions:**
- Use dirty flags to skip unnecessary updates
- Profile and optimize hot code paths
- Use incremental updates (`update_text`, `update_color`)
- Parallelize independent work with task pool

### High GPU Time

**Symptoms:**
- Render pass scopes take >10ms
- Many draw calls or complex shaders

**Solutions:**
- Use GPU instancing (already enabled for UI/geometry)
- Reduce overdraw (cull offscreen widgets)
- Simplify shaders or reduce texture sampling
- Use texture atlases to reduce bind group changes

### CPU/GPU Bubbles

**Symptoms:**
- Gaps in timeline between CPU and GPU work
- Total frame time > CPU time + GPU time

**Solutions:**
- Overlap CPU and GPU work (start next frame's CPU work while GPU renders)
- Reduce synchronization points (avoid `queue.submit()` mid-frame)
- Use double buffering for uniform buffers

### Layout Thrashing

**Symptoms:**
- `layout()` called multiple times per frame
- Cascading dirty flag propagation

**Solutions:**
- Batch layout updates
- Use constraint caching
- Avoid circular dependencies (parent size depends on child, child on parent)

## Common Profiling Patterns

### Baseline Profiling

Capture a baseline before optimization:

```rust
// Run for 1000 frames and save profile
init_profiling(ProfilingBackend::PuffinLocalFile("baseline.puffin"));

for _ in 0..1000 {
    app.update();
    app.render();
}

astrelis_core::profiling::shutdown_profiling();
```

### A/B Testing

Compare two implementations:

```rust
// Version A
{
    profile_scope!("Algorithm A");
    algorithm_a();
}

// Version B
{
    profile_scope!("Algorithm B");
    algorithm_b();
}
```

Then compare timings in the viewer.

### Regression Testing

Monitor performance over time:

```bash
# Capture nightly profiles
cargo run --release --features profiling > profile_$(date +%Y%m%d).puffin
```

## Example: Profiling a UI Update

```rust
use astrelis_core::profiling::{profile_function, profile_scope};

fn update_ui(&mut self, time: &FrameTime) {
    profile_function!();

    {
        profile_scope!("Event processing");
        self.process_events();
    }

    {
        profile_scope!("Animation update");
        self.update_animations(time.delta_seconds());
    }

    {
        profile_scope!("Layout computation");
        self.ui.compute_layout();
    }

    {
        profile_scope!("Draw list generation");
        self.ui.generate_draw_list();
    }
}
```

Run the app and view results:
- Open `http://127.0.0.1:8585`
- Navigate to "update_ui" in the flame graph
- Drill down into child scopes
- Identify slowest operations

## Advanced: Custom Profiling Metrics

Extend profiling with custom metrics:

```rust
use astrelis_core::profiling::profile_scope;

fn render_complex_scene(&mut self) {
    profile_scope!("Render scene");

    let widget_count = self.ui.widget_count();
    let draw_calls = self.ui.draw_call_count();

    tracing::debug!(
        "Rendered {} widgets in {} draw calls",
        widget_count,
        draw_calls
    );
}
```

View these logs in the puffin viewer's "Messages" tab.

## Performance Targets

Typical performance budgets for 60fps (16.67ms):

| Operation              | Budget | Notes                                |
|------------------------|--------|--------------------------------------|
| Event processing       | 1ms    | Batched, should be very fast         |
| Layout computation     | 2-3ms  | Only dirty subtrees                  |
| Draw list generation   | 2-3ms  | Incremental updates help             |
| GPU rendering          | 8-10ms | Includes all render passes           |
| CPU overhead (other)   | 2-3ms  | App logic, profiling, etc.           |

**Total: ~16ms** (leaves headroom for frame variability)

## Profiling Demo Example

See the `profiling_demo.rs` example:

```bash
cargo run -p astrelis-render --example profiling_demo --features gpu-profiling
```

This demonstrates:
- Automatic profiling setup
- Manual profiling scopes
- GPU timestamp collection
- Real-time viewer integration

## Next Steps

- See [Performance Optimization](../ui/performance-optimization.md) for optimization strategies
- Explore [Batched Rendering](./batched-rendering.md) for GPU performance
- Check [Custom Shaders](./custom-shaders.md) for shader optimization techniques
