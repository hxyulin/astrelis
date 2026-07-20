# astrelis-ui-host

Cross-platform hosting for one retained Astrelis UI tree. Native window
creation initializes the GPU synchronously; `wasm32-unknown-unknown` starts
WebGPU initialization asynchronously and exposes `HostStatus` while the page
event loop remains responsive. UI-only and compositor-backed scene frames share
surface recovery, resize handling, and idle-aware invalidation.
