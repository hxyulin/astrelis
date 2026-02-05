//! Benchmarks for fine-grained dirty flag system.
//!
//! Measures the performance of different dirty flag scenarios:
//! - Color-only updates (no layout)
//! - Text-only updates
//! - Layout updates
//! - Mixed updates

use astrelis_render::Color;
use astrelis_ui::{DirtyFlags, UiCore, tree::NodeId};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

fn setup_ui_core() -> UiCore {
    UiCore::new()
}

fn build_test_tree(ui: &mut UiCore, node_count: usize) {
    ui.build(|root| {
        root.column()
            .width(800.0)
            .height(600.0)
            .gap(5.0)
            .child(|col| {
                for i in 0..node_count {
                    col.text(format!("Item {}", i))
                        .size(16.0)
                        .color(Color::WHITE)
                        .build();
                }
                col.container().build()
            })
            .build();
    });
}

fn bench_color_only_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/color_only");

    for node_count in [10, 50, 100, 200] {
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            &node_count,
            |b, &count| {
                let mut ui = setup_ui_core();
                build_test_tree(&mut ui, count);

                // Initial layout
                ui.compute_layout();

                b.iter(|| {
                    // Color-only update should skip layout and text shaping
                    ui.tree_mut().mark_dirty_flags(NodeId(1), DirtyFlags::COLOR);

                    ui.compute_layout();
                });
            },
        );
    }

    group.finish();
}

fn bench_text_only_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/text_only");

    for node_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            &node_count,
            |b, &count| {
                let mut ui = setup_ui_core();
                build_test_tree(&mut ui, count);

                // Initial layout
                ui.compute_layout();

                b.iter(|| {
                    // Text update requires shaping but may skip full layout
                    ui.tree_mut()
                        .mark_dirty_flags(NodeId(1), DirtyFlags::TEXT_SHAPING);

                    ui.compute_layout();
                });
            },
        );
    }

    group.finish();
}

fn bench_layout_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/layout");

    for node_count in [10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            &node_count,
            |b, &count| {
                let mut ui = setup_ui_core();
                build_test_tree(&mut ui, count);

                // Initial layout
                ui.compute_layout();

                b.iter(|| {
                    // Layout update requires full recomputation
                    ui.tree_mut()
                        .mark_dirty_flags(NodeId(1), DirtyFlags::LAYOUT);

                    ui.compute_layout();
                });
            },
        );
    }

    group.finish();
}

fn bench_mixed_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/mixed");

    group.bench_function("paint_and_layout", |b| {
        let mut ui = setup_ui_core();
        build_test_tree(&mut ui, 50);
        ui.compute_layout();

        b.iter(|| {
            // Mix of paint-only and layout updates
            ui.tree_mut().mark_dirty_flags(NodeId(1), DirtyFlags::COLOR);
            ui.tree_mut()
                .mark_dirty_flags(NodeId(5), DirtyFlags::LAYOUT);
            ui.tree_mut()
                .mark_dirty_flags(NodeId(10), DirtyFlags::GEOMETRY);

            ui.compute_layout();
        });
    });

    group.finish();
}

fn bench_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/propagation");

    group.bench_function("deep_tree", |b| {
        let mut ui = setup_ui_core();

        // Build deep nested tree
        ui.build(|root| {
            root.column()
                .width(800.0)
                .height(600.0)
                .child(|c1| {
                    c1.column()
                        .padding(5.0)
                        .child(|c2| {
                            c2.column()
                                .padding(5.0)
                                .child(|c3| {
                                    c3.column()
                                        .padding(5.0)
                                        .child(|c4| {
                                            c4.column()
                                                .padding(5.0)
                                                .child(|c5| {
                                                    c5.column()
                                                        .padding(5.0)
                                                        .child(|c6| {
                                                            c6.text("Deep leaf node")
                                                                .color(Color::WHITE)
                                                                .build();
                                                            c6.container().build()
                                                        })
                                                        .build()
                                                })
                                                .build()
                                        })
                                        .build()
                                })
                                .build()
                        })
                        .build()
                })
                .build();
        });

        ui.compute_layout();

        b.iter(|| {
            // Mark deep leaf dirty and measure propagation cost
            ui.tree_mut()
                .mark_dirty_flags(NodeId(20), DirtyFlags::LAYOUT);

            ui.compute_layout();
        });
    });

    group.finish();
}

fn bench_selective_compute(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/selective_compute");

    group.bench_function("skip_clean_nodes", |b| {
        let mut ui = setup_ui_core();
        build_test_tree(&mut ui, 100);
        ui.compute_layout();

        b.iter(|| {
            // Mark only one node dirty
            ui.tree_mut()
                .mark_dirty_flags(NodeId(50), DirtyFlags::COLOR);

            // Should skip most nodes
            ui.compute_layout();
        });
    });

    group.bench_function("all_dirty", |b| {
        let mut ui = setup_ui_core();
        build_test_tree(&mut ui, 100);

        b.iter(|| {
            // Mark all nodes dirty
            for i in 0..100 {
                ui.tree_mut()
                    .mark_dirty_flags(NodeId(i), DirtyFlags::LAYOUT);
            }

            ui.compute_layout();
        });
    });

    group.finish();
}

fn bench_metrics_collection(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirty_flags/metrics");

    group.bench_function("with_instrumentation", |b| {
        let mut ui = setup_ui_core();
        build_test_tree(&mut ui, 50);

        b.iter(|| {
            ui.tree_mut()
                .mark_dirty_flags(NodeId(1), DirtyFlags::LAYOUT);

            let metrics = ui.compute_layout_instrumented();

            black_box(metrics);
        });
    });

    group.bench_function("without_instrumentation", |b| {
        let mut ui = setup_ui_core();
        build_test_tree(&mut ui, 50);

        b.iter(|| {
            ui.tree_mut()
                .mark_dirty_flags(NodeId(1), DirtyFlags::LAYOUT);

            ui.compute_layout();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_color_only_update,
    bench_text_only_update,
    bench_layout_update,
    bench_mixed_updates,
    bench_propagation,
    bench_selective_compute,
    bench_metrics_collection,
);

criterion_main!(benches);
