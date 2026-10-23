# Native drag integration

`astrelis_winit::drag` separates reusable native drag mechanics from RXUI docking
policy, previews, and panel transfer. **Reliable automatic cross-window native
routing is not implemented by the winit-only backend.** Applications can supply a
native host; the API does not turn geometry guesses into native guarantees.
The default backend introduces no unsafe code or platform dependencies. The optional
[`x11-drag` native host](native-drag-x11.md) now supplies server-tested targeting,
capture and cancellation for its documented X11 subset. It changes primary-input
ownership and requires explicit host integration. The experimental
[`desktop-drag` AppKit/Win32 adapters](native-drag-desktop.md) connect native
observation through a small isolated bindings crate; interactive validation is
still pending for those adapters.

## Available primitives

- `DragGeometry` snapshots current native client origin, size, and scale. It maps
  source physical cursor coordinates through desktop space to destination logical
  content coordinates. Mapping permits negative/out-of-content coordinates and
  makes no visibility, stacking, or destination assertion.
- `DragTracker` follows one source/button after an observed press. It ignores
  releases from other buttons/windows, permits cursor exit, terminates once on
  matching release, and cancels on Escape, focus loss, close request, or destruction.
  The application explicitly cancels on suspension, programmatic source removal,
  host capture loss, and shutdown. It acquires no native capture.
- `NativeDragCapabilities::WINIT_ONLY` reports authoritative targeting, explicit
  unconstrained capture, and observable capture-loss support as unavailable.
  `reliable_routing()` requires all three guarantees.
- `NativeDragHost` lets an application supply those native guarantees.
  `CapturedDrag::begin` refuses incomplete capabilities and reports capture errors.
  Its lease releases acquired capture once on button release, cancellation, or
  drop. It re-queries the live native target on release before releasing capture;
  it never commits a cached hover target. A capability downgrade cancels the lease.
  `into_host()` ends the lease and returns the host borrow for the next drag,
  allowing an event-loop handler to move it between host/session `Option` fields.
  RXUI decides whether to accept the result.

`NativeDragTarget` distinguishes `Unavailable` (no authoritative answer),
`NoWindow`, `Foreign` (including unregistered windows or blocking decorations), and
an identified registered window with destination logical coordinates. Unknown
answers must not be interpreted as empty space or an eligible docking target.

## Coordinate conventions and precision

macOS winit 0.30.13 obtains a top-left Cocoa desktop origin in points, then
multiplies it by **that window's scale factor** to return `inner_position()`.
Those reported positions cannot be compared directly between differently scaled
windows. `DragGeometry` divides each origin and each source-local physical cursor
position by its own window scale, producing `DesktopSpace::MacOsPoints`. After
subtracting the normalized destination origin, the result is already destination
logical coordinates. winit rounds the reported native origin to integer physical
pixels, so normalization cannot recover the lost fractional precision.

On Windows and X11, desktop origins and cursor positions use physical pixel space.
Map by adding source origin to source physical position, subtracting destination
origin, and dividing by the **destination** scale. Do not divide the whole global
position by the source scale. Negative desktop origins and fractional DPI are valid.
Tagged `DesktopPoint`s reject mapping across incompatible conventions; non-finite
input or arithmetic results are rejected. Geometry queries are best-effort snapshots,
not atomic native transactions. Refresh after moves/DPI changes and at drop time;
a cached geometry map is never proof of current destination identity.

## Native host contract

A host advertising reliable routing must:

1. Acquire unconstrained pointer capture following the initiating press and deliver
   motion/release to the source outside its bounds. Cursor confinement/locking
   (`set_cursor_grab`) prevents the required free cross-window movement.
2. Observe capture theft/compositor cancellation and call `CapturedDrag::cancel`
   with `CaptureLost` before any release can be accepted. While the host is borrowed
   by the lease, deliver external loss through the application event channel or a
   shared native state bridge. Do not synchronously re-enter the lease from the host.
3. Resolve the current pointer against real native stacking and input regions,
   including foreign windows, decorations, hidden/minimized windows, and stale
   registrations. Return only current registered native identities and accurately
   mapped destination content coordinates. A foreign window covering an app window
   blocks that app window even if its content rectangle contains the cursor.
4. Return `Unavailable` on query failure, stale identity, loss of required capability,
   or uncertainty. Clean up partial acquisition on failure and release safely after
   capture loss. `release_capture` must finish cleanup without panicking.

