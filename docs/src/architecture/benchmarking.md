# Benchmarking and Performance Analysis

Astrelis provides comprehensive benchmarking and profiling tools to measure and optimize performance. This document covers the benchmarking infrastructure, how to use it, and how to interpret results.

## Overview

The engine uses two complementary approaches:

1. **Criterion Benchmarks** - Microbenchmarks for isolated operations
2. **Puffin Profiling** - Frame-based profiling for real-world scenarios

## Criterion Benchmarks

### Running Benchmarks

Run all benchmarks:
```bash
cargo bench
```

Run specific crate:
```bash
cargo bench --package astrelis-core
cargo bench --package astrelis-text
cargo bench --package astrelis-ui
```

Run specific benchmark:
```bash
cargo bench --bench collections
cargo bench "sparse_set_insert"
```

### Benchmark Coverage

#### Core Data Structures
- **HashMap/HashSet**: AHash vs std comparison
- **SparseSet**: Insert, access, remove, iteration
- **Mixed workloads**: ECS simulation scenarios

#### Text Rendering
- **Text preparation**: Buffer creation and layout
- **Text measurement**: Size calculation performance
- **Font sizes**: 8px to 96px range
- **Styles**: Plain, bold, italic combinations
- **Wrapping**: Word-wrap performance
- **Batch operations**: Multiple text rendering

#### UI System
- **Tree building**: Widget creation and hierarchy
- **Layout computation**: Flexbox and grid layouts
- **Incremental updates**: Full rebuild vs partial update
- **Realistic scenarios**: Dashboard, forms, lists

### Results Location

HTML reports generated in `target/criterion/`:
```
target/criterion/
├── report/
│   └── index.html          # Main report
├── collections/
│   └── hashmap_insert/
│       ├── report/
│       └── base/           # Baseline data
└── sparse_set/
    └── report/
```

Open `target/criterion/report/index.html` in browser for interactive charts.

### Baseline Comparison

Save current performance as baseline:
```bash
cargo bench -- --save-baseline main
```

Compare against baseline:
```bash
# Make changes
cargo bench -- --baseline main
```

Criterion automatically detects regressions:
- **Green**: Improved (faster)
- **Red**: Regressed (slower)
- **Gray**: No significant change

### Interpreting Results

Criterion reports statistical metrics:

```
hashmap_insert/100      time:   [1.98 µs 2.01 µs 2.04 µs]
                        change: [-5.2% -2.1% +0.8%] (p = 0.19 > 0.05)
                        No change in performance detected.
Found 3 outliers among 100 measurements (3%)
  2 (2%) high mild
  1 (1%) high severe
```

- **Time range**: [lower bound, mean, upper bound]
- **Change**: Performance delta vs baseline with confidence interval
- **p-value**: Statistical significance (< 0.05 = significant)
- **Outliers**: Measurements affected by noise

### Performance Targets

Expected performance ranges:

**Core Collections**:
- HashMap insert: < 50ns
- HashMap lookup: < 30ns
- SparseSet insert: < 20ns
- SparseSet access: < 15ns

**Text Rendering**:
- Short text prepare: < 1ms
- Text measurement: < 0.5ms
- Batch 100 texts: < 50ms

**UI System**:
- Simple tree build (100 widgets): < 5ms
- Layout compute (100 widgets): < 2ms
- Incremental update (1 widget): < 0.5ms
- Large list (1000 items): < 20ms

## Puffin Profiling

### Setup

Initialize profiling at application start:

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame};

fn main() {
    // Start profiling server
    init_profiling(ProfilingBackend::PuffinHttp);
    
    // Main loop
    loop {
        new_frame(); // Mark frame boundary
        
        // Your game logic
        update();
        render();
    }
}
```

### Instrumenting Code

Add scopes to measure:

```rust
use astrelis_core::profiling::{profile_function, profile_scope};

fn expensive_function() {
    profile_function!(); // Automatic scope for entire function
    
    // Function body
}

fn complex_function() {
    profile_function!();
    
    {
        profile_scope!("physics_step");
        physics.update();
    }
    
    {
        profile_scope!("ai_update");
        ai.update();
    }
}
```

### Viewing Profiles

1. Start application with profiling enabled
2. Install puffin viewer: `cargo install puffin_viewer`
3. Run: `puffin_viewer`
4. Navigate to `http://127.0.0.1:8585` in viewer
5. View frame timings and hierarchical scopes

### Profiler Features

- **Flamegraph**: Hierarchical time visualization
- **Frame timeline**: Frame-by-frame comparison
- **Scope details**: Min/max/average times
- **Zooming**: Drill into specific frames/scopes
- **Filtering**: Show/hide specific scopes

### Current Coverage

Performance-critical areas instrumented:

**Text Rendering**:
- `text_measurement` - Measuring text size
- `text_layout` - Layout computation
- `text_draw` - Drawing preparation
- `atlas_update` - Texture atlas uploads

**UI System**:
- `ui_build` - Tree construction
- `ui_layout` - Layout computation
- `ui_render` - Rendering
- `ui_event` - Event handling
- `ui_hit_test` - Hit testing

**Rendering**:
- `frame_acquire` - Swapchain frame acquisition
- `command_encode` - Command buffer recording
- `queue_submit` - GPU submission

## Benchmarks vs Profiling

### When to Use Benchmarks

