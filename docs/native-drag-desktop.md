# AppKit and Win32 drag adapters

`astrelis-winit`'s optional `desktop-drag` feature connects
`drag::desktop::DesktopDragHost` to the existing `CapturedDrag` API. It uses the
small `astrelis-native-drag` workspace crate for native hooks. Rendering, layout,
docking policy, previews, and panel transfers remain with the application/RXUI.
The adapters are experimental; implementation is available for macOS and Windows,
but interactive acceptance testing remains outstanding.

## Try the two-window example

```sh
mbx run -p astrelis-winit --features desktop-drag --example native_drag_desktop
# Bounded registration/lifecycle smoke; does not exercise a mouse gesture:
mbx run -p astrelis-winit --features desktop-drag --example native_drag_desktop -- --smoke
```

Press primary in either window's content and drag outside it into the other.
Titles and logs show native targets and destination logical coordinates. Cover
the destination with another application and release: it must report `Foreign`,
not the covered destination. Escape, source focus loss, suspension, and source
removal cancel. The example uses raw winit windows without a GPU, so it isolates
native input from surface initialization/presentation. It transfers no panels.

## Host integration

1. Construct `DesktopDragHost` on the winit event-loop thread. On AppKit this must
   be the main thread. Keep the host alive longer than its registrations/leases.
2. `register(&Arc<Window>)` each destination/source before its native primary down.
   Registration uses native identity and weak winit ownership, not rectangle maps.
3. After the normal winit primary press, call
   `CapturedDrag::begin(&mut host, source, MouseButton::Left)`. Failure means no
   verified eligible native gesture; do not fall back to guessed targeting.
4. While active, call `poll_native()` from input handling and `about_to_wait`.
   Use a short `ControlFlow::WaitUntil` (the example uses 8 ms). Native callbacks
   latch cancellation but do not independently wake a sleeping winit loop.
   These observations are drag mechanics; ordinary UI input still goes through
   winit. Choose one source of drag motion/release updates to avoid duplicates.
5. Apply application policy to each `RoutedDragUpdate`. A native release uses its
   release-boundary target snapshot, not an earlier hover or a later cursor move.
   Respect UI cancellation/default prevention before committing a docking transfer.
6. Cancel on suspension/programmatic closure and unregister before destroying
   windows. `into_host()` returns the host borrow for another gesture. Ending
   the lease preserves native button capture owned by winit or a newer gesture.

Only primary mouse gestures are supported; this does not add native touch/pen,
cross-process payload transfer, OS data DnD, or automatic RXUI orchestration.
Unsupported operating systems return an initialization error. Wayland continues
to require a compositor-level integration; the separate `x11-drag` host retains
its own passive-grab/input-bridge contract.

## macOS implementation

An AppKit local event monitor observes native mouse down/motion/up and Escape,
returns the original event unchanged, and never invokes application UI code.
The adapter adopts AppKit/winit's active mouse-down tracking sequence; it does
not install a global event tap, confine the pointer, synthesize input, or start
an AppKit payload-drag session. Acquisition requires a fresh observed down in a
registered source, an active application, and a currently held primary button.
A native down can be consumed only once. Missing up, deactivation, source key
loss, closure, destruction, or monitor failure invalidates the lease. Source
liveness is checked again even after native release but before polling.

AppKit's global [`pressedMouseButtons`](https://developer.apple.com/documentation/appkit/nsevent/pressedmousebuttons) state can report up before its queued
`LeftMouseUp` reaches the local monitor. A mask-up sample now starts a fixed
500 ms wait for that native event, keeping public state `Held`. Repeated polls,
older queued drag events, and later mask-held samples cannot renew or clear the
deadline. Only an observed matching native up can set `Released`; a missing or
late up cancels once with no drop. Explicit native liveness loss still cancels
immediately, including during this wait. Fresh native down retires the old
gesture and deadline, preserving the new press across old-lease cleanup.

The grace period is a bounded cancellation policy, not proof that AppKit's queue
has drained. A backlog longer than the bound may still cancel conservatively.
This fixes a confirmed event-ordering bug; its role in a reported physical
second-tab preview without transfer still needs post-fix verification. A
controlled RXUI baseline reproduction stalled its main thread for about 60 ms,
queued native drag/up, then resumed: the old adapter emitted `Lost` without
`Released`. Fake-clock tests cover that ordering and the timeout/loss boundaries;
the RXUI owner runs the matching native integration test separately.

