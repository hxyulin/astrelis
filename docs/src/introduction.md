# Introduction

Astrelis is a modular game engine written in Rust, designed with performance, flexibility, and modularity as core principles. The engine provides a comprehensive set of tools for building games and interactive applications, from low-level graphics rendering to high-level UI systems.

## Design Philosophy

### Modularity
Astrelis is structured as a collection of independent crates that can be used together or separately. Each crate focuses on a specific domain (rendering, UI, text, windowing) and has minimal dependencies on other engine components. This allows developers to:
- Use only the parts they need
- Replace or extend specific subsystems
- Integrate engine components into existing projects

### Performance
Performance is prioritized throughout the engine:
- Zero-cost abstractions where possible
- GPU-accelerated rendering via WGPU
- Optimized data structures (AHash collections, SparseSet)
- Profiling integration (puffin) for performance analysis
- Lazy/incremental updates in UI system
- Efficient memory layouts and minimal allocations

### Type Safety
Rust's type system is leveraged to provide:
- Compile-time error detection
- Memory safety without garbage collection
- Generational indices to prevent use-after-free bugs
- Clear ownership and borrowing semantics

## Architecture Overview

Astrelis is organized into several foundational layers:

1. **Core Layer** (`astrelis-core`)
   - Common math types and utilities
   - Profiling infrastructure
   - Optimized collections and allocators
   - Foundational data structures

2. **Platform Layer** (`astrelis-winit`)
   - Window creation and management
   - Event handling and dispatch
   - Platform abstraction

3. **Rendering Layer** (`astrelis-render`, `astrelis-text`)
   - Graphics context management
   - WGPU abstraction and resource management
   - Text rendering with cosmic-text
   - Frame and render pass management

4. **UI Layer** (`astrelis-ui`)
   - Declarative widget API
   - Flexbox/Grid layouts via Taffy
   - Event handling and interaction
   - GPU-accelerated rendering

5. **Game Systems** (`astrelis-ecs`, `astrelis-scene`, `astrelis-assets`, `astrelis-audio`)
   - Entity-component-system (planned)
   - Scene graph and management (planned)
   - Asset loading and management (planned)
   - Audio playback and mixing (planned)

## Current State

The engine is in active development. Core systems are functional:
- Graphics context and rendering pipeline
- Window management and event handling
- Text rendering with full styling support
- UI system with incremental updates and layout caching
- Profiling infrastructure

Several crates are placeholders for future development (ECS, assets, audio, scene management).

## Getting Started

To use Astrelis in your project, add the relevant crates to your `Cargo.toml`:

```toml
[dependencies]
astrelis-core = "0.0.1"
astrelis-render = "0.0.1"
astrelis-winit = "0.0.1"
astrelis-ui = "0.0.1"
astrelis-text = "0.0.1"
```

See the architecture documentation for detailed information about each subsystem.