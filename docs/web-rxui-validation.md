# RXUI browser validation — 2026-10-23

This records the browser investigation before the optional X11 backend was added.
Subsequent [native X11 host work](native-drag-x11.md) passed real-server capture
and targeting tests; it still needs an RXUI primary-input bridge. The failed
macOS GPU-window smoke below remains a separate, unchanged validation result.

## Result and source revisions

RXUI runs in an actual browser/WebAssembly runtime through its `rendering`
feature and an application-owned Astrelis canvas host. A browser runner is not
required for this integration path. It would consolidate lifecycle/scheduling
and support a shared application entry point; it would not provide RXUI's missing
browser platform services by itself.

- Astrelis base: `70467297a33193f425b67ec3bea3bf6241590698`, plus this session's
  local WebGPU example/MSAA storage-and-view-format fix. No dependency/toolchain
  pin was upgraded for this validation.
- RXUI base: `34b6d2d` (`feat: optimize docking and add controlled workspace
  surfaces`). An isolated detached worktree adds `examples/web-probe` and a
  root Cargo patch pointing at the local Astrelis checkout. RXUI main was not edited.
- The only RXUI library change in that worktree is a wasm-specific `web-time`
  dependency and `web_time::Instant` for select type-ahead. A separate
  `rxui-clock.patch` contains that change without local workspace paths.
- Runtime: Playwright 1.64.1, Chromium `156.0.8078.4`, WebGPU with adapter vendor
  `google`, architecture `swiftshader`, on macOS. This establishes the browser
  WebGPU path, not hardware-driver or cross-browser certification.

The complete runnable probe and an evidence bundle are handed back to the RXUI
thread. They remain local investigation artifacts; machine-specific Cargo paths
must not be published. The probe README records build/serve/test instructions.

## Browser coverage

**Twelve tests passed**, six cases at each device pixel ratio, 1 and 2:

1. Embedded-font GPU pixels, reactive counter clicks, responsive layout after
   resize, animation through host frame timestamps, idle redraw suppression,
   and explicit surface teardown/recreation followed by successful input/render.
2. Actual DOM mouse pointer capture, motion outside the canvas, release outside,
   and RXUI cancellation through the host deactivation handler.
3. Precise wheel scrolling, controlled tab selection, split creation, and
   actual mouse dragging of the split divider.
4. Zero-size acquisition suspension/resume with model preservation, plus the
   shared visibility handler stopping/resuming frame scheduling.
5. Actual character/Enter keyboard input exercising select type-ahead using the
   isolated browser-safe clock fix.
6. Actual mouse dragging of a dock tab to the pane edge, committing a controlled
   docking split and releasing capture.

All tests check the visible host status and uncaught/console errors. A screenshot
capture also records browser adapter identity and a full RXUI element/state
snapshot. The canvas uses 4x MSAA and the embedded Source Sans 3 font. Core
Astrelis's separate web suite had already verified 1x/4x sRGB colors.

The visibility case calls the real handler directly; it does not emulate an
actual background tab. Surface recreation is host-requested, not a spontaneous
surface-loss event. Device loss, real tab freeze/throttling, moving between
physical displays, touch/pen, Firefox, Safari and mobile browsers were not tested.
T3 preview inspection timed out twice; the repository's Playwright browser
workflow supplied the working browser automation and screenshots.

## Concrete RXUI issues and host requirements

**Runtime failure reproduced:** select type-ahead in `standard/select.rs` calls
`std::time::Instant::now()`. An actual browser keypress trapped with wasm
`unreachable` at both DPRs. The local wasm-specific `web_time::Instant` patch
removes that trap; type-ahead and selection now pass. Native clock behavior stays
unchanged.

**Input preparation is required:** reconcile dirty UI descriptions before another
input dispatch, as RXUI's native host does. Otherwise a quick type-ahead/Enter
sequence can invoke the old selection listener before the next RAF prepares the
new description. The probe now does this before pointer and keyboard events.
Likewise, DOM wheel deltas map directly to RXUI's positive content-offset deltas;
winit's native wheel sign convention must not be copied blindly.

