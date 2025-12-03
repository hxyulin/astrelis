# Performance

This document covers performance considerations, optimization strategies, and profiling techniques used throughout the Astrelis engine.

## Performance Philosophy

Astrelis prioritizes performance through:
- **Zero-cost abstractions** where possible
- **Incremental updates** to minimize redundant work
- **Batching** to reduce API overhead
- **Caching** to avoid recomputation
- **Profiling** to identify bottlenecks

## Profiling Infrastructure

### Puffin Integration

The engine uses puffin for frame-based profiling:

```rust
use astrelis_core::profiling::{init_profiling, ProfilingBackend, new_frame, profile_function};

// Initialize once at startup
init_profiling(ProfilingBackend::PuffinHttp);

// Main loop
loop {
    new_frame(); // Mark frame boundary
    
    {
        profile_function!();
        game_update();
    }
    
    {
        profile_function!();
        game_render();
    }
}
```

### Viewing Profiles

1. Start application with profiling enabled
2. Run `puffin_viewer` from terminal
3. Navigate to `http://127.0.0.1:8585`
4. View hierarchical scopes, flamegraphs, frame times

### Instrumented Areas

Current profiling coverage:
- Text rendering (measurement, layout, draw, atlas updates)
- UI system (layout computation, rendering, event handling, hit testing)
- Rendering pipeline (frame acquisition, command encoding, submission)
- Collection operations (SparseSet, in debug builds)

## Optimization Techniques

### 1. Incremental UI Updates

**Problem**: Full UI rebuild every frame is expensive (O(n) operations).

**Solution**: Dirty tracking with cached measurements.

```rust
// Bad: Full rebuild
loop {
    ui.build(|root| {
        root.text(format!("FPS: {}", fps));  // O(n) every frame
    });
}

// Good: Incremental update
let fps_id = WidgetId::new("fps");
loop {
    ui.update_text(fps_id, format!("FPS: {}", fps));  // O(1) update
}
```

**Impact**: 10-100x faster for small updates.

### 2. Text Measurement Caching

**Problem**: Text layout is expensive (~0.5-2ms per text node).

**Solution**: Cache measurements per node, invalidate on changes.

```rust
pub struct UiNode {
    pub text_measurement: Option<(f32, f32)>,
    pub dirty: bool,
    // ...
}

// Only remeasure if dirty
if !node.dirty {
    if let Some(cached) = node.text_measurement {
        return cached;
    }
}
```

**Impact**: ~90% reduction in layout time after initial frame.

### 3. Render Batching

**Problem**: Each draw call has ~0.1ms overhead.

**Solution**: Batch geometry by texture/material.

```rust
// Bad: One draw call per quad
for quad in quads {
    pass.draw(quad.vertices, quad.indices);
}

// Good: Single draw call for all quads
let batched_vertices = quads.flat_map(|q| q.vertices).collect();
pass.draw(batched_vertices, batched_indices);
```

**Impact**: Reduced from 100+ draw calls to 5-20 per frame.

### 4. Optimized Collections

**Problem**: std::HashMap uses cryptographic hash (slow).

**Solution**: Use AHash for non-cryptographic cases.

```rust
use astrelis_core::alloc::HashMap;  // AHash-based

let mut map = HashMap::new();  // 2-3x faster than std
```

**When to use**: Entity lookups, component maps, resource caches.

### 5. Generational Indices

**Problem**: Direct pointers/references unsafe with dynamic allocation.

**Solution**: SparseSet with generational indices.

```rust
use astrelis_core::alloc::{SparseSet, IndexSlot};

let mut entities = SparseSet::new();
let entity = entities.push(data);  // O(1) insert
let data = entities.get(entity);    // O(1) access with generation check
entities.remove(entity);            // O(1) remove
```

**Benefits**:
- O(1) operations
- Safe handle invalidation
- Cache-friendly iteration
- No reference counting overhead

### 6. Buffer Reuse

**Problem**: Allocating new buffers every frame is wasteful.

**Solution**: Reuse and resize buffers.

