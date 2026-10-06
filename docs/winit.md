# astrelis-winit: window lifecycle and native runner

Status: implemented. `WindowContext` is available for application-owned loops;
`Runner`, `Handler`, `AppContext`, and native scheduling are exported on macOS,
Windows, and Linux. The crate provides standalone triangle, two-window, and custom
loop examples. RXUI adaptation is deliberately deferred for this milestone.

## Boundary and ownership

Astrelis remains a rendering library without a winit runtime dependency.
astrelis-winit depends on Astrelis and winit, and re-exports both. Applications
retain their models, meshes, renderers, Painter instances, framebuffers and UI.
The integration owns managed native windows, presentation targets, metrics,
lifecycle state and scheduling. RXUI translates native events into control input,
publishes semantic accessibility, and chooses IME/cursor state.

Use winit's WindowId, WindowAttributes, WindowEvent, DeviceEvent and
EventLoopProxy directly. A handler's Message is the event loop's actual user-event
type. There is no private user-message envelope, automatic event cloning, renderer
base class, UI dependency, clipboard service, or task executor in this integration.

There are two levels: WindowContext for an application-owned winit event loop, and
Runner for the convenient managed lifecycle. Both use the same surface/lifecycle
and scheduling implementation.

## WindowContext and window information

Public signatures (bodies omitted):

```rust,ignore
impl WindowContext {
    // Select an adapter compatible with this window; creates a context and target.
    pub async fn new(
        window: Arc<Window>, settings: SurfaceSettings,
    ) -> Result<Self, WindowError>;

    // Create another surface on an explicitly selected/shared context.
    pub fn with_graphics(
        graphics: &GraphicsContext, window: Arc<Window>, settings: SurfaceSettings,
    ) -> Result<Self, WindowError>;

    pub fn window(&self) -> &Arc<Window>;
    pub fn graphics(&self) -> &GraphicsContext;
    pub fn metrics(&self) -> WindowMetrics;
    pub fn settings(&self) -> SurfaceSettings;
    pub fn target(&self) -> Option<&RenderTarget<'static>>;
    pub fn render_format(&self) -> Option<&RenderFormat>;
    pub fn surface_generation(&self) -> u64;
    pub fn can_present(&self) -> bool;

    pub fn process_event(&mut self, event: &WindowEvent)
        -> Result<WindowUpdate, WindowError>;
    pub fn suspend(&mut self);
    pub fn resume(&mut self) -> Result<(), WindowError>;
    pub fn set_sample_count(&mut self, count: u32) -> Result<(), WindowError>;
    pub fn set_visible(&mut self, visible: bool);
    pub fn begin_frame(&mut self) -> Result<Frame<'_, 'static>, FrameError>;
}
```

WindowContext owns an Arc<Window>, a GraphicsContext clone, an optional surface
RenderTarget, requested SurfaceSettings, cached WindowMetrics/RenderFormat,
visibility/occlusion state, and a surface generation. Dimensions come from the
window; settings contain only sample count and depth/stencil format/usages.
WindowUpdate reports metrics, presentation availability, and explicit exposure
redraw requests so a custom
event loop can request redraw. Native window events do not replace app-wide
suspend/resume calls.

Only a live target produces a render format. Zero-sized targets may retain a
format but cannot present. App suspension drops the target, retains GPU resources
and requested settings, and makes acquisition return FrameError::Suspended.
Repeated suspension/resume calls are idempotent. Resume/recovery resamples native
metrics and creates a new target; it never assumes the old attachments survived.

A successful MSAA change updates both the live target and recovery settings.
While the target is absent, validate against cached supported counts before
queuing the setting; validate again on recreation. Rejected changes preserve the
previous request. Depth/stencil format remains a creation setting in V1, matching
Astrelis's current target API. Target access is read-only; managed setting methods
keep the recovery configuration consistent. Custom encoding/passes remain
available through Frame. Use core Astrelis directly for fully application-owned
targets or unsupported presentation policies.

WindowInfo is a short-lived view supplied during rendering:

```rust,ignore
impl WindowInfo<'_> {
    pub fn id(&self) -> WindowId;
    pub fn window(&self) -> &Window;
    pub fn graphics(&self) -> &GraphicsContext;
    pub fn metrics(&self) -> WindowMetrics;
    pub fn render_format(&self) -> &RenderFormat;
    pub fn surface_generation(&self) -> u64;
}
```

