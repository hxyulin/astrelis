//! Benchmarks for UI tree operations

use astrelis_render::Color;
use astrelis_ui::{UiCore, WidgetId};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

fn setup() -> UiCore {
    UiCore::new()
}

fn bench_tree_build_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_build_simple");

    for count in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut ui = setup();
            b.iter(|| {
                ui.build(|root| {
                    root.container()
                        .width(800.0)
                        .height(600.0)
                        .child(|container| {
                            for i in 0..count {
                                container.text(format!("Item {}", i)).size(16.0).build();
                            }
                            container.container().build()
                        })
                        .build();
                });
                ui.compute_layout();
                black_box(())
            });
        });
    }

    group.finish();
}

fn bench_tree_build_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_build_nested");

    for depth in [2, 5, 10, 20] {
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            let mut ui = setup();
            b.iter(|| {
                ui.build(|root| {
                    // Build multi-level structure to simulate nesting
                    root.container()
                        .width(800.0)
                        .height(600.0)
                        .child(|level1| {
                            for i in 0..depth {
                                level1
                                    .container()
                                    .width(750.0 - i as f32 * 30.0)
                                    .child(|level2| {
                                        level2.text(format!("Nested item {}", i)).size(16.0).build()
                                    })
                                    .build();
                            }
                            level1.container().build()
                        })
                        .build();
                });
                ui.compute_layout();
                black_box(())
            });
        });
    }

    group.finish();
}

fn bench_tree_build_complex_ui(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_build_complex_ui");

    group.bench_function("menu_ui", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .padding(20.0)
                    .child(|main| {
                        // Header
                        main.container()
                            .width(800.0)
                            .height(60.0)
                            .child(|header| {
                                header.text("Application Menu").size(24.0).bold().build()
                            })
                            .build();

                        // Menu items
                        main.container()
                            .width(800.0)
                            .height(400.0)
                            .gap(10.0)
                            .child(|menu| {
                                for i in 0..10 {
                                    menu.container()
                                        .width(800.0)
                                        .height(50.0)
                                        .child(|item| {
                                            item.text(format!("Menu Item {}", i)).size(16.0).build()
                                        })
                                        .build();
                                }
                                menu.container().build()
                            })
                            .build();

                        // Footer
                        main.container()
                            .width(800.0)
                            .height(40.0)
                            .child(|footer| footer.text("Status: Ready").size(12.0).build())
                            .build();

                        main.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("form_ui", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(400.0)
                    .padding(20.0)
                    .gap(15.0)
                    .child(|form| {
                        // Title
                        form.text("User Form").size(20.0).bold().build();

                        // Fields
                        for field in ["Name", "Email", "Password", "Confirm Password"] {
                            form.container()
                                .width(400.0)
                                .gap(10.0)
                                .child(|row| {
                                    row.text(format!("{}:", field)).size(14.0).build();
                                    row.text_input("").width(200.0).build();
                                    row.container().build()
                                })
                                .build();
                        }

                        // Buttons
                        form.container()
                            .width(400.0)
                            .gap(10.0)
                            .child(|buttons| {
                                buttons.button("Submit").build();
                                buttons.button("Cancel").build();
                                buttons.container().build()
                            })
                            .build();

                        form.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.finish();
}

fn bench_tree_node_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_node_lookup");

    for count in [100, 500, 1000] {
        group.throughput(Throughput::Elements(count as u64));

        let mut ui = setup();
        let ids: Vec<WidgetId> = (0..count)
            .map(|i| WidgetId::new(&format!("widget_{}", i)))
            .collect();

        ui.build(|root| {
            root.container()
                .child(|container| {
                    for i in 0..count {
                        container.text(format!("Text {}", i)).size(16.0).build();
                    }
                    container.container().build()
                })
                .build();
        });

        group.bench_with_input(BenchmarkId::from_parameter(count), &ids, |b, ids| {
            b.iter(|| {
                for id in ids {
                    black_box(ui.get_node_id(*id));
                }
            });
        });
    }

    group.finish();
}

fn bench_tree_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_traversal");

    for count in [50, 100, 500] {
        let mut ui = setup();
        ui.build(|root| {
            root.container()
                .child(|container| {
                    for i in 0..count {
                        container.text(format!("Item {}", i)).size(16.0).build();
                    }
                    container.container().build()
                })
                .build();
        });

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                let tree = ui.tree();
                black_box(tree)
            });
        });
    }

    group.finish();
}

