//! Context types for middleware callbacks.
//!
//! Provides read-only access to UI state for middleware inspection,
//! and a drawing API for rendering debug overlays.

use astrelis_core::math::Vec2;
use astrelis_render::{Color, Viewport};

use crate::event::UiEventSystem;
use crate::inspector::OverlayQuad;
use crate::metrics_collector::MetricsCollector;
use crate::tree::UiTree;
use crate::widget_id::WidgetIdRegistry;

use super::overlay_draw_list::OverlayDrawList;

/// Read-only context provided to middleware during callbacks.
///
/// Provides access to UI tree, events, metrics, and viewport information.
#[derive(Clone)]
pub struct MiddlewareContext<'a> {
    /// Reference to the UI tree.
    pub tree: &'a UiTree,
    /// Performance metrics collector (if available).
    pub metrics: Option<&'a MetricsCollector>,
    /// Event system state (hover, focus).
    pub events: &'a UiEventSystem,
    /// Widget ID registry for lookups.
    pub registry: &'a WidgetIdRegistry,
    /// Current viewport.
    pub viewport: Viewport,
    /// Current mouse position in logical coordinates.
    pub mouse_position: Vec2,
    /// Frame delta time in seconds.
    pub delta_time: f32,
    /// Current frame number.
    pub frame_number: u64,
}

impl<'a> MiddlewareContext<'a> {
    /// Create a new middleware context.
    pub fn new(
        tree: &'a UiTree,
        events: &'a UiEventSystem,
        registry: &'a WidgetIdRegistry,
        viewport: Viewport,
    ) -> Self {
        Self {
            tree,
            metrics: None,
            events,
            registry,
            viewport,
            mouse_position: events.mouse_position(),
            delta_time: 0.0,
            frame_number: 0,
        }
    }

    /// Set the metrics collector.
    pub fn with_metrics(mut self, metrics: &'a MetricsCollector) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Set the delta time.
    pub fn with_delta_time(mut self, delta_time: f32) -> Self {
        self.delta_time = delta_time;
        self
    }

    /// Set the frame number.
    pub fn with_frame_number(mut self, frame_number: u64) -> Self {
        self.frame_number = frame_number;
        self
    }

    /// Get the viewport size in logical coordinates.
    pub fn viewport_size(&self) -> Vec2 {
        let logical = self.viewport.to_logical();
        Vec2::new(logical.width, logical.height)
    }
}

/// Context for drawing debug overlays.
///
/// Provides a simple drawing API for middleware to render debug visualizations
/// on top of the UI.
pub struct OverlayContext<'a> {
    draw_list: &'a mut OverlayDrawList,
}

impl<'a> OverlayContext<'a> {
    /// Create a new overlay context wrapping a draw list.
    pub fn new(draw_list: &'a mut OverlayDrawList) -> Self {
        Self { draw_list }
    }

    /// Draw a filled rectangle.
    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.draw_list
            .add_quad(position, size, color, None, 0.0, 0.0);
    }

    /// Draw a filled rectangle with rounded corners.
    pub fn draw_rect_rounded(&mut self, position: Vec2, size: Vec2, color: Color, radius: f32) {
        self.draw_list
            .add_quad(position, size, color, None, 0.0, radius);
    }

    /// Draw a bordered rectangle.
    pub fn draw_rect_bordered(
        &mut self,
        position: Vec2,
        size: Vec2,
        fill: Color,
        border: Color,
        border_width: f32,
    ) {
        self.draw_list
            .add_quad(position, size, fill, Some(border), border_width, 0.0);
    }

    /// Draw a bordered rectangle with rounded corners.
    pub fn draw_rect_bordered_rounded(
        &mut self,
        position: Vec2,
        size: Vec2,
        fill: Color,
        border: Color,
        border_width: f32,
        radius: f32,
    ) {
        self.draw_list
            .add_quad(position, size, fill, Some(border), border_width, radius);
    }

    /// Draw text at a position.
    pub fn draw_text(&mut self, position: Vec2, text: &str, color: Color, size: f32) {
        self.draw_list
            .add_text(position, text.to_string(), color, size);
    }

    /// Draw a line between two points.
    pub fn draw_line(&mut self, start: Vec2, end: Vec2, color: Color, thickness: f32) {
        self.draw_list.add_line(start, end, color, thickness);
    }

    /// Draw from an existing OverlayQuad (for inspector compatibility).
    pub fn draw_overlay_quad(&mut self, quad: &OverlayQuad) {
        self.draw_list.add_quad(
            quad.position,
            quad.size,
            quad.fill_color,
            quad.border_color,
            quad.border_width,
            0.0,
        );
    }

    /// Get direct access to the draw list for advanced usage.
    pub fn draw_list_mut(&mut self) -> &mut OverlayDrawList {
        self.draw_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::UiEventSystem;
    use crate::tree::UiTree;
    use crate::widget_id::WidgetIdRegistry;

    #[test]
    fn test_middleware_context_creation() {
        let tree = UiTree::new();
        let events = UiEventSystem::new();
        let registry = WidgetIdRegistry::new();
        let viewport = Viewport::default();

        let ctx = MiddlewareContext::new(&tree, &events, &registry, viewport)
            .with_delta_time(0.016)
            .with_frame_number(42);

        assert_eq!(ctx.delta_time, 0.016);
        assert_eq!(ctx.frame_number, 42);
    }

    #[test]
    fn test_overlay_context_drawing() {
        let mut draw_list = OverlayDrawList::new();

        {
            let mut ctx = OverlayContext::new(&mut draw_list);
            ctx.draw_rect(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0), Color::RED);
            ctx.draw_text(Vec2::new(10.0, 10.0), "Test", Color::WHITE, 16.0);
            ctx.draw_line(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::GREEN, 2.0);
        }

        assert_eq!(draw_list.commands().len(), 3);
    }

    #[test]
    fn test_overlay_quad_compatibility() {
        let mut draw_list = OverlayDrawList::new();

        let quad = OverlayQuad {
            position: Vec2::new(0.0, 0.0),
            size: Vec2::new(100.0, 100.0),
            fill_color: Color::RED,
            border_color: Some(Color::WHITE),
            border_width: 2.0,
        };

        {
            let mut ctx = OverlayContext::new(&mut draw_list);
            ctx.draw_overlay_quad(&quad);
        }

        assert_eq!(draw_list.commands().len(), 1);
    }
}
