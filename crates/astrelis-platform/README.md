# astrelis-platform

Backend-neutral desktop platform API for Astrelis.

Applications render only after `WindowEvent::RedrawRequested`. A desktop
application normally follows:

```text
event/invalidation → request affected redraws → Wait/WaitUntil
```

A continuously updating game normally follows:

```text
new_events/about_to_wait → update → request active redraws → Poll/WaitUntil
```

IME must be enabled with `Window::set_ime_allowed` while accepting text.
`Window::set_ime_cursor_area` positions the native candidate window next to
the active text caret.
