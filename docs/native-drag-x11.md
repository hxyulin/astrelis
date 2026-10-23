# Native X11 drag host

The optional `astrelis-winit/x11-drag` feature implements an X11 primary-pointer
host with live native targeting and unconstrained capture. It uses x11rb's safe
Rust protocol connection; Astrelis's `unsafe_code = "forbid"` policy remains
unchanged. It is opt-in and does not enable automatic RXUI docking.

The supported subset is one X server screen, its core primary mouse pointer,
registered X11 clients, and normal X11 grab semantics. It is tested against real
Xvfb, including a separate foreign client. This is native protocol validation,
not a certification of Linux desktop/window-manager integration or physical
mixed-DPI displays. Wayland, AppKit and Win32 clients are not accepted.

## Native guarantees and limits

- `QueryPointer` follows the server's current input-shaped child path. Registered
  client leaves are eligible; an unregistered leaf (including a foreign client
  or window-manager decoration) blocks the destination. The host never searches
  behind it or guesses order from rectangles. Inconsistent pointer snapshots,
  missing/dead registrations, other screens and protocol failures yield
  `Unavailable`. Native input-transparent holes pass through according to X11's
  input shape, rather than visual opacity. A short RAII server grab spans the
  bounded tree walk so another client cannot restack between its queries. No UI
  callback/window API runs under that grab; cleanup releases it on every return.
  Use a local server connection: network latency extends this serialization.
- Physical destination coordinates come from the server's client-relative query.
  Winit registrations read that window's current scale; custom registrations use
  the explicitly supplied scale. No global source-DPI division is applied.
- A passive grab is armed **before** the press. Capture then belongs to this host's
  connection. An explicit unconfined grab retains motion and release outside the
  source. Acquisition failure is reported; no rectangle-only fallback occurs.
  Only `MouseButton::Left` is supported; other buttons are rejected before capture.
  Button chords and core wheel buttons during a primary gesture are not bridged;
  they are outside this subset. Other-button delivery remains ordinary native
  delivery only when no primary grab is active.
- `CapturedDrag::poll_x11` routes native motion/release and cancels on source
  unmap/destruction/ungrab notification or connection failure. Normal core X11
  grabs cannot be stolen by another client's grab request: it gets
  `AlreadyGrabbed`. On release, the host queries the current native destination
  before relinquishing capture. Cancellation/drop releases the acquired grab.
  Queued release and unmap/remap cannot revive an obsolete initiating press.
- Poll promptly and cancel on application suspension, Escape, source removal and
  focus policy. The host does not grab the keyboard or implement UI policy.
  Exclusive use of the host connection is required: other code must not consume
  its events or change its grabs. Server extensions/admin operations that forcibly
  break grabs without core notifications are outside this supported contract.
  Xwayland provides only its X11 tree, not authoritative native Wayland occlusion;
  it must not be used for compositor-wide routing there. The constructor rejects
  an implicit Wayland session and known `XWAYLAND`/`Apple-WM` server extensions.
  Other nested/embedded X servers are outside the standalone-server precondition;
  the caller must not bypass it by selecting an explicit display.

## Winit integration: primary input changes owner

This is not a host you can attach after a winit `MouseInput::Pressed`. Winit's
connection already owns an implicit grab at that point, and a separate connection
gets `AlreadyGrabbed`. The real-server tests explicitly cover this failure.

Register and arm each eligible native X11 window ahead of time:

```rust,ignore
use astrelis_winit::drag::x11::X11DragHost;
let mut host = X11DragHost::connect(None)?; // Same DISPLAY/screen as winit.
host.register(&window)?;                   // Arc<winit::window::Window>.
host.arm(window.id())?;
```

An armed window's primary press, captured motion and release now arrive via
`host.poll_input()`, **not** ordinary winit mouse delivery. Bridge those events
into the UI even when no dock drag starts. Continue forwarding winit keyboard,
focus/lifecycle and other native events; avoid double-routing mouse motion during
capture. Modifier interpretation and normal control behavior remain UI-host work.
Unregister before native identity reuse, and cancel the UI gesture first.

After the application accepts a host-delivered press as a drag:

