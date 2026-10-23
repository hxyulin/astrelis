# Windows native drag adapter

The isolated `astrelis-native-drag` Windows module observes winit's native primary
mouse capture through `SetWindowSubclass`. Register each window on its owning
event-loop thread **before** the press, then call `begin_capture` after winit's
public primary press. The adapter verifies an observed native press and winit's
current capture; it does not take capture from another window. Only the primary
mouse button is supported. This implementation requires Windows 10 with PMv2
thread DPI contexts (1703 or later) and per-monitor-aware winit windows.

## Release and cancellation

Winit 0.30.13 calls `ReleaseCapture` before delivering public primary release.
The subclass therefore samples the native target and latches `Released` during
`WM_LBUTTONUP`, before `DefSubclassProc`. The nested `WM_CAPTURECHANGED` produced
by normal release does not cancel that snapshot. `target_at_pointer` keeps the
snapshot until the lease ends or another gesture begins, so subsequent cursor
movement cannot retarget the drop. A registered destination is checked again for
identity/liveness when the snapshot is read.

Capture theft, `WM_CANCELMODE`, focus loss, application deactivation, and native
source destruction cancel an active gesture. Loss remains latched even if the
same HWND subsequently regains capture before polling. Source destruction and
explicit unregistration also invalidate an unconsumed release. Native callbacks
enqueue signals; they store and invoke no application callback.

`end_capture`, unregistration, and adapter destruction end only the adapter's
lease. Native capture remains winit's responsibility: winit shares it across
pressed buttons and releases it when its own count reaches zero. The adapter
never calls `ReleaseCapture`, so cancelling a primary drag cannot disrupt a
right/middle/X-button gesture or capture acquired by a newer gesture/window.
Each successful adoption consumes that native press generation. Ending a lease
cannot make the same held press eligible again. Cleanup does not consume a newer
Down observed before the previous guard is dropped, so that genuinely new press
can still start a drag.

## Native targeting and coordinates

`GetPhysicalCursorPos` plus `WindowFromPhysicalPoint` selects the actual native
input window. Only an exact live registered HWND is eligible. An unregistered
child, foreign application window, or overlay is not resolved by searching
registered rectangles behind it. Native client bounds exclude title bars and
standard decorations. Hidden/disabled-window behavior follows
[WindowFromPhysicalPoint](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-windowfromphysicalpoint),
which skips those windows; the adapter models native input targeting rather than
compositor-visible pixels.

Client conversion runs in a short PMv2 thread context, restored before forwarding
any message. Destination physical pixels become logical coordinates using that
window's `GetDpiForWindow / 96`. Registration rejects non-per-monitor-aware
windows instead of guessing at virtualized coordinates. Motion/release signals
use the native source's signed physical client coordinates from the mouse
message, including negative outside-client positions. Their range follows the
Win32 mouse message's signed 16-bit coordinate fields.

## Ownership and unsafe boundary

All bindings and unsafe operations are confined to `src/windows.rs` and its
Windows tests. Registration validates the HWND's process, owner thread, and live
winit identity. The registry stores `Weak<Window>` and does not retain removed UI
windows. Each subclass owns one retained `Rc<Hook>`; each invocation immediately
takes another strong reference to survive reentrant removal or destruction.
Successful removal or `WM_NCDESTROY` retires the subclass reference exactly once.
An unexpected removal failure retains its reference until destruction, avoiding
a dangling callback; only weak references to the adapter/window remain.

Gesture state uses `Cell`. Registry/signal `RefCell` borrows end before OS calls
and temporary window-reference drops, including drops that might synchronously
destroy the HWND. A final panic boundary prevents Rust unwinding through the
subclass ABI. The public adapter is neither `Send` nor `Sync`.

## Validation

On the macOS implementation host, these checks passed with the existing Rust
toolchain and mbx-managed target storage:

```sh
mbx check -p astrelis-native-drag --target x86_64-pc-windows-gnu
mbx check -p astrelis-native-drag --tests --target x86_64-pc-windows-gnu
mbx clippy -p astrelis-native-drag --tests --target x86_64-pc-windows-gnu -- -D warnings
```

**Windows tests have been cross-checked, not executed on this host.** On Windows,
run state-machine tests with:

```sh
mbx test -p astrelis-native-drag windows::tests
```

Run the ignored native test explicitly on an interactive Windows desktop, with
no other mouse automation running. It creates real winit windows and takes real
native capture, then verifies release ordering/physical signal coordinates,
foreign-window capture preservation, left-plus-right capture across drag
cancellation and the individual button releases, same-HWND recapture after
theft, rejection of a cancelled press, eligibility of a newer press observed
before old cleanup, and destruction/unbinding:

```sh
mbx test -p astrelis-native-drag native_winit_release_theft_and_destruction -- --ignored --test-threads=1
```

Additional interactive acceptance checks remain necessary: drag between two
registered windows at 100% and 150%/200% monitor scaling, release after moving to
the other monitor, cover the destination with a foreign application, release over
its title bar/resize border, remove source/destination during capture, and move
the pointer again before consuming public release. Expected results are logical
destination coordinates, no drop through an occluder/decoration, source loss on
removal, and the unchanged native release snapshot.

## Win32 references

- [SetWindowSubclass](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass)
  and [RemoveWindowSubclass](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-removewindowsubclass):
  callback identity/reference ownership and owner-thread restriction.
- [WM_CAPTURECHANGED](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-capturechanged):
  notification occurs even for an explicit `ReleaseCapture`.
- [GetCapture](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getcapture):
  the result is limited to capture associated with the current thread.
- [SetThreadDpiAwarenessContext](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddpiawarenesscontext)
  and [GetDpiForWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow):
  scoped coordinate conversion and per-monitor destination scale.
- [GetClientRect](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect):
  native client bounds with exclusive bottom/right edges.