```rust
pub struct VertexBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl VertexBuffer {
    pub fn write(&mut self, data: &[Vertex]) {
        if data.len() > self.capacity {
            // Grow buffer
            self.buffer = create_buffer(data.len() * 2);
            self.capacity = data.len() * 2;
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }
}
```

### 7. Lazy Layout Computation

**Problem**: Computing layout for entire tree every frame is expensive.

**Solution**: Only recompute dirty subtrees.

```rust
impl UiTree {
    pub fn compute_layout(&mut self, viewport: Vec2) {
        if self.dirty_nodes.is_empty() {
            return;  // No work needed
        }
        
        // Only compute for dirty subtrees
        for node_id in &self.dirty_nodes {
            self.compute_layout_recursive(*node_id);
        }
        
        self.dirty_nodes.clear();
    }
}
```

## Performance Characteristics

### UI System

| Operation | Cold (First Time) | Warm (Cached) | Notes |
|-----------|------------------|---------------|-------|
| Full build | 5-10ms (1000 widgets) | N/A | O(n) allocations |
| Incremental update | 0.1-1ms | 0.05-0.5ms | O(m) where m = dirty |
| Layout computation | 2-5ms | 0.1-1ms | Only dirty subtrees |
| Rendering | 1-3ms | 1-3ms | Batched draw calls |
| Hit testing | 0.1-0.5ms | 0.1-0.5ms | O(n) tree traversal |

### Text Rendering

| Operation | Cold | Warm | Notes |
|-----------|------|------|-------|
| Text preparation | 1-2ms | 0.1ms | Layout required on cold |
| Glyph rasterization | 0.5ms per glyph | 0.01ms | Atlas lookup on warm |
| Text measurement | 0.5ms | 0.05ms | Cached per node |
| Batch rendering | 0.5-2ms | 0.5-2ms | Single draw call |

### Collections

| Operation | std::HashMap | AHashMap | SparseSet | Notes |
|-----------|--------------|----------|-----------|-------|
| Insert | ~50ns | ~20ns | ~15ns | AHash 2-3x faster |
| Lookup | ~40ns | ~15ns | ~10ns | SparseSet fastest |
| Remove | ~60ns | ~25ns | ~12ns | Direct indexing wins |
| Iteration | O(capacity) | O(capacity) | O(capacity) | All cache-friendly |

### Memory Usage

| Component | Size | Scaling | Notes |
|-----------|------|---------|-------|
| GraphicsContext | ~1KB | Static | Leaked, one per app |
| FontSystem | ~50MB | Static | System fonts loaded |
| Text atlas | 4MB initial | Grows to 16MB+ | Doubles when full |
| Per UiNode | ~200 bytes | Linear | Node + widget + taffy |
| Per TextBuffer | 1-5KB | Per text | Layout data |
| Vertex buffer | Dynamic | Per frame | Resized as needed |

## Benchmarking

### Microbenchmarks

Use criterion for hot path benchmarks:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_sparse_set(c: &mut Criterion) {
    c.bench_function("sparse_set_insert", |b| {
        let mut set = SparseSet::new();
        b.iter(|| {
            black_box(set.push(42));
        });
    });
}

