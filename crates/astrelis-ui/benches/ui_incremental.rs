//! Benchmarks for incremental UI updates vs full rebuild

use astrelis_ui::{UiCore, WidgetId};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

fn setup() -> UiCore {
    UiCore::new()
}

fn bench_full_rebuild_vs_incremental(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_rebuild_vs_incremental");

    // Simple counter update scenario
    group.bench_function("full_rebuild_counter", |b| {
        let mut ui = setup();
        let mut counter = 0;

        b.iter(|| {
            counter += 1;
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .child(|container| {
                        container
                            .text(format!("Count: {}", counter))
                            .size(24.0)
                            .build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("incremental_update_counter", |b| {
        let mut ui = setup();
        let counter_id = WidgetId::new("counter");
        let mut counter = 0;

        // Initial build
        ui.build(|root| {
            root.container()
                .width(800.0)
                .height(600.0)
                .child(|container| container.text("Count: 0").size(24.0).build())
                .build();
        });
        ui.compute_layout();

        b.iter(|| {
            counter += 1;
            ui.update_text(counter_id, format!("Count: {}", counter));
            ui.compute_layout();
            black_box(())
        });
    });

    group.finish();
}

fn bench_multiple_text_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_text_updates");

    for count in [10, 50, 100] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("full_rebuild", count),
            &count,
            |b, &count| {
                let mut ui = setup();
                b.iter(|| {
                    ui.build(|root| {
                        root.container()
                            .width(800.0)
                            .height(600.0)
                            .child(|container| {
                                for i in 0..count {
                                    container.text(format!("Updated: {}", i)).size(16.0).build();
                                }
                                container.container().build()
                            })
                            .build();
                    });
                    ui.compute_layout();
                    black_box(())
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("incremental_update", count),
            &count,
            |b, &count| {
                let mut ui = setup();
                let ids: Vec<WidgetId> = (0..count)
                    .map(|i| WidgetId::new(&format!("text_{}", i)))
                    .collect();

                // Initial build
                ui.build(|root| {
                    root.container()
                        .width(800.0)
                        .height(600.0)
                        .child(|container| {
                            for i in 0..count {
                                container.text(format!("Initial: {}", i)).size(16.0).build();
                            }
                            container.container().build()
                        })
                        .build();
                });
                ui.compute_layout();

                b.iter(|| {
                    for (i, id) in ids.iter().enumerate() {
                        ui.update_text(*id, format!("Updated: {}", i));
                    }
                    ui.compute_layout();
                    black_box(())
                });
            },
        );
    }

    group.finish();
}

fn bench_partial_tree_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("partial_tree_update");

    group.bench_function("rebuild_single_node", |b| {
        let mut ui = setup();
        let mut counter = 0;

        // Initial complex tree
        ui.build(|root| {
            root.container()
                .width(800.0)
                .height(600.0)
                .child(|main| {
                    for i in 0..20 {
                        main.container()
                            .width(800.0)
                            .child(|item| {
                                if i == 10 {
                                    item.text("Dynamic: 0").size(16.0).build()
                                } else {
                                    item.text(format!("Static: {}", i)).size(16.0).build()
                                }
                            })
                            .build();
                    }
                    main.container().build()
                })
                .build();
        });
        ui.compute_layout();

        b.iter(|| {
            counter += 1;
            ui.update_text(WidgetId::new("dynamic"), format!("Dynamic: {}", counter));
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("full_tree_rebuild", |b| {
        let mut ui = setup();
        let mut counter = 0;

        b.iter(|| {
            counter += 1;
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .child(|main| {
                        for i in 0..20 {
                            main.container()
                                .width(800.0)
                                .child(|item| {
                                    if i == 10 {
                                        item.text(format!("Dynamic: {}", counter))
                                            .size(16.0)
                                            .build()
                                    } else {
                                        item.text(format!("Static: {}", i)).size(16.0).build()
                                    }
                                })
                                .build();
                        }
                        main.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.finish();
}

fn bench_realistic_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_scenarios");

    group.bench_function("dashboard_fps_update", |b| {
        let mut ui = setup();
        let fps_id = WidgetId::new("fps");
        let mut fps = 60.0;

        // Initial dashboard
        ui.build(|root| {
            root.container()
                .width(1200.0)
                .height(800.0)
                .padding(20.0)
                .child(|main| {
                    // Header
                    main.container()
                        .width(1160.0)
                        .height(60.0)
                        .child(|header| header.text("Dashboard").size(24.0).bold().build())
                        .build();

                    // Content
                    main.container()
                        .width(1160.0)
                        .height(680.0)
                        .child(|content| {
                            for i in 0..10 {
                                content
                                    .container()
                                    .width(1160.0)
                                    .padding(10.0)
                                    .child(|card| {
                                        card.text(format!("Card {}", i)).size(18.0).build()
                                    })
                                    .build();
                            }
                            content.container().build()
                        })
                        .build();

                    // Status bar with FPS
                    main.container()
                        .width(1160.0)
                        .height(30.0)
                        .child(|status| status.text("FPS: 60.0").size(14.0).build())
                        .build();

                    main.container().build()
                })
                .build();
        });
        ui.compute_layout();

        b.iter(|| {
            fps = 59.0 + (fps % 2.0);
            ui.update_text(fps_id, format!("FPS: {:.1}", fps));
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("dashboard_full_rebuild", |b| {
        let mut ui = setup();
        let mut fps = 60.0;

        b.iter(|| {
            fps = 59.0 + (fps % 2.0);
            ui.build(|root| {
                root.container()
                    .width(1200.0)
                    .height(800.0)
                    .padding(20.0)
                    .child(|main| {
                        main.container()
                            .width(1160.0)
                            .height(60.0)
                            .child(|header| header.text("Dashboard").size(24.0).bold().build())
                            .build();

                        main.container()
                            .width(1160.0)
                            .height(680.0)
                            .child(|content| {
                                for i in 0..10 {
                                    content
                                        .container()
                                        .width(1160.0)
                                        .padding(10.0)
                                        .child(|card| {
                                            card.text(format!("Card {}", i)).size(18.0).build()
                                        })
                                        .build();
                                }
                                content.container().build()
                            })
                            .build();

                        main.container()
                            .width(1160.0)
                            .height(30.0)
                            .child(|status| {
                                status.text(format!("FPS: {:.1}", fps)).size(14.0).build()
                            })
                            .build();

                        main.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.finish();
}

fn bench_metrics_dashboard(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_dashboard");

    group.bench_function("update_all_metrics", |b| {
        let mut ui = setup();
        let metric_ids: Vec<WidgetId> = (0..6)
            .map(|i| WidgetId::new(&format!("metric_{}", i)))
            .collect();
        let mut counter = 0;

        // Initial dashboard with metrics
        ui.build(|root| {
            root.container()
                .width(1200.0)
                .height(800.0)
                .padding(20.0)
                .child(|main| {
                    main.container()
                        .width(1160.0)
                        .height(60.0)
                        .child(|header| header.text("Dashboard").size(24.0).bold().build())
                        .build();

                    main.container()
                        .width(1160.0)
                        .height(700.0)
                        .gap(20.0)
                        .child(|grid| {
                            for i in 0..6 {
                                grid.container()
                                    .width(360.0)
                                    .padding(20.0)
                                    .child(|card| {
                                        card.text(format!("Metric {}", i)).size(16.0).build();
                                        card.text(format!("Value: {}", i * 100))
                                            .size(32.0)
                                            .bold()
                                            .build();
                                        card.container().build()
                                    })
                                    .build();
                            }
                            grid.container().build()
                        })
                        .build();

                    main.container().build()
                })
                .build();
        });
        ui.compute_layout();

        b.iter(|| {
            counter += 1;
            for (i, id) in metric_ids.iter().enumerate() {
                ui.update_text(*id, format!("Value: {}", i * 100 + counter));
            }
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("full_dashboard_rebuild", |b| {
        let mut ui = setup();
        let mut counter = 0;

        b.iter(|| {
            counter += 1;
            ui.build(|root| {
                root.container()
                    .width(1200.0)
                    .height(800.0)
                    .padding(20.0)
                    .child(|main| {
                        main.container()
                            .width(1160.0)
                            .height(60.0)
                            .child(|header| header.text("Dashboard").size(24.0).bold().build())
                            .build();

                        main.container()
                            .width(1160.0)
                            .height(700.0)
                            .gap(20.0)
                            .child(|grid| {
                                for i in 0..6 {
                                    grid.container()
                                        .width(360.0)
                                        .padding(20.0)
                                        .child(|card| {
                                            card.text(format!("Metric {}", i)).size(16.0).build();
                                            card.text(format!("Value: {}", i * 100 + counter))
                                                .size(32.0)
                                                .bold()
                                                .build();
                                            card.container().build()
                                        })
                                        .build();
                                }
                                grid.container().build()
                            })
                            .build();

                        main.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_full_rebuild_vs_incremental,
    bench_multiple_text_updates,
    bench_partial_tree_update,
    bench_realistic_scenarios,
    bench_metrics_dashboard
);
criterion_main!(benches);
