//! Benchmarks for UI layout computation

use astrelis_ui::UiCore;
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

fn bench_flexbox_layouts(c: &mut Criterion) {
    let mut group = c.benchmark_group("flexbox_layouts");

    group.bench_function("row_layout", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(100.0)
                    .flex_direction(astrelis_ui::FlexDirection::Row)
                    .gap(10.0)
                    .child(|container| {
                        for i in 0..20 {
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

    group.bench_function("column_layout", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(200.0)
                    .height(600.0)
                    .flex_direction(astrelis_ui::FlexDirection::Column)
                    .gap(5.0)
                    .child(|container| {
                        for i in 0..30 {
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

fn bench_nested_containers(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_containers");

    group.bench_function("nested_rows_columns", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .flex_direction(astrelis_ui::FlexDirection::Column)
                    .gap(10.0)
                    .child(|main| {
                        for _row in 0..10 {
                            main.container()
                                .width(800.0)
                                .flex_direction(astrelis_ui::FlexDirection::Row)
                                .gap(5.0)
                                .child(|row| {
                                    for i in 0..5 {
                                        row.text(format!("Item {}", i)).size(16.0).build();
                                    }
                                    row.container().build()
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

fn bench_sizing_constraints(c: &mut Criterion) {
    let mut group = c.benchmark_group("sizing_constraints");

    group.bench_function("fixed_sizes", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .child(|container| {
                        for i in 0..50 {
                            container
                                .text(format!("Item {}", i))
                                .size(16.0)
                                .width(200.0)
                                .height(30.0)
                                .build();
                        }
                        container.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("min_max_sizes", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .child(|container| {
                        for i in 0..50 {
                            container
                                .text(format!("Item {}", i))
                                .size(16.0)
                                .min_width(100.0)
                                .max_width(300.0)
                                .build();
                        }
                        container.container().build()
                    })
                    .build();
            });
            ui.compute_layout();
            black_box(())
        });
    });

    group.bench_function("padding_margin", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(800.0)
                    .height(600.0)
                    .padding(20.0)
                    .child(|container| {
                        for i in 0..30 {
                            container
                                .text(format!("Item {}", i))
                                .size(16.0)
                                .padding(5.0)
                                .margin(3.0)
                                .build();
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

fn bench_alignment_variations(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment_variations");

    for align in [
        astrelis_ui::AlignItems::Start,
        astrelis_ui::AlignItems::Center,
        astrelis_ui::AlignItems::End,
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", align)),
            &align,
            |b, &align| {
                let mut ui = setup();
                b.iter(|| {
                    ui.build(|root| {
                        root.container()
                            .width(800.0)
                            .height(600.0)
                            .align_items(align)
                            .child(|container| {
                                for i in 0..30 {
                                    container.text(format!("Item {}", i)).size(16.0).build();
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
    }

    group.finish();
}

fn bench_multi_level_nesting(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_level_nesting");

    for levels in [2, 5, 10] {
        group.bench_with_input(
            BenchmarkId::from_parameter(levels),
            &levels,
            |b, &levels| {
                let mut ui = setup();
                b.iter(|| {
                    ui.build(|root| {
                        root.container()
                            .width(800.0)
                            .height(600.0)
                            .padding(5.0)
                            .child(|level1| {
                                for i in 0..levels {
                                    level1
                                        .container()
                                        .width(750.0 - i as f32 * 30.0)
                                        .padding(5.0)
                                        .child(|level2| {
                                            level2
                                                .text(format!("Nested item {}", i))
                                                .size(16.0)
                                                .build()
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
            },
        );
    }

    group.finish();
}

fn bench_form_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("form_layout");

    group.bench_function("registration_form", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(600.0)
                    .padding(30.0)
                    .gap(20.0)
                    .child(|form| {
                        form.text("Registration Form").size(28.0).bold().build();

                        for field in [
                            "Username",
                            "Email",
                            "Password",
                            "Confirm Password",
                            "First Name",
                            "Last Name",
                            "Phone Number",
                            "Address",
                        ] {
                            form.container()
                                .width(540.0)
                                .gap(10.0)
                                .child(|row| {
                                    row.text(format!("{}:", field)).size(14.0).build();
                                    row.text_input("").width(300.0).build();
                                    row.container().build()
                                })
                                .build();
                        }

                        form.container()
                            .width(540.0)
                            .gap(10.0)
                            .child(|buttons| {
                                buttons.button("Submit").width(100.0).build();
                                buttons.button("Cancel").width(100.0).build();
                                buttons.button("Reset").width(100.0).build();
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

fn bench_dashboard_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("dashboard_layout");

    group.bench_function("full_dashboard", |b| {
        let mut ui = setup();
        b.iter(|| {
            ui.build(|root| {
                root.container()
                    .width(1200.0)
                    .height(800.0)
                    .padding(20.0)
                    .gap(10.0)
                    .child(|main| {
                        // Header
                        main.container()
                            .width(1160.0)
                            .height(60.0)
                            .padding(10.0)
                            .gap(20.0)
                            .child(|header| {
                                header.text("Dashboard").size(24.0).bold().build();
                                header.button("Settings").build();
                                header.button("Logout").build();
                                header.container().build()
                            })
                            .build();

                        // Content
                        main.container()
                            .width(1160.0)
                            .height(680.0)
                            .gap(20.0)
                            .flex_direction(astrelis_ui::FlexDirection::Row)
                            .child(|content| {
                                // Sidebar
                                content
                                    .container()
                                    .width(250.0)
                                    .height(680.0)
                                    .padding(10.0)
                                    .gap(5.0)
                                    .child(|sidebar| {
                                        for item in ["Home", "Projects", "Tasks", "Team", "Reports"]
                                        {
                                            sidebar.button(item).width(230.0).build();
                                        }
                                        sidebar.container().build()
                                    })
                                    .build();

                                // Main panel
                                content
                                    .container()
                                    .width(870.0)
                                    .height(680.0)
                                    .padding(10.0)
                                    .gap(15.0)
                                    .child(|panel| {
                                        for i in 0..8 {
                                            panel
                                                .container()
                                                .width(850.0)
                                                .padding(15.0)
                                                .child(|card| {
                                                    card.text(format!("Card {}", i))
                                                        .size(18.0)
                                                        .bold()
                                                        .build();
                                                    card.text("Description").size(14.0).build();
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

                        // Footer
                        main.container()
                            .width(1160.0)
                            .height(30.0)
                            .padding(5.0)
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

    group.finish();
}

criterion_group!(
    benches,
    bench_tree_build_simple,
    bench_flexbox_layouts,
    bench_nested_containers,
    bench_sizing_constraints,
    bench_alignment_variations,
    bench_multi_level_nesting,
    bench_form_layout,
    bench_dashboard_layout
);
criterion_main!(benches);