It borrows window/context/cached format fields independently of the mutable target
borrowed by Frame. The driver does not clone a color-format vector or Arc<Window>
per draw. Native window operations remain accessible. WindowInfo does not expose
the mutable manager or target while a frame is live.

In a custom event loop, retain a native window handle before begin_frame, call
Window::pre_present_notify before Frame::finish, and handle acquisition errors
explicitly. Holding a frame prevents mutation of that WindowContext until the
frame ends. The runner takes care of notification/submission itself.

## Runner and application context

```rust,ignore
impl<Message: 'static> Runner<Message> {
    pub fn new() -> Result<Self, winit::error::EventLoopError>;
    pub fn from_event_loop(event_loop: EventLoop<Message>) -> Self;
    pub fn with_graphics(self, graphics: GraphicsContext) -> Self;
    pub fn with_options(self, options: RunnerOptions) -> Self;
    pub fn proxy(&self) -> EventLoopProxy<Message>;

    // Native desktop entry point. Handler/model remain inspectable after return.
    pub fn run<H: Handler<Message = Message>>(
        self, handler: &mut H,
    ) -> Result<(), RunError>;
}

impl<Message: 'static> AppContext<'_, Message> {
    pub fn create_window(
        &mut self, attributes: WindowAttributes, settings: SurfaceSettings,
    ) -> Result<WindowId, WindowError>;
    pub fn adopt_window(&mut self, window: WindowContext)
        -> Result<WindowId, WindowError>;
    pub fn window(&self, id: WindowId) -> Option<&WindowContext>;
    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut WindowContext>;

    pub fn request_redraw(&mut self, id: WindowId) -> Result<(), UnknownWindow>;
    pub fn request_redraw_at(&mut self, id: WindowId, at: Instant)
        -> Result<(), UnknownWindow>;
    pub fn cancel_redraw_at(&mut self, id: WindowId) -> Result<(), UnknownWindow>;
    pub fn set_redraw_mode(&mut self, id: WindowId, mode: RedrawMode)
        -> Result<(), UnknownWindow>;
    pub fn close_window(&mut self, id: WindowId) -> Result<(), UnknownWindow>;
    pub fn exit(&mut self);

    pub fn event_loop(&self) -> &ActiveEventLoop;
    pub fn proxy(&self) -> EventLoopProxy<Message>;
}
```

Runner is generic over messages, and run is generic over the concrete handler.
The runner owns the event loop/managed registry and borrows the handler for a
native run. A supplied EventLoop preserves platform-specific builder settings.
A supplied GraphicsContext preserves requested device features/limits. Without
one, the first managed window selects a compatible adapter; subsequent windows
share that context. Incompatible surfaces return an error. Adopted WindowContexts
may use independent contexts; the application selects compatible renderers.

Initial implementation scope is desktop. Default create_window may synchronously
wait for the one-time GPU initialization inside Resumed; it never waits for GPU
completion per frame. This startup convenience should be documented explicitly.
WindowContext::new is async, and adopt_window supports caller-managed initialization
with an existing context or executor. Pending async windows and stale completion
results remain the caller's responsibility until adoption. A browser spawn API and
runner-managed asynchronous GPU initialization need a separate lifecycle design;
native run is not advertised as a browser/mobile event-loop implementation.

Creating a managed window forces it hidden through initialization. Once the
current callback returns, window_created runs with native window and GPU context
available; requested visibility is applied afterwards. This gives AccessKit its
required pre-show installation point. set_visible updates the desired visibility
during that hook. Managed visibility changes should use WindowContext::set_visible
so scheduling and desired visibility remain consistent; native title/IME/cursor
operations can use Window directly. An adopted window preserves its existing visibility; its caller
must arrange pre-show installation if needed.

New-window notifications and close operations are drained between callbacks, so
creating/closing a window does not re-enter the handler. Windows requested for
close are skipped by rendering even before the close queue drains. Dropping managed
window/surface handles releases their ownership; an externally retained Arc<Window>
can keep an adopted native window alive. window_closed lets the application release
its own per-window state/handles.