**Browser-local tasks remain unsupported by the existing task boundary:**
`BackgroundFuture` requires `Send`, and `TaskExecutor` requires `Send + Sync`.
An explicit wasm compile-fail probe with `Rc` retained across `await` fails with
`future cannot be sent between threads safely`. A browser-local future/task
adapter is still needed; this is not fixed by supplying an executor alone.

**Tooling:** strict wasm Clippy finds two existing
`clippy::arc_with_non_send_sync` diagnostics in RXUI's image/composition shared
ownership. The scoped command allowing only that lint passes. These are recorded
as tooling diagnostics, not runtime failures or a fully clean strict wasm check.

The prototype bridges primary mouse, capture, wheel, pointer focus, and limited
character/Enter keys. It does not implement general keyboard/default editing,
Tab traversal, IME/composition, clipboard, DOM accessibility publication, tasks,
file dialogs, or browser window/service policy. These belong to RXUI's web host
and platform-service integration, above Astrelis rendering/lifecycle.

## Remaining Astrelis responsibilities

A shared browser runner still needs asynchronous device initialization, an owned
browser spawn entry point instead of a borrowed blocking native `run`,
browser-safe scheduling clocks, canvas binding and resize/DPI/visibility policy,
and event/message/redraw/deadline integration. No browser Runner/Handler API was
added during this probe. The working JS/Rust host is evidence for the required
boundary, not a claim that the native runner already supports wasm.

For native cross-window drag, [the new drag primitives](native-drag.md) provide
per-window macOS normalization, physical/logical mapping, observable event
tracking, explicit capabilities, fresh release targeting and an RAII capture
contract. The winit-only backend reports authoritative native targeting,
unconstrained capture and capture-loss notification as unavailable. An Astrelis
OS host adapter (or upstream winit capability) remains necessary for reliable
automatic routing. RXUI should own docking acceptance/preview/layout transfer,
not infer native z-order from window rectangles.

The two-window native example compiles and created two actual macOS windows at
DPR 2. Its bounded submission smoke assertion failed: a temporary acquisition
probe returned `Suspended`, and no frame submissions were recorded. This is
explicitly not a successful interactive native validation. Physical mixed-DPI,
foreign occlusion/stacking and capture theft remain unmeasured. Wayland global
positioning/compositor DnD and Windows silent capture theft remain public-winit
capability gaps; no unsupported native guarantee is advertised.

## Commands and results

From the isolated RXUI root:

```sh
mbx build -p rxui-web-probe --target wasm32-unknown-unknown --release
sh examples/web-probe/build.sh
# build.sh repeats the build with --locked, then runs wasm-bindgen --target web.
cd examples/web-probe
npm ci
npm test
# 12 passed, 14.0 seconds on the recorded final run.
```

From that worktree root, the task limitation is intentionally a failing check:

```sh
mbx check -p rxui-web-probe --target wasm32-unknown-unknown --features non-send-task-probe
# Expected: future cannot be sent between threads safely (Rc across await).
mbx clippy -p rxui-web-probe --target wasm32-unknown-unknown --locked -- -D warnings
# Two RXUI Arc diagnostics.
mbx clippy -p rxui-web-probe --target wasm32-unknown-unknown --locked -- -D warnings -A clippy::arc_with_non_send_sync
# Passes.
```

From the Astrelis checkout:

```sh
mbx test --workspace --locked
# 225 unit/GPU tests + 30 doctests pass; one existing ignored test.
mbx clippy --workspace --all-targets --locked -- -D warnings
# Passes, including the native two-window example.
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
# Passes.
mbx run -p astrelis-winit --example native_drag --locked -- --smoke
# Fails the frame-submission assertion in this desktop session; exits and closes windows.
```

The evidence bundle includes raw logs for the baseline wasm trap, passing browser
suite, task compile failure, strict/scoped Clippy, native tests and the failed
native acquisition diagnostic. Temporary browser servers and native processes
are stopped after capture. No commits, pushes, PRs or published artifacts are
part of this handoff.
