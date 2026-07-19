//! Headless walk-through of opt-in background (asynchronous) text reshaping.
//!
//! Shaping — BiDi, itemization, font fallback, kerning — is the expensive half
//! of text layout. By default Astrelis shapes inline on the layout pass, which
//! is deterministic but puts that cost on the per-frame critical path. Calling
//! [`Ui::enable_async_shaping`] opts a `Ui` into offloading eligible reshapes
//! to a background worker: the previous `TextLayout` stays on screen (so nothing
//! reflows mid-flight) until the worker delivers the new one.
//!
//! This example drives frames by hand and blocks on [`Ui::flush_async`] so the
//! behaviour is reproducible in a terminal. A real GUI never blocks: it calls
//! [`Ui::poll_async`] on the next frame after the worker's wake fires (see the
//! `settings_window` example, which wires the wake to an event-loop proxy).

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use astrelis_core::geometry::Size;
use astrelis_text::{FontDatabase, FontFamily};
use astrelis_ui_core::{LayoutStyle, Length, Theme, Ui};

const NOTO_SANS: &[u8] = include_bytes!("../assets/NotoSans.ttf");

/// Builds the font database. The main thread and the worker each call this to
/// get their *own* database; because both register the same blob into an empty
/// collection, they shape byte-identically, so text never shifts when a node
/// moves between the synchronous and asynchronous paths.
fn fonts() -> FontDatabase {
    let mut fonts = FontDatabase::empty();
    fonts
        .register_font(Arc::<[u8]>::from(NOTO_SANS))
        .expect("Noto Sans should register");
    fonts
}

fn main() {
    let theme = Theme {
        font_families: vec![FontFamily::Named("Noto Sans".into())],
        ..Default::default()
    };
    let mut ui: Ui = Ui::new(fonts(), theme);

    // Opt in. The worker builds its own identical font database via the factory
    // we pass. Its wake would normally schedule a redraw; here we drive frames
    // manually, so the wake just records that a result became ready.
    let wakes = Arc::new(AtomicUsize::new(0));
    let worker_wakes = Arc::clone(&wakes);
    ui.enable_async_shaping(fonts, move || {
        worker_wakes.fetch_add(1, Ordering::Relaxed);
    });

    ui.set_viewport(Size::new(640.0, 480.0), 1.0);
    let root = ui.root();
    // A wrapping label bounded to 200px: a longer string breaks onto more
    // lines, so the reshape is visible as a change in height (the label's
    // width just tracks its 200px cap).
    let label = ui.add_label(root, "short label").expect("add label");
    ui.set_layout(
        label,
        LayoutStyle {
            max_width: Length::Px(200.0),
            ..Default::default()
        },
    )
    .expect("set layout");
    ui.set_wrap(label, true).expect("enable wrap");

    // First layout: the label has never been shaped, so there is no previous
    // extent to keep on screen — it is shaped synchronously this pass.
    let initial = ui.layout_bounds(label).expect("layout");
    println!(
        "1. first shape (synchronous): height {:.1} ({:.0}px wide)",
        initial.size.height, initial.size.width
    );

    // Now change the text to something that wraps onto several lines. The label
    // already has a layout, so this reshape is eligible for the worker: the
    // layout pass enqueues it and returns with the OLD extent still in place —
    // the frame is not blocked on shaping.
    ui.set_label_text(
        label,
        "a considerably longer label that wraps onto several lines once reshaped",
    )
    .expect("set label text");
    let while_pending = ui.layout_bounds(label).expect("layout");
    println!(
        "2. text changed, worker reshaping (old extent kept): height {:.1}",
        while_pending.size.height
    );
    assert_eq!(
        while_pending.size, initial.size,
        "layout must not reflow until the worker result is applied",
    );

    // Block until the worker delivers the new layout. A GUI would instead call
    // poll_async() on the next frame after the wake fired.
    let changed = ui.flush_async();
    let settled = ui.layout_bounds(label).expect("layout");
    println!(
        "3. after flush_async (worker result applied): height {:.1}",
        settled.size.height
    );
    assert!(
        settled.size.height > initial.size.height,
        "the wrapped string must be taller once applied",
    );

    println!(
        "\nlayout changed by the worker: {changed}; wake fired {} time(s).",
        wakes.load(Ordering::Relaxed)
    );
    println!(
        "The layout pass never blocked on shaping the longer string — that work \
         ran on the worker while the previous extent stayed on screen."
    );
}
