//! Unit tests for draw list generation (no GPU required).
//!
//! These tests verify that the draw list system correctly manages draw commands,
//! tracks node-to-command mappings, and handles incremental updates.

use astrelis_ui::{DrawCommand, DrawList, QuadCommand};
use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_ui::tree::NodeId;

#[test]
fn test_draw_list_creation() {
    let draw_list = DrawList::new();

    // Should create empty draw list
    assert_eq!(draw_list.len(), 0);
    assert!(draw_list.is_empty());
}

#[test]
fn test_draw_list_with_capacity() {
    let draw_list = DrawList::with_capacity(100);

    assert_eq!(draw_list.len(), 0);
    assert!(draw_list.is_empty());
}

#[test]
fn test_add_quad_command() {
    let mut draw_list = DrawList::new();
    let node_id = NodeId(1);

    let quad = QuadCommand::filled(
        Vec2::new(10.0, 10.0),
        Vec2::new(100.0, 100.0),
        Color::RED,
        0,
    );

    draw_list.update_node(node_id, vec![DrawCommand::Quad(quad)]);

    assert_eq!(draw_list.len(), 1);
    assert!(!draw_list.is_empty());
}

#[test]
fn test_add_multiple_commands() {
    let mut draw_list = DrawList::new();
    let node_id = NodeId(1);

    let commands = vec![
        DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 50.0),
            Color::RED,
            0,
        )),
        DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(60.0, 60.0),
            Vec2::new(50.0, 50.0),
            Color::BLUE,
            0,
        )),
    ];

    draw_list.update_node(node_id, commands);

    assert_eq!(draw_list.len(), 2);
}

#[test]
fn test_update_node_replaces_commands() {
    let mut draw_list = DrawList::new();
    let node_id = NodeId(1);

    // Add initial command
    let quad1 = QuadCommand::filled(
        Vec2::new(0.0, 0.0),
        Vec2::new(100.0, 100.0),
        Color::RED,
        0,
    );
    draw_list.update_node(node_id, vec![DrawCommand::Quad(quad1)]);

    assert_eq!(draw_list.len(), 1);

    // Update with new command
    let quad2 = QuadCommand::filled(
        Vec2::new(50.0, 50.0),
        Vec2::new(200.0, 200.0),
        Color::BLUE,
        0,
    );
    draw_list.update_node(node_id, vec![DrawCommand::Quad(quad2)]);

    // Before compaction, old command still exists
    assert_eq!(draw_list.len(), 2);

    // After sort (which compacts), should have only 1 command
    draw_list.sort_if_needed();
    assert_eq!(draw_list.len(), 1);
}

#[test]
fn test_multiple_nodes() {
    let mut draw_list = DrawList::new();

    let node1 = NodeId(1);
    let node2 = NodeId(2);
    let node3 = NodeId(3);

    draw_list.update_node(
        node1,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::ZERO,
            Vec2::new(100.0, 100.0),
            Color::RED,
            0,
        ))],
    );

    draw_list.update_node(
        node2,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Color::GREEN,
            0,
        ))],
    );

    draw_list.update_node(
        node3,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(200.0, 0.0),
            Vec2::new(100.0, 100.0),
            Color::BLUE,
            0,
        ))],
    );

    assert_eq!(draw_list.len(), 3);
}

#[test]
fn test_remove_node() {
    let mut draw_list = DrawList::new();
    let node_id = NodeId(1);

    // Add command
    draw_list.update_node(
        node_id,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::ZERO,
            Vec2::new(100.0, 100.0),
            Color::RED,
            0,
        ))],
    );

    assert_eq!(draw_list.len(), 1);

    // Remove by updating with empty vec
    draw_list.update_node(node_id, vec![]);

    // Before compaction, command still exists
    assert_eq!(draw_list.len(), 1);

    // After sort (which compacts), command is removed
    draw_list.sort_if_needed();
    assert_eq!(draw_list.len(), 0);
    assert!(draw_list.is_empty());
}

