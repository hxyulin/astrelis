# astrelis-app

Shared application scheduling for Astrelis desktop applications, games, and
tools.

The runtime adapts an `astrelis_app::App` to the platform lifecycle and owns
only scheduling mechanisms: timers, main-thread task wakeups, per-window
invalidation, fixed updates, and frame pacing. Applications retain their own
state and window handles.

- `RuntimePolicy::Desktop` sleeps with `Wait` or `WaitUntil` and redraws only
  invalidated windows.
- `RuntimePolicy::Continuous` updates every frame using `Poll` or a paced
  `WaitUntil` deadline.

Run the examples with:

```text
cargo run -p astrelis-app --example idle_counter
cargo run -p astrelis-app --example continuous_animation
```