```rust,ignore
use astrelis_winit::{drag::CapturedDrag, winit::event::MouseButton};
let mut drag = CapturedDrag::begin(&mut host, source, MouseButton::Left)?;
while drag.is_active() {
    if let Some(update) = drag.poll_x11()? {
        // Preview or finish according to update.update and update.target.
        // Unavailable/Foreign are never an eligible docking destination.
    }
    // Return to the application's wake mechanism; do not busy-spin.
}
```

The separate connection needs its own wake integration while armed, including
while the UI is idle. The supplied example polls with bounded sleeps; a winit
host can use wake/deadline scheduling. Runner-managed socket readiness and an
RXUI primary-input bridge are not implemented here. A borrowed capture lease can
be kept across handler calls by storing the host outside the handler, with host
and lease in mutually exclusive `Option` fields; `into_host()` returns the borrow
when the gesture ends. Capture-start failure should terminate that host operation.

## Two-window proof and tests

The standalone `native_drag_x11` example creates blue and green native X11
windows with a different owning connection from the drag host. Drag from blue to
green; the terminal reports source physical motion and native destination logical
coordinates. It does not render a GPU scene or transfer RXUI panels.

```sh
DISPLAY=:0 mbx run -p astrelis-winit --features x11-drag --example native_drag_x11 --locked
```

For isolated automation on Linux, install Xvfb/xauth, start an isolated server,
and set `ASTRELIS_X11_TEST_DISPLAY` explicitly. Tests never default to the user's
normal display because XTEST generates real pointer input on the selected server:

```sh
xvfb-run -a -s '-screen 0 1024x768x24 -noreset' sh -c '
  export ASTRELIS_X11_TEST_DISPLAY="$DISPLAY"
  mbx test -p astrelis-winit --features x11-drag --lib drag::x11 --locked -- --ignored
  mbx run -p astrelis-winit --features x11-drag --example native_drag_x11 --locked -- --smoke
'
```

The tests are ignored in ordinary runs and CI supplies a dedicated Xvfb job.
Eleven server tests validate stacking/foreign occlusion, input-shaped holes,
window-manager-style decorations, logical scaling, outside-source release,
reacquisition/RAII cleanup, fresh release targeting, destruction/unmap/disconnect,
rejection of stale or already-owned presses, and lost-capture cleanup preserving
a newer window's passive grab/release (both polled cancellation and guard drop). The smoke command injects a
real down/motion/up sequence, checks the destination and exits, closing its windows.
It refuses to inject without `ASTRELIS_X11_TEST_DISPLAY`.

Recorded local validation (2026-10-23): x11rb 0.13.2, Rust 1.99.1 on macOS,
connected over loopback to Debian 13 Xvfb in a disposable container. Eleven tests and
the two-window smoke passed. The smoke reported source motion (420, 60) outside
its 260-pixel-wide client and destination logical (60, 60). No Rust toolchain or
existing backend version changed. Physical mixed-DPI displays, Linux winit event
bridging, window-manager policy and Xwayland/native-Wayland interop remain unverified.

## Other native hosts

AppKit provides authoritative mouse-down hit testing through
[`NSWindow.windowNumber(at:belowWindowWithWindowNumber:)`](https://developer.apple.com/documentation/appkit/nswindow/windownumber(at:belowwindowwithwindownumber:)),
including foreign windows. The current safe bindings can query it and match
winit content-view identity without dereferencing raw handles. A complete
capture/cancellation host is not implemented: local monitors and native dragging
protocol implementations need unsafe integration, and a nested event loop alone
does not establish capture ownership or loss notification.

Windows has [`SetCapture`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcapture)
and [`WM_CAPTURECHANGED`](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-capturechanged).
Safe dependency wrappers offer a possible adapter, but winit's normal button-up
path releases capture before delivering its public event. A native subclass must
record expected release versus theft in order; no tested Windows host is shipped.
The public winit message-queue hook cannot observe every sent window-procedure
message. Wayland still requires compositor-supported DnD rather than global window
coordinates. These platforms continue reporting the conservative winit-only baseline.

X11 references: [grab semantics](https://www.x.org/releases/X11R6.8.0/doc/XGrabPointer.3.html),
[pointer queries](https://www.x.org/releases/X11R6.9.0/doc/html/XQueryPointer.3.html),
and [x11rb](https://docs.rs/x11rb/0.13.2/x11rb/).