criterion_group!(benches, bench_sparse_set);
criterion_main!(benches);
```

### Frame Time Budgets

Target 60 FPS = 16.67ms per frame:

```
┌──────────────────────────────────────┐
│ Total Frame Budget: 16.67ms          │
├──────────────────────────────────────┤
│ Update logic:        2-5ms           │
│ UI layout:           0.5-2ms         │
│ Text preparation:    0.5-1ms         │
│ Rendering:           2-5ms           │
│ GPU execution:       3-8ms           │
│ Present/VSync:       Variable        │
└──────────────────────────────────────┘
```

For 144 FPS, budget drops to 6.94ms - requires aggressive optimization.

## Common Bottlenecks

### 1. Text Layout

**Symptom**: High CPU usage during UI updates.

**Diagnosis**: Profile shows time in `cosmic_text::layout`.

**Solutions**:
- Cache text buffers
- Limit max text length
- Use simpler fonts (fewer glyphs)
- Reduce unique font sizes

### 2. Layout Thrashing

**Symptom**: Layout computed multiple times per frame.

**Diagnosis**: `compute_layout` appears multiple times in profiler.

**Solutions**:
- Batch layout updates
- Mark dirty only when needed
- Avoid reading layout during construction

### 3. Draw Call Overhead

**Symptom**: High GPU idle time, low GPU utilization.

**Diagnosis**: Many small draw calls in frame capture.

**Solutions**:
- Batch geometry by texture
- Use instancing for repeated objects
- Combine atlases to reduce texture switches

### 4. Allocation Churn

**Symptom**: Time spent in allocator, memory usage fluctuates.

**Diagnosis**: Profiler shows frequent malloc/free calls.

**Solutions**:
- Reuse buffers and vectors
- Use arena allocators for temporary data
- Pre-allocate with capacity hints

### 5. Hash Map Lookups

**Symptom**: Time in hash function or collision resolution.

**Diagnosis**: Profile shows time in `hash` or `eq` functions.

**Solutions**:
- Switch to AHash
- Use FxHash for integer keys
- Consider SparseSet for generational handles

## Optimization Workflow

### 1. Measure

Always profile before optimizing:

```rust
{
    profile_scope!("suspect_function");
    suspect_function();
}
```

Run with puffin_viewer to identify hotspots.

### 2. Hypothesize

Form theory about bottleneck:
- Algorithmic complexity?
- Cache misses?
- API overhead?
- Redundant work?

### 3. Optimize

Implement targeted optimization:
- Algorithmic improvements first
- Then data structure changes
- Finally micro-optimizations

### 4. Verify

Measure again to confirm improvement:
- Check profiler for reduced time
- Verify no regressions elsewhere
- Document the change

## Platform-Specific Considerations

### Windows

- DX12 has lower driver overhead than DX11
- Prefer sequential memory access (cache-friendly)
- GPU scheduler improves multi-queue performance

### macOS

- Metal has excellent performance characteristics
- Unified memory reduces copies on M-series chips
- Prefer native types (simd for M-series)

### Linux

- Vulkan performance varies by driver
- Mesa drivers generally excellent
- Consider wayland vs X11 overhead

### Web (WASM)

- Single-threaded (no parallel work)
- Startup time critical (minimize bundle size)
- WebGPU still maturing (fallbacks needed)

### Mobile

- Battery life critical (reduce GPU usage)
- Thermal throttling (sustained performance matters)
- Memory constrained (aggressive caching limits)

## Future Optimizations

Planned performance improvements:

1. **Spatial indexing** - Quad-tree for O(log n) hit testing
2. **Multi-threading** - Parallel layout computation
3. **Compute shaders** - GPU-accelerated UI rendering
4. **Instancing** - Reduce draw calls for repeated geometry
5. **SmallVec/SmolStr** - Stack allocation for small collections
6. **SDF text** - Resolution-independent text rendering
7. **Culling** - Skip rendering for off-screen widgets
8. **LOD** - Lower detail for distant/small objects
9. **Streaming** - Load assets on demand
10. **Job system** - Task-based parallelism for ECS

## Performance Guidelines

### DO

- Profile before optimizing
- Cache expensive computations
- Batch API calls
- Reuse allocations
- Use appropriate data structures
- Measure impact of changes

### DON'T

- Optimize without profiling
- Assume bottlenecks without data
- Sacrifice correctness for speed (until proven necessary)
- Micro-optimize before fixing algorithms
- Ignore platform differences
- Forget about memory usage

## Resources

- **Puffin**: https://github.com/EmbarkStudios/puffin
- **WGPU Performance**: https://github.com/gfx-rs/wgpu/wiki/Performance
- **Rust Performance Book**: https://nnethercote.github.io/perf-book/
- **Taffy Performance**: https://github.com/DioxusLabs/taffy/blob/main/PERFORMANCE.md