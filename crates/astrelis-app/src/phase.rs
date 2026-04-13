//! Execution phases that define the fixed order of per-frame processing.

/// Execution phases for systems.
///
/// Phases run in declaration order every frame (except [`Phase::Startup`],
/// which runs exactly once after all plugins have been registered).
///
/// ```text
/// Startup          (once)
///     ↓
/// ┌─ PreUpdate     (input, assets, event buffer swap)
/// │  FixedUpdate   (0..N times at fixed rate)
/// │  Update        (main game logic, variable dt)
/// │  PostUpdate    (cleanup, state transitions)
/// │  Render        (draw commands)
/// │  Present       (surface present, profiler frame mark)
/// └─ loop
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Runs once after all plugins are registered, before the first frame.
    Startup,
    /// Runs at the start of each frame. Built-in plugins use this for
    /// input polling, asset server updates, and event buffer swaps.
    PreUpdate,
    /// Runs 0..N times per frame at a fixed tick rate. Use for
    /// physics and gameplay logic that needs deterministic timesteps.
    FixedUpdate,
    /// Main game logic, running once per frame with variable delta time.
    Update,
    /// Runs after `Update`. Use for cleanup, state transitions, or
    /// anything that must happen after main logic but before rendering.
    PostUpdate,
    /// Draw commands and render pass encoding.
    Render,
    /// Surface presentation and profiler frame marks.
    Present,
}

impl Phase {
    /// Returns all per-frame phases in execution order (excludes `Startup`).
    pub(crate) const fn frame_phases() -> &'static [Phase] {
        &[
            Phase::PreUpdate,
            Phase::FixedUpdate,
            Phase::Update,
            Phase::PostUpdate,
            Phase::Render,
            Phase::Present,
        ]
    }
}