RunnerOptions initially contains exit_on_last_window (true), retry_delay (16 ms)
and maximum consecutive surface recoveries (3). Last-window exit applies after a
managed window has existed; a background/no-window app can opt out. Recovery counts
reset after successful presentation. Zero retry delay and zero recovery limit are
rejected at startup rather than enabling a busy retry loop.

## Handler

The public trait is:

```rust,ignore
pub trait Handler {
    type Message: 'static;
    type Error: Into<Box<dyn std::error::Error>>;

    fn resumed(&mut self, cx: &mut AppContext<'_, Self::Message>)
        -> Result<(), Self::Error> { Ok(()) }

    fn suspended(&mut self, cx: &mut AppContext<'_, Self::Message>)
        -> Result<(), Self::Error> { Ok(()) }

    fn window_created(&mut self, cx: &mut AppContext<'_, Self::Message>, id: WindowId)
        -> Result<(), Self::Error> { Ok(()) }

    fn window_event(
        &mut self, cx: &mut AppContext<'_, Self::Message>,
        id: WindowId, event: WindowEvent,
    ) -> Result<(), Self::Error> { Ok(()) }

    fn user_event(&mut self, cx: &mut AppContext<'_, Self::Message>, message: Self::Message)
        -> Result<(), Self::Error> { Ok(()) }

    fn device_event(
        &mut self, cx: &mut AppContext<'_, Self::Message>,
        id: DeviceId, event: DeviceEvent,
    ) -> Result<(), Self::Error> { Ok(()) }

    fn prepare(&mut self, cx: &mut AppContext<'_, Self::Message>, id: WindowId)
        -> Result<PrepareAction, Self::Error> { Ok(PrepareAction::Render) }

    fn render(&mut self, window: WindowInfo<'_>, frame: &mut Frame<'_, 'static>)
        -> Result<(), Self::Error>;

    fn submitted(
        &mut self, cx: &mut AppContext<'_, Self::Message>,
        id: WindowId, submission: wgpu::SubmissionIndex,
    ) -> Result<(), Self::Error> { Ok(()) }

    fn close_requested(&mut self, cx: &mut AppContext<'_, Self::Message>, id: WindowId)
        -> Result<CloseResponse, Self::Error> { Ok(CloseResponse::Close) }

    fn window_closed(&mut self, cx: &mut AppContext<'_, Self::Message>, id: WindowId)
        -> Result<(), Self::Error> { Ok(()) }

    fn exiting(&mut self, cx: &mut AppContext<'_, Self::Message>)
        -> Result<(), Self::Error> { Ok(()) }
}
```

render is the only required callback. A simple app overrides resumed to open a
window, window_created to initialize resources, and render to draw. Other hooks
are opt-in. The Error conversion accepts concrete errors and Box<dyn Error>;
requiring Error itself to implement std::error::Error would unnecessarily exclude
common boxed-error application signatures. Errors are boxed only on failure.

prepare can change attachments, evaluate UI/layout, prepare renderer pipelines
and upload resources before an image is acquired. PrepareAction is Render or Skip;
Skip acquires no frame and waits for another invalidation. Preparation may run on
an acquisition retry, so simulation progression should be driven by application
time/messages, rather than by counting prepare calls.

render receives the existing Astrelis Frame, supports any number of passes,
offscreen passes and custom encoding, and does not consume/finish the borrowed
frame. After success the runner calls pre_present_notify and finish, then submitted
with the resulting SubmissionIndex. submitted can arrange readback, GPU completion
tracking or the next redraw after a successful submission, with mutable context
available again. It reports queued GPU work and a presentation request; actual OS
display completion is outside that callback. An error
drops the recording without managed submission/presentation. During render,
schedule via a proxy/native redraw request or schedule the next deadline in
prepare/submitted; mutating the window registry is confined to context-bearing callbacks.

window_event receives the original owned winit event after managed size/DPI/
occlusion bookkeeping, including RedrawRequested and CloseRequested. It runs
before the corresponding prepare/render or close decision. Unmanaged window
events are forwarded too, with window(id) returning None. Raw events are never
implicitly converted into UI actions. user_event receives Message directly.