#[test]
fn test_quad_command_creation() {
    let quad = QuadCommand::filled(
        Vec2::new(10.0, 20.0),
        Vec2::new(100.0, 50.0),
        Color::RED,
        5,
    );

    assert_eq!(quad.position, Vec2::new(10.0, 20.0));
    assert_eq!(quad.size, Vec2::new(100.0, 50.0));
    assert_eq!(quad.color, Color::RED);
    assert_eq!(quad.z_index, 5);
    assert_eq!(quad.border_radius, 0.0);
    assert_eq!(quad.border_thickness, 0.0);
}

#[test]
fn test_quad_command_rounded() {
    let mut quad = QuadCommand::filled(
        Vec2::ZERO,
        Vec2::new(100.0, 100.0),
        Color::BLUE,
        0,
    );

    quad.border_radius = 10.0;

    assert_eq!(quad.border_radius, 10.0);
}

#[test]
fn test_quad_command_bordered() {
    let quad = QuadCommand::bordered(
        Vec2::new(5.0, 5.0),
        Vec2::new(90.0, 90.0),
        Color::BLACK,
        2.0,
        0.0,
        0,
    );

    assert_eq!(quad.border_thickness, 2.0);
    assert_eq!(quad.color, Color::BLACK);
}

#[test]
fn test_draw_command_z_index() {
    let quad1 = QuadCommand::filled(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::RED, 0);
    let quad2 = QuadCommand::filled(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::BLUE, 10);

    let cmd1 = DrawCommand::Quad(quad1);
    let cmd2 = DrawCommand::Quad(quad2);

    assert_eq!(cmd1.z_index(), 0);
    assert_eq!(cmd2.z_index(), 10);
}

#[test]
fn test_draw_command_opacity() {
    let opaque_quad = QuadCommand::filled(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::RED, 0);
    let transparent_quad = QuadCommand::filled(
        Vec2::ZERO,
        Vec2::new(100.0, 100.0),
        Color::rgba(1.0, 0.0, 0.0, 0.5),
        0,
    );

    let opaque_cmd = DrawCommand::Quad(opaque_quad);
    let transparent_cmd = DrawCommand::Quad(transparent_quad);

    assert!(opaque_cmd.is_opaque());
    assert!(!transparent_cmd.is_opaque());
}

#[test]
fn test_draw_command_node_id() {
    let mut quad = QuadCommand::filled(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::RED, 0);
    quad.node_id = NodeId(42);

    let cmd = DrawCommand::Quad(quad);

    assert_eq!(cmd.node_id(), NodeId(42));
}

#[test]
fn test_draw_command_set_node_id() {
    let quad = QuadCommand::filled(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::RED, 0);
    let mut cmd = DrawCommand::Quad(quad);

    cmd.set_node_id(NodeId(99));

    assert_eq!(cmd.node_id(), NodeId(99));
}

#[test]
fn test_clear_draw_list() {
    let mut draw_list = DrawList::new();

    // Add several commands
    for i in 0..10 {
        draw_list.update_node(
            NodeId(i),
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::RED,
                0,
            ))],
        );
    }

    assert_eq!(draw_list.len(), 10);

    draw_list.clear();

    assert_eq!(draw_list.len(), 0);
    assert!(draw_list.is_empty());
}

#[test]
fn test_draw_list_incremental_update() {
    let mut draw_list = DrawList::new();

    let node1 = NodeId(1);
    let node2 = NodeId(2);

    // Add node1
    draw_list.update_node(
        node1,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::ZERO,
            Vec2::new(100.0, 100.0),
            Color::RED,
            0,
        ))],
    );

    // Add node2
    draw_list.update_node(
        node2,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Color::GREEN,
            0,
        ))],
    );

    assert_eq!(draw_list.len(), 2);

    // Update only node1
    draw_list.update_node(
        node1,
        vec![DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(50.0, 50.0),
            Vec2::new(150.0, 150.0),
            Color::BLUE,
            0,
        ))],
    );

    // Before compaction: node1's old command + node2 + node1's new command = 3
    assert_eq!(draw_list.len(), 3);

    // After compaction: only node1's new command + node2 = 2
    draw_list.sort_if_needed();
    assert_eq!(draw_list.len(), 2);
}