Set `ASTRELIS_NATIVE_DRAG_TRACE=1` (or `RXUI_DOCK_TRACE=1`) to log pending-up
start, observed release, and terminal loss reasons. `pending-native-up-timeout`
distinguishes missing/late up from `native-liveness`, `monitor-failure`, Escape,
or an invalid native up. Liveness diagnostics include application activity,
notification flags, and source availability. Tracing adds no native hooks and
is disabled unless one of those variables is present.

`NSWindow.windowNumberAtPoint:belowWindowWithWindowNumber:` selects the frontmost
native input window, including foreign windows. Unknown window numbers block
registered destinations. An owned window's native content view is identity-matched
to winit's AppKit handle; no raw view dereference is needed. Screen coordinates
stay in AppKit points and convert through the destination `NSWindow`/`NSView`;
this avoids multiplying mixed-monitor global coordinates by one backing scale.
Content bounds exclude decorations. This models AppKit mouse-down targeting,
including native mouse-ignoring-window behavior, rather than compositor pixels.

AppKit has no Win32-style arbitrary mouse-capture lease. The advertised capture
is limited to this verified existing native primary tracking sequence. It cannot
be acquired from a timer, another application's press, or a synthetic UI event.
Actual outside-window delivery and interruption behavior need the interactive
acceptance matrix below before production support is claimed.

## Unsafe boundary and alternatives

Workspace crates retain `unsafe_code = "forbid"`. The new native crate denies
unsafe by default and allows it only in its OS modules, with documented blocks
and `unsafe_op_in_unsafe_fn = "deny"`. It exposes a safe, non-Send/non-Sync API.
AppKit monitor tokens/notification observers are retained and removed on the
creation thread; callbacks never unwind through the native ABI. Notification
callbacks capture only atomic flags. Win32 subclass ownership and cleanup are
described in [the Windows notes](native-drag-windows.md).

Existing options were inspected before adding these bindings:

- [`drag`](https://docs.rs/drag/2.1.1/drag/) implements OS payload drag sessions.
  Its endpoint callback does not supply this host's live hover, capture lease,
  and synchronous loss contract.
- [`native-windows-gui`'s raw handler](https://docs.rs/native-windows-gui/1.0.13/native_windows_gui/fn.bind_raw_event_handler.html)
  supplies a subclass wrapper, but importing another GUI framework just for
  this hook is unnecessary. The adapter uses `windows-sys` directly.
- `objc2`/`objc2-app-kit` provide the native AppKit bindings. The small unsafe
  monitor boundary is audited here instead of weakening Astrelis's core lint.

## Validation and remaining acceptance work

On 2026-10-23, the macOS host ran the two-window smoke: both native windows
registered at scale 2 with 840×520 physical content, target `Foreign`, and exited
cleanly. **The console session was locked**, so no physical/synthetic drag was
attempted and this does not establish outside-window release, stacking, or
mixed-DPI correctness. State regressions exercise consumed press, cancellation,
pending-release invalidation, expected button-up, and main-thread restriction.
Windows was cross-compiled/type-checked (including tests and strict clippy),
not executed. Its ignored native test and exact commands are in the Windows notes.

Validation commands (existing toolchain, default shared mbx cache):

```sh
mbx test -p astrelis-native-drag --locked
mbx test -p astrelis-winit --all-features --locked
mbx clippy --workspace --all-targets --all-features --locked -- -D warnings
mbx check -p astrelis-winit --features desktop-drag --all-targets --target x86_64-pc-windows-gnu --locked
mbx check --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu --locked
mbx check --workspace --lib --all-features --target wasm32-unknown-unknown --locked
```

The macOS native crate has 14 passing regressions; winit has 26 passing unit
tests and two passing doctests. Its 11 X11 server tests stay ignored in this run
(their prior real-server validation is described in the X11 notes). Eight Windows
unit tests plus one ignored interactive native test are compiled, not executed.

Before claiming production support, run the example on each interactive OS:
outside-window release; foreign overlay/decorations; source/destination removal;
Escape/focus/deactivation; a new press before old cleanup; primary plus another
held button; cancellation/theft; moving the pointer after native up but before
polling; and 1×/1.5×/2× monitor transitions. Verify final logical coordinates and
that cancellation never becomes a drop or disrupts another native gesture.