Initial resumed occurs before window creation. Subsequent resumed callbacks run
after existing targets are restored. suspended runs after targets have been
released; callbacks may repeat. window_created fires once per registration, not
on surface recreation. A managed Destroyed event releases its target and unregisters
its window, delivers the raw event, then fires window_closed without a close veto.
Surface generation and current format are available to
prepare/render after recovery. OS close requests call close_requested; explicit
close_window is an already-decided close and bypasses that question. Lifecycle
callbacks continue during shutdown, with window_closed preceding exiting.

## Dispatch and scheduling contract

For a renderable managed RedrawRequested:

1. Clear the pending native-redraw marker and consume the current dirty request.
2. Forward the native event; drain queued operations/notifications.
3. Recheck that the window exists, is active, has area and can present.
4. Call prepare; drain operations and recheck the window/settings.
5. If preparation requests Render, acquire a frame.
6. Borrow independent WindowInfo fields and call render.
7. On success, notify the native window and finish the frame.
8. Release frame borrows, call submitted, and drain queued operations.
9. Preserve invalidations issued during any callback for the next frame.

Resize and scale-factor notifications update metrics, preserve requested settings,
and invalidate the view. Repeating unchanged metrics does not replace attachments.
Known occlusion, zero size and app suspension pause continuous redraws while
retaining dirty state. Resume/exposure produces a redraw; pending size/settings
are reflected before preparation. An OS redraw or explicit redraw may probe a
previous acquisition-level suspension when native state allows presentation.

OnDemand is default. Continuous requests a next frame only while presentation is
available. request_redraw_at is a one-shot per-window wake deadline: earliest
requests coalesce, later calls cannot postpone an earlier deadline, and callers
reissue future deadlines after a wake. It is a scheduling hint, not a general
timer/animation clock. An immediate redraw does not cancel a pending deadline.
cancel_redraw_at removes that window's deadline explicitly.

A due deadline invalidates its window and is consumed. If unavailable, retain the
dirty flag without spinning on the expired deadline. before-wait processing
requests pending native redraws and sets Wait/WaitUntil to the earliest eligible
deadline/retry. It does not draw directly from about_to_wait or default to Poll.
Default bookkeeping uses O(1) ID lookup and an O(number of windows) deadline scan;
there is no per-draw scheduling lookup or layout work. RenderFormat clones occur
only when compatibility changes; native window queries occur on lifecycle updates.

Retry keeps the window dirty and schedules bounded-frequency acquisition attempts.
Suspended waits for availability/redraw triggers. SurfaceLost drops the old target,
recreates with saved settings, invalidates and schedules a retry; recurring loss
beyond the recovery limit is terminal. Validation, recreation and submission
failures are returned as structured RunError with window/callback context.
Handler errors terminate the runner and retain the original error as a source.
Before finish, any acquired recording is abandoned. Errors in submitted/shutdown
do not roll back earlier successful submissions. The first failure is returned; a shutdown-hook failure
is primary only if no earlier failure exists. Device-loss/resource reconstruction
is an application recovery policy beyond this surface-recovery V1.

## Consumer example

Managed application usage:

```rust,ignore
impl Handler for Demo {
    type Message = ();
    type Error = Box<dyn std::error::Error>;

    fn resumed(&mut self, cx: &mut AppContext<'_, ()>) -> Result<(), Self::Error> {
        if self.window.is_none() {
            self.window = Some(cx.create_window(
                Window::default_attributes().with_title("Astrelis"),
                SurfaceSettings::new(),
            )?);
        }
        Ok(())
    }

    fn window_created(&mut self, cx: &mut AppContext<'_, ()>, id: WindowId)
        -> Result<(), Self::Error> {
        let window = cx.window(id).unwrap();
        self.renderer = Some(MeshRenderer::new(window.graphics()));
        self.mesh = Some(window.graphics().create_mesh(&[
            Vertex::new([0., 0.7, 0.], [1., 0., 0., 1.]),
            Vertex::new([-0.7, -0.7, 0.], [0., 1., 0., 1.]),
            Vertex::new([0.7, -0.7, 0.], [0., 0., 1., 1.]),
        ], &[0, 1, 2])?);
        Ok(())
    }

    fn prepare(&mut self, cx: &mut AppContext<'_, ()>, id: WindowId)
        -> Result<PrepareAction, Self::Error> {
        self.renderer.as_mut().unwrap()
            .prepare(cx.window(id).unwrap().render_format().unwrap())?;
        Ok(PrepareAction::Render)
    }

    fn render(&mut self, _window: WindowInfo<'_>, frame: &mut Frame<'_, 'static>)
        -> Result<(), Self::Error> {
        let mut pass = frame.render_pass().clear_color(wgpu::Color::BLACK).begin()?;
        self.renderer.as_mut().unwrap().draw(&mut pass, self.mesh.as_ref().unwrap())?;
        Ok(())
    }
}

Runner::new()?.run(&mut demo)?;
```

