//! Incremental retained-core microbenchmarks.

use astrelis_core::{color::Color, geometry::LogicalSize};
use astrelis_ui_next::{Axis, Flex, Invalidation, Label, UiRoot};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn ui(count: usize) -> (UiRoot, astrelis_ui_next::NodeHandle<Label>) {
    let mut ui = UiRoot::new(
        Flex {
            axis: Axis::Vertical,
            ..Flex::default()
        },
        LogicalSize::new(1280.0, 720.0),
    );
    let root = ui.root();
    let mut target = None;
    for index in 0..count {
        let handle = ui
            .append(
                root,
                Label::new(format!("Item {index}")).with_color(Color::WHITE),
            )
            .unwrap();
        if index == count / 2 {
            target = Some(handle);
        }
    }
    ui.update_passes().unwrap();
    (ui, target.unwrap())
}

fn incremental(criterion: &mut Criterion) {
    let (mut ui, target) = ui(1_000);
    let mut flip = false;
    criterion.bench_function("ui_next/label_update_1000", |bencher| {
        bencher.iter(|| {
            flip = !flip;
            ui.update(target, Invalidation::LAYOUT_ALL, |label| {
                label.text = if flip { "Item flip" } else { "Item flop" }.into();
            })
            .unwrap();
            black_box(ui.update_passes().unwrap().stats);
        });
    });
}

criterion_group!(benches, incremental);
criterion_main!(benches);
