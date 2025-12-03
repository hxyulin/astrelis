# Astrelis Engine Benchmarks

Comprehensive benchmarking suite for the Astrelis game engine covering core data structures, text rendering, and UI systems.

## Overview

The benchmark suite uses [Criterion.rs](https://github.com/bheisler/criterion.rs) for accurate, statistical benchmarking with:
- HTML reports with charts
- Statistical analysis (mean, std dev, outliers)
- Regression detection
- Comparison between runs

## Running Benchmarks

### Run All Benchmarks

```bash
cargo bench
```

### Run Specific Crate Benchmarks

```bash
# Core data structures
cargo bench --package astrelis-core

# Text rendering
cargo bench --package astrelis-text

# UI system
cargo bench --package astrelis-ui
```

### Run Specific Benchmark

```bash
# Specific benchmark file
cargo bench --package astrelis-core --bench collections

# Specific benchmark function
cargo bench --package astrelis-ui "layout_compute_simple"

# Benchmark with filter
cargo bench "sparse_set"
```

### Baseline Comparison

```bash
# Save baseline
cargo bench -- --save-baseline main

# Compare against baseline
cargo bench -- --baseline main
```

## Benchmark Categories

### Core Benchmarks (`astrelis-core`)

#### Collections (`collections.rs`)
- **HashMap operations**: Insert, lookup, iteration
- **HashSet operations**: Insert, contains, iteration
- **String key performance**: Real-world entity/component lookups
- **Mixed workload**: ECS simulation scenarios

**Key metrics**:
- AHash vs std HashMap: 2-3x faster
- Integer keys: ~20ns insert, ~15ns lookup (AHash)
- String keys: ~40ns insert, ~30ns lookup (AHash)

#### SparseSet (`sparse_set.rs`)
- **CRUD operations**: Insert, access, remove
- **Iteration**: Dense iteration with holes
- **Slot reuse**: Free list performance
- **Generation checks**: Validation overhead
- **Entity simulation**: Realistic game scenarios

**Key metrics**:
- Insert: ~15ns
- Lookup: ~10ns (with generation check)
- Iteration: Cache-friendly, skips freed slots
- Entity simulation: 500 entities, 100 frames = ~2ms

### Text Rendering Benchmarks (`astrelis-text`)

#### Text Rendering (`text_rendering.rs`)
- **Text preparation**: Buffer creation and layout
- **Font sizes**: Performance across 8px-96px
- **Text styles**: Plain, bold, italic, colored
- **Alignment**: Left, center, right
- **Wrapping**: None vs word-wrap
- **Multiple texts**: Batch operations (10-500 texts)
- **UI scenarios**: Realistic menu/HUD layouts
- **Buffer reuse**: Impact of caching

**Key metrics**:
- Short text prepare: ~0.5-1ms (cold), ~0.1ms (warm)
- Text measurement: ~0.5ms
- Wrapped text: ~1-2ms
- UI frame (10 texts): ~5-8ms

#### Text Measurement (`text_measurement.rs`)
- **Basic measurement**: Single char to long text
- **Font sizes**: 8px-96px measurement cost
- **Wrapping constraints**: Width-constrained text
- **Styles**: Impact of bold/italic
- **Batch measurement**: 10-500 texts
- **Multiline**: Performance vs line count
- **Unicode**: ASCII, emoji, CJK, Arabic, mixed
- **UI scenarios**: Layout measurement simulation

**Key metrics**:
- Basic measurement: ~0.3-0.5ms
- With wrapping: ~0.5-1.5ms (depends on width)
- Batch (100 texts): ~40-50ms
- Unicode impact: Minimal (<10% overhead)

### UI System Benchmarks (`astrelis-ui`)

#### UI Tree (`ui_tree.rs`)
- **Tree building**: Simple, nested, complex structures
- **Node lookup**: Widget ID resolution
- **Tree traversal**: Iteration performance
- **Widget creation**: Text, button, container, input
- **Tree modification**: Adding/removing children
- **Widget styles**: Plain vs styled elements
- **Memory usage**: Large tree allocation
- **Realistic scenarios**: Dashboard, menu, form UIs

**Key metrics**:
- Simple tree (100 widgets): ~2-3ms
- Nested tree (depth 10): ~1-2ms
- Complex menu UI: ~5-8ms
- Dashboard (50 widgets): ~8-12ms

#### UI Layout (`ui_layout.rs`)
- **Layout computation**: Simple to complex layouts
- **Flexbox**: Row, column, nested flex
- **Sizing modes**: Fixed, auto, percentage, flex-grow
- **Constraints**: Min/max width/height
- **Spacing**: Padding, margin, gap
- **Deep nesting**: Performance vs depth (5-20 levels)
- **Text measurement**: Integration cost
- **Viewport sizes**: 640x480 to 4K
- **Realistic forms**: Login, settings forms
- **List scenarios**: Simple and complex lists (50-1000 items)

**Key metrics**:
- Simple layout (100 widgets): ~1-2ms
- Complex flexbox: ~2-4ms
- Deep nesting (depth 20): ~3-5ms
- Text measurement (100 texts): ~40-60ms
- Large list (1000 items): ~10-20ms

#### UI Incremental (`ui_incremental.rs`)
- **Full rebuild vs incremental**: Direct comparison
- **Counter updates**: Single widget update
- **Multiple updates**: 5-50 widgets
- **Partial updates**: 1 of 100 widgets
- **FPS counter**: Real-time HUD simulation
- **Dashboard updates**: Multiple metrics
- **Text input**: Character-by-character updates
- **Button labels**: State changes
- **Large UI small update**: Scalability testing
- **Layout recomputation**: Post-update cost

**Key metrics**:
- Full rebuild: ~5-10ms (100 widgets)
- Incremental update: ~0.1-0.5ms (1 widget)
- Speedup: **10-100x** for small changes
- FPS counter update: <0.1ms incremental vs ~5ms rebuild
- Dashboard (10 values): ~0.5ms incremental vs ~15ms rebuild

## Interpreting Results

### HTML Reports

Criterion generates detailed HTML reports in `target/criterion/`:
- Open `target/criterion/report/index.html` in browser
- View charts, distributions, regressions
- Compare multiple runs

### Statistical Significance

Criterion reports:
- **Mean**: Average execution time
- **Std Dev**: Variation in measurements
- **Median**: Middle value (less affected by outliers)
- **MAD**: Median Absolute Deviation
- **Outliers**: Measurements far from mean

### Regression Detection

Criterion automatically detects regressions:
- **Improved**: Green, faster than baseline
- **Regressed**: Red, slower than baseline
- **No change**: Performance within noise threshold

## Performance Targets

### Core
- HashMap insert: <50ns
- HashMap lookup: <30ns
- SparseSet insert: <20ns
- SparseSet access: <15ns

### Text
- Text prepare (short): <1ms
- Text measurement: <0.5ms
- Batch (100 texts): <50ms
- UI frame (10 texts): <10ms

### UI
- Simple tree build (100 widgets): <5ms
- Layout computation (100 widgets): <2ms
- Incremental update (1 widget): <0.5ms
- Large list layout (1000 items): <20ms

## Best Practices

### Running Benchmarks

1. **Close other applications**: Reduce noise
2. **Consistent power settings**: Disable CPU throttling
3. **Multiple runs**: Criterion runs 100+ iterations
4. **Baseline comparison**: Track performance over time
5. **Release mode**: Always benchmark with optimizations

### Writing Benchmarks

1. **Use `black_box()`**: Prevent compiler optimizations
2. **Setup outside timing**: Don't benchmark initialization
3. **Realistic scenarios**: Match real-world usage
4. **Multiple sizes**: Test scalability (10, 100, 1000)
5. **Throughput**: Set for per-element metrics

### Analyzing Results

1. **Check std dev**: High variance indicates noise
2. **Outliers**: Investigate causes (GC, OS interrupts)
3. **Compare baselines**: Track regressions
4. **Profile slowness**: Use puffin for detailed analysis
5. **Validate assumptions**: Ensure benchmarks match reality

## Integration with Profiling

Benchmarks complement puffin profiling:

### When to Use Benchmarks
- Comparing implementations (AHash vs std)
- Measuring specific operations (insert, lookup)
- Regression detection (CI/CD)
- Micro-optimizations (hot path tuning)

### When to Use Profiling
- Understanding frame time breakdown
- Finding bottlenecks (where time is spent)
- Analyzing call hierarchies
- Real-world scenario analysis

### Combined Workflow

1. **Profile**: Identify bottleneck (e.g., text layout)
2. **Benchmark**: Isolate and measure (text_measurement.rs)
3. **Optimize**: Implement improvement (caching)
4. **Benchmark**: Verify improvement (10x faster)
5. **Profile**: Confirm overall impact (frame time down)
6. **Baseline**: Save for regression detection

## Continuous Integration

### CI Benchmark Setup

```yaml
# .github/workflows/benchmarks.yml
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
          profile: minimal
          toolchain: stable
      
      # Fetch baseline
      - name: Fetch main baseline
        run: |
          git fetch origin main
          git checkout origin/main
          cargo bench -- --save-baseline main
          git checkout -
      
      # Run PR benchmarks
      - name: Run benchmarks
        run: cargo bench -- --baseline main
      
      # Upload results
      - name: Upload results
        uses: actions/upload-artifact@v2
        with:
          name: benchmark-results
          path: target/criterion
```

## Benchmark Results Archive

Store baseline results for historical comparison:

```bash
# Create baseline for version
cargo bench -- --save-baseline v0.1.0

# Compare new version
cargo bench -- --baseline v0.1.0

# List all baselines
ls target/criterion/*/base/
```

## Troubleshooting

### Inconsistent Results

**Symptoms**: High variance, different results each run

**Solutions**:
- Close background applications
- Disable turbo boost
- Run multiple times, take median
- Increase sample size: `cargo bench -- --sample-size 1000`

### Benchmark Too Fast

**Symptoms**: Time < 1μs, high overhead

**Solutions**:
- Batch operations: Test 100 iterations
- Increase workload size
- Use `iter_batched` for setup cost

### Benchmark Too Slow

**Symptoms**: Takes hours to complete

**Solutions**:
- Reduce sample size: `--sample-size 10`
- Reduce warmup: `--warm-up-time 1`
- Focus on specific benchmarks: Filter by name
- Use `--quick` mode for development

### Out of Memory

**Symptoms**: Benchmark crashes, OS kills process

**Solutions**:
- Reduce problem size (fewer widgets)
- Use `iter_batched` to cleanup between iterations
- Split into smaller benchmark groups
- Increase system memory

## Further Reading

- [Criterion.rs Book](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Benchmarking Best Practices](https://phoronix-test-suite.com/documentation/benchmarking.html)
- [Statistical Analysis](https://en.wikipedia.org/wiki/Bootstrapping_(statistics))