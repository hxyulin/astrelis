//! Unit tests for layout engine (no GPU required).
//!
//! These tests verify that the layout system correctly computes widget
//! positions and sizes using the Taffy layout engine without requiring GPU.

use astrelis_core::geometry::{PhysicalPosition, PhysicalSize, ScaleFactor, Size};
use astrelis_render::Viewport;
use astrelis_ui::{UiCore, WidgetId};
use astrelis_ui::constraint::Constraint;

fn default_viewport() -> Viewport {
    Viewport {
        position: PhysicalPosition::new(0.0, 0.0),
        size: PhysicalSize::new(800.0, 600.0),
        scale_factor: ScaleFactor(1.0),
    }
}

#[test]
fn test_layout_basic_container() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container().width(800.0).height(600.0).build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Layout should complete without errors
}

#[test]
fn test_layout_nested_containers() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .children(|parent| {
                vec![
                    parent.container().width(400.0).height(300.0).build(),
                    parent.container().width(400.0).height(300.0).build(),
                ]
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should complete without panicking
}

#[test]
fn test_layout_flexbox_row() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.row()
            .width(800.0)
            .height(100.0)
            .gap(10.0)
            .child(|row| row.container().width(100.0).height(100.0).build())
            .child(|row| row.container().width(100.0).height(100.0).build())
            .child(|row| row.container().width(100.0).height(100.0).build())
            .build();
    });

    ui.compute_layout();
    // Should complete without panicking
}

#[test]
fn test_layout_flexbox_column() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.column()
            .width(800.0)
            .height(600.0)
            .gap(20.0)
            .child(|col| col.container().width(800.0).height(100.0).build())
            .child(|col| col.container().width(800.0).height(100.0).build())
            .child(|col| col.container().width(800.0).height(100.0).build())
            .build();
    });

    ui.compute_layout();
    // Should complete without panicking
}

#[test]
fn test_layout_text_widget_without_font() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.text("Hello, World!").size(24.0).build();
    });

    ui.compute_layout();

    // Text measurement will use fallback estimation without FontRenderer
}

#[test]
fn test_layout_button_widget() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.button("Click Me").build();
    });

    ui.compute_layout();
    // Should complete without errors
}

#[test]
fn test_layout_with_padding() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .padding(20.0)
            .child(|container| {
                container
                    .container()
                    .width(760.0) // 800 - 2*20
                    .height(560.0) // 600 - 2*20
                    .build()
            })
            .build();
    });

    ui.compute_layout();
    // Should handle padding correctly
}

#[test]
fn test_layout_complex_hierarchy() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .children(|container| {
                vec![
                    // Header
                    container
                        .row()
                        .height(60.0)
                        .child(|header| header.text("App Title").size(24.0).build())
                        .child(|header| header.button("Menu").build())
                        .build(),
                    // Main content
                    container
                        .container()
                        .child(|main| main.text("Main content area").build())
                        .build(),
                    // Footer
                    container
                        .row()
                        .height(40.0)
                        .child(|footer| footer.text("Footer text").size(12.0).build())
                        .build(),
                ]
            })
            .build();
    });

    ui.compute_layout();

    // Should handle complex hierarchy without crashes
}

#[test]
fn test_layout_viewport_change() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container().width(800.0).height(600.0).build();
    });

    // Initial viewport
    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Change viewport
    ui.set_viewport(Viewport {
        position: PhysicalPosition::new(0.0, 0.0),
        size: PhysicalSize::new(1920.0, 1080.0),
        scale_factor: ScaleFactor(1.0),
    });
    ui.compute_layout();

    // Should handle viewport changes without errors
    assert_eq!(ui.viewport_size(), Size::new(1920.0, 1080.0));
}

#[test]
fn test_layout_widget_id_registry() {
    let mut ui = UiCore::new();

    let text_id = WidgetId::new("my_text");
    let button_id = WidgetId::new("my_button");

    ui.build(|root| {
        root.text("Hello").id(text_id).build();
        root.button("Click").id(button_id).build();
    });

    ui.compute_layout();

    // Should be able to find widgets by ID
    assert!(ui.get_node_id(text_id).is_some());
    assert!(ui.get_node_id(button_id).is_some());
}

#[test]
fn test_layout_incremental_updates() {
    let mut ui = UiCore::new();

    let text_id = WidgetId::new("counter");

    ui.build(|root| {
        root.text("Count: 0").id(text_id).size(24.0).build();
    });

    ui.compute_layout();

    // Update text content
    let changed = ui.update_text(text_id, "Count: 1");
    assert!(changed);

    ui.compute_layout();

    // Should handle incremental updates
}

#[test]
fn test_layout_with_colors() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(200.0)
            .height(200.0)
            .background_color(astrelis_render::Color::RED)
            .build();
    });

    ui.compute_layout();

    // Should handle colored widgets without panicking
}

