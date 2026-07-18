//! Benchmarks for the retained UI core's per-frame hot paths.
//!
//! These establish the Milestone 16 baseline that Milestone 17 measures
//! against. Everything here drives the public API only, so the benchmarks stay
//! valid across the module split and across the internal rewrites that follow.
//!
//! `ensure_layout`, `prepare_text_layouts`, and `hit_test` are private, so
//! costs are attributed by differencing rather than by calling them directly:
//!
//! - `layout` calls `layout_bounds`, which runs layout and stops before paint;
//!   `display_list` runs layout and paint. The gap between them is paint.
//! - `text_heavy` and `text_free` build the same node count with and without
//!   text. The gap between them is shaping.
//! - `warm` repeats a phase with no intervening mutation, so it measures the
//!   dirty-flag early-out rather than the work itself.

use astrelis_core::geometry::{LogicalPoint, LogicalSize};
use astrelis_text::FontDatabase;
use astrelis_ui_core::{ElementHandle, Label, LayoutStyle, Length, Theme, Ui};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// Node count for every fixture. Large enough that per-node costs dominate
/// fixed overhead, small enough to stay a plausible screenful of UI.
const NODES: usize = 1_000;

const VIEWPORT: LogicalSize = LogicalSize::new(1280.0, 720.0);

/// A column of labels: exercises layout and text shaping together.
fn text_heavy(count: usize) -> (Ui, Vec<ElementHandle<Label>>) {
    let mut ui = Ui::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(VIEWPORT, 1.0);
    let root = ui.root();
    let column = ui.add_column(root).unwrap();
    let mut labels = Vec::with_capacity(count);
    for index in 0..count {
        labels.push(ui.add_label(column, format!("Item {index}")).unwrap());
    }
    (ui, labels)
}

/// The same node count with no text at all: isolates layout from shaping.
fn text_free(count: usize) -> Ui {
    let mut ui = Ui::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(VIEWPORT, 1.0);
    let root = ui.root();
    let column = ui.add_column(root).unwrap();
    for _ in 0..count {
        let child = ui.add_column(column).unwrap();
        ui.set_layout(
            child,
            LayoutStyle {
                width: Length::Px(120.0),
                height: Length::Px(18.0),
                ..Default::default()
            },
        )
        .unwrap();
    }
    ui
}

/// Forces a full reflow the way a window resize does. Alternating by one
/// logical pixel invalidates layout without meaningfully changing its cost.
fn resize(ui: &mut Ui, even: bool) {
    let width = if even {
        VIEWPORT.width
    } else {
        VIEWPORT.width - 1.0
    };
    ui.set_viewport(LogicalSize::new(width, VIEWPORT.height), 1.0);
}

fn bench_layout(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("layout");

    let (mut ui, _labels) = text_heavy(NODES);
    let root = ui.root();
    let mut even = false;
    group.bench_function("resize/text_heavy", |bencher| {
        bencher.iter(|| {
            even = !even;
            resize(&mut ui, even);
            black_box(ui.layout_bounds(root).unwrap())
        })
    });

    let mut ui = text_free(NODES);
    let root = ui.root();
    let mut even = false;
    group.bench_function("resize/text_free", |bencher| {
        bencher.iter(|| {
            even = !even;
            resize(&mut ui, even);
            black_box(ui.layout_bounds(root).unwrap())
        })
    });

    // No mutation between iterations: measures the dirty-flag early-out.
    let (mut ui, _labels) = text_heavy(NODES);
    let root = ui.root();
    group.bench_function("warm/text_heavy", |bencher| {
        bencher.iter(|| black_box(ui.layout_bounds(root).unwrap()))
    });

    group.finish();
}

fn bench_display_list(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("display_list");

    let (mut ui, _labels) = text_heavy(NODES);
    let mut even = false;
    group.bench_function("resize/text_heavy", |bencher| {
        bencher.iter(|| {
            even = !even;
            resize(&mut ui, even);
            black_box(ui.display_list().unwrap())
        })
    });

    let mut ui = text_free(NODES);
    let mut even = false;
    group.bench_function("resize/text_free", |bencher| {
        bencher.iter(|| {
            even = !even;
            resize(&mut ui, even);
            black_box(ui.display_list().unwrap())
        })
    });

    let (mut ui, _labels) = text_heavy(NODES);
    group.bench_function("warm/text_heavy", |bencher| {
        bencher.iter(|| black_box(ui.display_list().unwrap()))
    });

    group.finish();
}

/// The headline incremental case: one label changes out of `NODES`.
///
/// `set_label_text` now enqueues only the changed node, so the measure-input
/// sweeps (text shaping, Taffy style reconciliation) revisit one node instead
/// of all `NODES`. The `layout` variant isolates that: it sits below
/// `layout/resize/text_heavy` (a full reflow) by the shaping cost of the
/// untouched labels, bounded below by `layout/resize/text_free` — the whole-tree
/// `assign_layout` position cascade and `measure_map` are still O(nodes) and are
/// a later step. The `display_list` variant additionally repaints the whole
/// tree, since per-node paint invalidation is also still to come.
fn bench_incremental(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("incremental");

    // Layout-only: the measure/shaping path that node-granular dirtying speeds
    // up, with paint excluded.
    let (mut ui, labels) = text_heavy(NODES);
    let root = ui.root();
    let target = labels[NODES / 2];
    let mut even = false;
    group.bench_function("set_label_text/layout", |bencher| {
        bencher.iter(|| {
            even = !even;
            // Alternate the value: set_static_text early-outs if it is equal.
            let text = if even { "Item flip" } else { "Item flop" };
            ui.set_label_text(target, text).unwrap();
            black_box(ui.layout_bounds(root).unwrap())
        })
    });

    let (mut ui, labels) = text_heavy(NODES);
    let target = labels[NODES / 2];
    let mut even = false;
    group.bench_function("set_label_text/display_list", |bencher| {
        bencher.iter(|| {
            even = !even;
            let text = if even { "Item flip" } else { "Item flop" };
            ui.set_label_text(target, text).unwrap();
            black_box(ui.display_list().unwrap())
        })
    });

    group.finish();
}

fn bench_hit_test(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("hit_test");

    // Layout is settled and never invalidated, so this measures the pointer
    // path alone: the per-node child allocation and z-order sort.
    let (mut ui, _labels) = text_heavy(NODES);
    ui.display_list().unwrap();
    let mut step = 0.0f32;
    group.bench_function("pointer_move/text_heavy", |bencher| {
        bencher.iter(|| {
            step = (step + 1.0) % VIEWPORT.height;
            black_box(ui.hit_test_at(LogicalPoint::new(40.0, step)).unwrap())
        })
    });

    group.finish();
}

fn bench_semantics(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("semantic_tree");

    let (mut ui, _labels) = text_heavy(NODES);
    ui.display_list().unwrap();
    group.bench_function("warm/text_heavy", |bencher| {
        bencher.iter(|| black_box(ui.semantic_tree().unwrap()))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_layout,
    bench_display_list,
    bench_incremental,
    bench_hit_test,
    bench_semantics
);
criterion_main!(benches);