Use Criterion for:
- **Comparing alternatives**: AHash vs std HashMap
- **Isolated operations**: Single function performance
- **Regression detection**: CI/CD integration
- **Micro-optimizations**: Hot path tuning
- **Statistical analysis**: Variance, outliers

### When to Use Profiling

Use Puffin for:
- **Frame time breakdown**: Where time is spent
- **Finding bottlenecks**: Slowest operations
- **Call hierarchies**: Function relationships
- **Real-world scenarios**: Actual game performance
- **Interactive analysis**: Zoom, filter, compare frames

### Combined Workflow

Effective optimization workflow:

1. **Profile application**
   - Run with puffin enabled
   - Identify bottleneck (e.g., UI layout takes 10ms)

2. **Write benchmark**
   - Create isolated test for bottleneck
   - Measure baseline performance

3. **Optimize**
   - Implement improvements (caching, better algorithm)
   - Test with benchmark

4. **Verify improvement**
   - Benchmark shows 10x speedup
   - Profile shows frame time reduced

5. **Save baseline**
   - `cargo bench -- --save-baseline optimized`
   - Prevent future regressions

## Best Practices

### Benchmarking

**DO**:
- Close other applications during benchmarking
- Run on consistent hardware (no throttling)
- Use release builds (`cargo bench` does this)
- Set baselines for important changes
- Test multiple sizes (10, 100, 1000)
- Use `black_box()` to prevent optimizations

**DON'T**:
- Benchmark in debug mode
- Include setup/teardown in measurement
- Test unrealistic scenarios
- Ignore high variance (indicates noise)
- Optimize without measuring first

### Profiling

**DO**:
- Profile realistic scenarios (actual gameplay)
- Mark frame boundaries (`new_frame()`)
- Use descriptive scope names
- Profile multiple frames for patterns
- Compare before/after optimization

**DON'T**:
- Profile with debug symbols stripped
- Add too many fine-grained scopes (overhead)
- Profile only one frame (outliers)
- Forget to disable profiling in production

## Example: Optimizing Text Rendering

Step-by-step optimization example:

### 1. Identify Problem

Run with profiling:
```rust
init_profiling(ProfilingBackend::PuffinHttp);
// ... run game
```

Puffin shows `text_measurement` taking 50ms per frame.

### 2. Create Benchmark

```rust
// benches/text_measurement.rs
fn bench_text_measurement(c: &mut Criterion) {
    let mut renderer = setup();
    c.bench_function("measure_ui_text", |b| {
        let text = Text::new("Sample").size(16.0);
        b.iter(|| renderer.measure_text(&text));
    });
}
```

Run baseline:
```bash
cargo bench measure_ui_text -- --save-baseline before
```

Result: 0.5ms per measurement, but called 100 times per frame.

### 3. Implement Caching

```rust
pub struct UiNode {
    pub text_measurement: Option<(f32, f32)>,
    pub dirty: bool,
}

impl UiTree {
    pub fn measure_text(&mut self, node_id: NodeId) -> (f32, f32) {
        let node = &self.nodes[&node_id];
        
        // Return cached if clean
        if !node.dirty {
            if let Some(cached) = node.text_measurement {
                return cached;
            }
        }
        
        // Measure and cache
        let size = measure_expensive(node);
        self.nodes.get_mut(&node_id).unwrap().text_measurement = Some(size);
        size
    }
}
```

### 4. Verify with Benchmark

```bash
cargo bench measure_ui_text -- --baseline before
```

Result: Now 0.05ms when cached (10x faster).

### 5. Confirm with Profiling

Run with profiling again. Puffin shows:
- `text_measurement`: 5ms (was 50ms)
- Frame time: 12ms (was 60ms)

### 6. Save Baseline

```bash
cargo bench -- --save-baseline with-caching
```

Future changes will compare against this baseline.

## Continuous Integration

### GitHub Actions Example

```yaml
name: Benchmarks
on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Fetch baseline
        run: |
          git fetch origin main
          git checkout origin/main
          cargo bench -- --save-baseline main
          git checkout -
      
      - name: Run benchmarks
        run: cargo bench -- --baseline main
      
      - name: Check for regressions
        run: |
          # Parse criterion output for regressions
          # Fail if performance degraded > 10%
```

## Troubleshooting

### High Variance in Benchmarks

**Problem**: Results vary wildly between runs

**Solutions**:
- Close background applications
- Disable CPU frequency scaling
- Increase sample size: `cargo bench -- --sample-size 1000`
- Run on isolated hardware

### Profiling Shows No Data

**Problem**: Puffin viewer shows empty frames

**Solutions**:
- Ensure `init_profiling()` called
- Check `new_frame()` called each frame
- Verify `puffin::set_scopes_on(true)` (done by `init_profiling`)
- Check network: `http://127.0.0.1:8585` accessible

### Benchmark Crashes

**Problem**: Benchmark fails with OOM or panic

**Solutions**:
- Reduce problem size (fewer widgets)
- Use `iter_batched` for cleanup
- Check for resource leaks
- Increase system memory

## Further Resources

- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Puffin Documentation](https://github.com/EmbarkStudios/puffin)
- [Benchmarking Methodology](https://www.brendangregg.com/methodology.html)
- Benchmark README: `benches/README.md`
- Performance docs: `architecture/performance.md`