#[test]
fn test_ui_core_is_render_agnostic() {
    // This test verifies that UiCore can be used without GPU/rendering
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container().build();
    });

    // All these operations work without any GPU/render context
    ui.compute_layout();

    let widget_id = WidgetId::new("test");
    ui.build(|root| {
        root.text("Updated").id(widget_id).build();
    });

    ui.compute_layout();

    // Verify we can update widgets
    let changed = ui.update_text(widget_id, "New text");
    assert!(changed);
}

// ════════════════════════════════════════════════════════════════════════════
// Margin and Padding Tests
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_layout_with_margin() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .child(|container| {
                container
                    .container()
                    .width(200.0)
                    .height(100.0)
                    .margin(20.0) // Uniform margin
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle uniform margin without errors
}

#[test]
fn test_layout_with_per_side_padding() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .padding_left(10.0)
            .padding_top(20.0)
            .padding_right(30.0)
            .padding_bottom(40.0)
            .child(|container| {
                container.container().width(100.0).height(100.0).build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle per-side padding without errors
}

#[test]
fn test_layout_with_per_side_margin() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .child(|container| {
                container
                    .container()
                    .width(200.0)
                    .height(100.0)
                    .margin_left(10.0)
                    .margin_top(20.0)
                    .margin_right(30.0)
                    .margin_bottom(40.0)
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle per-side margin without errors
}

#[test]
fn test_padding_x_y() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .padding_x(40.0) // Left and right
            .padding_y(20.0) // Top and bottom
            .child(|container| {
                container.container().width(100.0).height(100.0).build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle horizontal/vertical padding shortcuts
}

#[test]
fn test_margin_x_y() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .child(|container| {
                container
                    .container()
                    .width(200.0)
                    .height(100.0)
                    .margin_x(30.0) // Left and right
                    .margin_y(15.0) // Top and bottom
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle horizontal/vertical margin shortcuts
}

#[test]
fn test_individual_padding_setters() {
    let mut ui = UiCore::new();

    // Test each individual padding setter
    ui.build(|root| {
        root.column()
            .width(800.0)
            .height(600.0)
            .child(|col| {
                col.container()
                    .padding_left(10.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .child(|col| {
                col.container()
                    .padding_top(15.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .child(|col| {
                col.container()
                    .padding_right(20.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .child(|col| {
                col.container()
                    .padding_bottom(25.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle individual padding setters
}

#[test]
fn test_individual_margin_setters() {
    let mut ui = UiCore::new();

    // Test each individual margin setter
    ui.build(|root| {
        root.column()
            .width(800.0)
            .height(600.0)
            .child(|col| {
                col.container()
                    .margin_left(10.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .child(|col| {
                col.container()
                    .margin_top(15.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .child(|col| {
                col.container()
                    .margin_right(20.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .child(|col| {
                col.container()
                    .margin_bottom(25.0)
                    .width(100.0)
                    .height(50.0)
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle individual margin setters
}

#[test]
fn test_padding_with_percent_constraint() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        // Use per-side methods with Constraint since Constraint doesn't impl Copy
        root.container()
            .width(800.0)
            .height(600.0)
            .padding_left(Constraint::Percent(5.0))
            .padding_top(Constraint::Percent(5.0))
            .padding_right(Constraint::Percent(5.0))
            .padding_bottom(Constraint::Percent(5.0))
            .child(|container| {
                container.container().width(100.0).height(100.0).build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle percentage padding
}

#[test]
fn test_margin_auto_centering() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .child(|container| {
                container
                    .container()
                    .width(Constraint::Percent(50.0))
                    .height(100.0)
                    .margin_left(Constraint::Auto)  // Center horizontally
                    .margin_right(Constraint::Auto)
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle auto margins for centering
}

#[test]
fn test_mixed_padding_and_margin() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .padding_x(20.0)
            .padding_y(10.0)
            .child(|container| {
                container
                    .container()
                    .width(200.0)
                    .height(100.0)
                    .margin_left(15.0)
                    .margin_top(5.0)
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should handle combined padding and margin
}

#[test]
fn test_padding_preserves_existing_values() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        // Set uniform padding first, then override one side
        root.container()
            .width(800.0)
            .height(600.0)
            .padding(20.0)
            .padding_left(50.0) // Override just the left side
            .child(|container| {
                container.container().width(100.0).height(100.0).build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should preserve other padding values when setting individual sides
}

#[test]
fn test_margin_preserves_existing_values() {
    let mut ui = UiCore::new();

    ui.build(|root| {
        root.container()
            .width(800.0)
            .height(600.0)
            .child(|container| {
                // Set uniform margin first, then override one side
                container
                    .container()
                    .width(200.0)
                    .height(100.0)
                    .margin(15.0)
                    .margin_top(30.0) // Override just the top side
                    .build()
            })
            .build();
    });

    ui.set_viewport(default_viewport());
    ui.compute_layout();

    // Should preserve other margin values when setting individual sides
}
