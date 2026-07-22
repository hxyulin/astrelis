# astrelis-ui-host

Cross-platform hosting for retained Astrelis UI trees. Native window creation
initializes the GPU synchronously; `wasm32-unknown-unknown` starts WebGPU
initialization asynchronously and exposes `HostStatus` while the page event
loop remains responsive. Hosts cloned from one `GraphicsContext` share its
adapter, device, and queue. UI-only and compositor-backed scene frames share
surface recovery, device-loss reporting, resize handling, and idle-aware
invalidation. An optional `AccessibilityAdapter` receives semantic-tree
updates and routes platform accessibility actions through the same retained
action path used by deterministic tests.
