# astrelis-gpu

Backend-neutral, WebGPU-shaped GPU primitives for Astrelis.

The crate owns stable resource descriptors and handles while backend
implementations live in separate crates. Persistent resources are clonable and
tagged with a device identity; command buffers and presentation frames preserve
consuming semantics.

This is not the display-list renderer. Painting, text, materials, render graphs,
and frame scheduling belong to higher layers.

The API requires callers to poll devices explicitly while mappings, completion
callbacks, or profiling frames are pending.