RXUI uses the same callbacks: install native adapters in window_created, translate
events in window_event/user_event, build/prepare a keyed view in prepare, and paint
into an application-created pass in render. There is no automatic UI render pass.

## Implementation order and acceptance

1. Implement WindowContext construction, metrics, settings, suspend/resume and
   recovery; retain direct-winit examples as consumer references.
2. Implement the native runner/context/handler and scheduler; add a standalone mesh
   example and a two-window example with shared resources and independent redraws.
3. RXUI adaptation remains a separate, deferred task.

Verify a borrowed handler remains inspectable after run; Box<dyn Error> handlers
compile; a native AccessKit-compatible user-event proxy needs no envelope adapter;
and WindowInfo/Frame field borrows coexist without unsafe or per-frame cloning.
Test scheduler invalidations issued inside prepare/render, deadline coalescing,
unavailable windows, retry pacing and recovery limits. Native checks must include
duplicate resize/DPI events, zero size, visibility, close veto/queued close, settings
preservation across recovery, 1x/4x MSAA and two windows. Repeated suspend/resume
needs platform-specific checks before claiming mobile support.

Measure a prepared frame against direct-winit/core-Astrelis recording with the same
passes/draws. Warm runner bookkeeping should allocate no per-frame buffers or
pipelines, perform no native size queries and create no extra passes/submissions.
Idle OnDemand must produce no periodic redraws. Report runner bookkeeping separately
from application preparation, recording and OS presentation latency.

## Primary references

- [winit ApplicationHandler](https://docs.rs/winit/0.30.13/winit/application/trait.ApplicationHandler.html):
  resume/suspend rules and redraw event guidance.
- [winit EventLoop](https://docs.rs/winit/0.30.13/winit/event_loop/struct.EventLoop.html):
  borrowed native run_app and native message proxies.
- [winit Window](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.pre_present_notify):
  native window access and notification before presentation.
- [AccessKit winit adapter](https://docs.rs/accesskit_winit/0.30.0/accesskit_winit/struct.Adapter.html):
  hidden-window installation and event-loop proxy integration.

## Validation performed

The workspace passes unit/GPU tests and compiled rustdoc examples. The integration
has eight policy/scheduler regression tests, strict native Clippy checks, and a
wasm library compile check. The wasm check covers low-level/configuration helpers;
the desktop runner/handler/context are intentionally not exported on that target.

A temporary native consumer exercised two shared-device windows, preparation and
submission invalidations, idle OnDemand, Continuous, Skip, deadline coalescing,
hidden/zero-sized windows, duplicate resize events, DPI snapshots, 1x/4x MSAA,
repeated suspension/resume, depth/stencil preservation, and queued closes during
preparation. Separate processes exercised errors in resumed, window_created,
prepare, render, submitted, window_closed, and exiting; original failures survived later shutdown errors,
all registrations received cleanup, and handlers remained inspectable after run.
Recreation was triggered deliberately through WindowContext suspension/resume;
a real backend SurfaceLost or device-loss event was not induced.

The standalone two-window example was checked through native UI interaction:
rendered output, animation/MSAA keys, close veto, closing one window independently,
and last-window shutdown. A compile-only external consumer installs an AccessKit
adapter with the actual typed AppContext proxy and a Box<dyn Error> handler; no
private message envelope or RXUI changes were needed. Native runtime validation
was performed on macOS/Metal, not Windows or Linux.

See [lifecycle performance and measurement boundaries](performance/winit.md) for
matched native prepared-frame timings, isolated scheduling costs, raw artifacts,
and reproduction commands. Library rustdoc documents ownership and errors
independently of the README.