fn bench_widget_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_creation");

    group.bench_function("text_widget", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(root.text("Sample text").size(16.0).build());
            });
        });
    });

    group.bench_function("button_widget", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(root.button("Click me").build());
            });
        });
    });

    group.bench_function("container_widget", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(root.container().width(200.0).height(100.0).build());
            });
        });
    });

    group.bench_function("text_input_widget", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(root.text_input("Placeholder").width(200.0).build());
            });
        });
    });

    group.finish();
}

fn bench_tree_modification(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_modification");

    group.bench_function("add_child", |b| {
        let mut ui = setup();
        ui.build(|root| {
            root.container().width(800.0).height(600.0).build();
        });

        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .child(|container| {
                        for i in 0..10 {
                            container.text(format!("Item {}", i)).size(16.0).build();
                        }
                        container.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.finish();
}

fn bench_widget_styles(c: &mut Criterion) {
    let mut group = c.benchmark_group("widget_styles");

    group.bench_function("plain_text", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(root.text("Plain text").size(16.0).build());
            });
        });
    });

    group.bench_function("styled_text", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(
                    root.text("Styled text")
                        .size(18.0)
                        .color(Color::rgba(1.0, 0.5, 0.0, 1.0))
                        .bold()
                        .build(),
                );
            });
        });
    });

    group.bench_function("styled_container", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                black_box(
                    root.container()
                        .width(200.0)
                        .height(100.0)
                        .padding(10.0)
                        .margin(5.0)
                        .background_color(Color::rgba(0.2, 0.2, 0.2, 1.0))
                        .build(),
                );
            });
        });
    });

    group.finish();
}

fn bench_tree_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_memory_usage");

    for count in [100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut ui = setup();
                ui.build(|root| {
                    root.container()
                        .child(|container| {
                            for i in 0..count {
                                container.text(format!("Item {}", i)).size(16.0).build();
                            }
                            container.container().build()
                        })
                        .build();
                });
                black_box(ui)
            });
        });
    }

    group.finish();
}

fn bench_realistic_dashboard(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_dashboard");

    group.bench_function("dashboard_full", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(1200.0)
                    .height(800.0)
                    .padding(20.0)
                    .gap(10.0)
                    .child(|main| {
                        // Top bar
                        main.container()
                            .width(1200.0)
                            .height(60.0)
                            .gap(20.0)
                            .child(|top_bar| {
                                top_bar.text("Dashboard").size(24.0).bold().build();
                                top_bar.button("Settings").build();
                                top_bar.button("Logout").build();
                                top_bar.container().build()
                            })
                            .build();

                        // Main content area
                        main.container()
                            .width(1200.0)
                            .height(680.0)
                            .gap(20.0)
                            .child(|content| {
                                // Left sidebar
                                content
                                    .container()
                                    .width(250.0)
                                    .gap(10.0)
                                    .child(|sidebar| {
                                        for item in [
                                            "Home", "Projects", "Tasks", "Team", "Calendar",
                                            "Reports", "Settings",
                                        ] {
                                            sidebar.button(item).build();
                                        }
                                        sidebar.container().build()
                                    })
                                    .build();

                                // Main panel with cards
                                content
                                    .container()
                                    .width(900.0)
                                    .gap(15.0)
                                    .child(|panel| {
                                        for i in 0..6 {
                                            panel
                                                .container()
                                                .width(900.0)
                                                .padding(15.0)
                                                .child(|card| {
                                                    card.text(format!("Card Title {}", i))
                                                        .size(18.0)
                                                        .bold()
                                                        .build();
                                                    card.text(
                                                        "Card description with some text content",
                                                    )
                                                    .size(14.0)
                                                    .build();
                                                    card.container()
                                                        .gap(10.0)
                                                        .child(|actions| {
                                                            actions.button("View").build();
                                                            actions.button("Edit").build();
                                                            actions.container().build()
                                                        })
                                                        .build();
                                                    card.container().build()
                                                })
                                                .build();
                                        }
                                        panel.container().build()
                                    })
                                    .build();

                                content.container().build()
                            })
                            .build();

                        // Status bar
                        main.container()
                            .width(1200.0)
                            .height(30.0)
                            .child(|status| {
                                status
                                    .text("Status: Connected | Users: 42 | FPS: 60")
                                    .size(12.0)
                                    .build()
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
    bench_tree_build_simple,
    bench_tree_build_nested,
    bench_tree_build_complex_ui,
    bench_tree_node_lookup,
    bench_tree_traversal,
    bench_widget_creation,
    bench_tree_modification,
    bench_widget_styles,
    bench_tree_memory_usage,
    bench_realistic_dashboard
);
criterion_main!(benches);
