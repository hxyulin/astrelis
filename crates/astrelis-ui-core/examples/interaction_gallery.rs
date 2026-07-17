//! Milestone 10 interaction gallery, shared with the cross-platform settings example.

#[path = "settings_window.rs"]
mod gallery;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    gallery::main().expect("interaction gallery failed");
    #[cfg(target_arch = "wasm32")]
    gallery::main();
}