The host may query native pointer coordinates directly; it need not reconstruct
them from the last source `CursorMoved`. This matters when the pointer/window moves
between the last motion event and release. Capability flags are an implementer's
contract, not runtime verification of native behavior. The supplied unit-test host
demonstrates the contract mechanically; it is not an OS adapter.

## Platform boundaries

| Platform | Geometry via winit | Native targeting / capture limits here |
| --- | --- | --- |
| macOS | Per-window scale-normalized desktop points | Optional experimental `desktop-drag`: native input targeting, AppKit primary tracking and monitor/notification cancellation. Two-window registration smoke passed; actual drag and mixed-DPI validation pending. |
| Windows | Desktop physical pixels, destination DPI conversion | Optional experimental `desktop-drag`: subclasses observe release/loss before winit processing and adopt winit capture. Windows 10 PMv2/per-monitor-aware subset; cross-checked, native execution pending. |
| X11 | Desktop physical pixels when native queries succeed | Optional `x11-drag` host supplies server-tested targeting/capture for its documented standalone X11, primary-button subset. A separate native input bridge is required. |
| Wayland | General global window positions unavailable; `from_window` returns `None` | winit offers neither general global targeting nor compositor DnD initiation for arbitrary application payloads. A capable compositor/host protocol integration is required; local rectangles cannot repair this. |

Focus, creation order, `CursorEntered`, and `WindowEvent::Occluded(false)` do not
establish actual stacking. A native hit-test may need platform-specific treatment
of transparent regions and overlays. This crate does not guess these semantics.
The module is desktop-only and does not add a browser drag API.

Reference behavior was checked against the pinned [winit 0.30.13 Window API](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html),
and its local sources: macOS `window_delegate.rs::inner_position`, Windows
`event_loop.rs` handling `WM_CAPTURECHANGED`, and `window.rs::set_cursor_grab`.

## Two-window measurement example

Run on a desktop with:

```sh
~/.cargo/bin/mbx run -p astrelis-winit --example native_drag
```

Two colored native windows overlap initially. Hold the left button in either and
move the pointer. Both titles show the same pointer mapped into their own logical
content coordinates. Escape/focus loss cancels; terminal events print to stderr.
The example deliberately installs no host, reports unavailable routing capabilities,
and performs no transfer. It is a runnable geometry/event-delivery probe, **not a
proof of reliable native targeting or capture**.

Manual checks (not automated measurements): move the windows between differently
scaled monitors, test negative monitor origins, overlap the two app windows, cover
one with another application's window, release outside the source, press Escape,
switch focus, and close/suspend during a drag. Titles may continue to map through a
covered window: that demonstrates the difference between geometry and targeting.
On Wayland titles should report global geometry unavailable. Windows capture theft
without focus loss cannot be validated through winit's public event stream.

Unit tests exercise mixed-DPI formulas, negative/fractional origins, refreshed
geometry, coordinate-space rejection, event filtering, cancellation idempotence,
capability rejection, failed acquisition, RAII release, and a foreign window
appearing between hover and release. These are deterministic protocol/coordinate
tests; native stacking, compositor behavior, and physical multi-monitor accuracy
remain unmeasured until tested with an actual native host and desktop interaction.

## Recorded validation (2026-10-23)

The ten drag unit tests and the existing sixteen lifecycle/scheduler unit tests
pass, as do both astrelis-winit doctests and strict native Clippy. Tests exercise
synthetic macOS 1x/2x/fractional coordinate normalization, physical-pixel mapping,
cancellation, unavailable targets, capture acquisition errors, RAII cleanup, and
fresh targeting at release. They do not measure native stacking.

The example also has a bounded desktop smoke command:

```sh
mbx run -p astrelis-winit --example native_drag --locked -- --smoke
```

It requires frame submission for both windows and exits automatically. On the
validation Mac, two actual 480x320 logical windows were created at scale factor 2,
with normalized desktop origins (120, 160) and (520, 160). Redraw/prepare events
were delivered, but a temporary acquisition diagnostic returned `Suspended` and
no frames were submitted. The smoke assertion therefore **failed**; this is not
recorded as a successful interactive native run. For a nonzero surface the core
maps wgpu's `Occluded` acquisition result to `Suspended`. A visible interactive
desktop run is still required, including physical mixed-DPI displays, overlaps
with foreign windows, and platform capture cancellation. The manual checklist
above remains outstanding.
