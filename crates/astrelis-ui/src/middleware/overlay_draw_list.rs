//! Draw list for middleware overlay commands.
//!
//! Collects drawing commands from middleware and provides them to the
//! overlay renderer for GPU rendering.

use astrelis_core::math::Vec2;
use astrelis_render::Color;

/// A quad command for the overlay.
#[derive(Debug, Clone)]
pub struct OverlayQuadCmd {
    /// Position (top-left corner).
    pub position: Vec2,
    /// Size of the quad.
    pub size: Vec2,
    /// Fill color.
    pub fill_color: Color,
    /// Optional border color.
    pub border_color: Option<Color>,
    /// Border width (0 for no border).
    pub border_width: f32,
    /// Border radius for rounded corners.
    pub border_radius: f32,
}

/// A text command for the overlay.
#[derive(Debug, Clone)]
pub struct OverlayText {
    /// Position (top-left corner).
    pub position: Vec2,
    /// Text content.
    pub text: String,
    /// Text color.
    pub color: Color,
    /// Font size in pixels.
    pub size: f32,
}

/// A line command for the overlay.
#[derive(Debug, Clone)]
pub struct OverlayLine {
    /// Start point.
    pub start: Vec2,
    /// End point.
    pub end: Vec2,
    /// Line color.
    pub color: Color,
    /// Line thickness in pixels.
    pub thickness: f32,
}

/// Command types for overlay rendering.
#[derive(Debug, Clone)]
pub enum OverlayCommand {
    /// Draw a filled/bordered quad.
    Quad(OverlayQuadCmd),
    /// Draw text.
    Text(OverlayText),
    /// Draw a line.
    Line(OverlayLine),
}

/// Accumulates overlay draw commands from middleware.
#[derive(Debug, Default)]
pub struct OverlayDrawList {
    commands: Vec<OverlayCommand>,
}

impl OverlayDrawList {
    /// Create a new empty draw list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Check if the draw list is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get the number of commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Get all commands.
    pub fn commands(&self) -> &[OverlayCommand] {
        &self.commands
    }

    /// Take ownership of all commands and clear the list.
    pub fn take_commands(&mut self) -> Vec<OverlayCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Add a quad command.
    pub fn add_quad(
        &mut self,
        position: Vec2,
        size: Vec2,
        fill_color: Color,
        border_color: Option<Color>,
        border_width: f32,
        border_radius: f32,
    ) {
        self.commands.push(OverlayCommand::Quad(OverlayQuadCmd {
            position,
            size,
            fill_color,
            border_color,
            border_width,
            border_radius,
        }));
    }

    /// Add a text command.
    pub fn add_text(&mut self, position: Vec2, text: String, color: Color, size: f32) {
        self.commands.push(OverlayCommand::Text(OverlayText {
            position,
            text,
            color,
            size,
        }));
    }

    /// Add a line command.
    pub fn add_line(&mut self, start: Vec2, end: Vec2, color: Color, thickness: f32) {
        self.commands.push(OverlayCommand::Line(OverlayLine {
            start,
            end,
            color,
            thickness,
        }));
    }

    /// Get iterators for specific command types.
    pub fn quads(&self) -> impl Iterator<Item = &OverlayQuadCmd> {
        self.commands.iter().filter_map(|c| {
            if let OverlayCommand::Quad(q) = c {
                Some(q)
            } else {
                None
            }
        })
    }

    /// Get iterators for text commands.
    pub fn texts(&self) -> impl Iterator<Item = &OverlayText> {
        self.commands.iter().filter_map(|c| {
            if let OverlayCommand::Text(t) = c {
                Some(t)
            } else {
                None
            }
        })
    }

    /// Get iterators for line commands.
    pub fn lines(&self) -> impl Iterator<Item = &OverlayLine> {
        self.commands.iter().filter_map(|c| {
            if let OverlayCommand::Line(l) = c {
                Some(l)
            } else {
                None
            }
        })
    }

    /// Merge another draw list into this one.
    pub fn extend(&mut self, other: &OverlayDrawList) {
        self.commands.extend(other.commands.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_list_creation() {
        let list = OverlayDrawList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_add_quad() {
        let mut list = OverlayDrawList::new();
        list.add_quad(
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 50.0),
            Color::RED,
            Some(Color::WHITE),
            2.0,
            4.0,
        );

        assert_eq!(list.len(), 1);
        assert_eq!(list.quads().count(), 1);

        let quad = list.quads().next().unwrap();
        assert_eq!(quad.position.x, 10.0);
        assert_eq!(quad.size.x, 100.0);
        assert_eq!(quad.border_radius, 4.0);
    }

    #[test]
    fn test_add_text() {
        let mut list = OverlayDrawList::new();
        list.add_text(
            Vec2::new(10.0, 10.0),
            "Hello".to_string(),
            Color::WHITE,
            16.0,
        );

        assert_eq!(list.len(), 1);
        assert_eq!(list.texts().count(), 1);

        let text = list.texts().next().unwrap();
        assert_eq!(text.text, "Hello");
        assert_eq!(text.size, 16.0);
    }

    #[test]
    fn test_add_line() {
        let mut list = OverlayDrawList::new();
        list.add_line(Vec2::ZERO, Vec2::new(100.0, 100.0), Color::GREEN, 2.0);

        assert_eq!(list.len(), 1);
        assert_eq!(list.lines().count(), 1);

        let line = list.lines().next().unwrap();
        assert_eq!(line.start, Vec2::ZERO);
        assert_eq!(line.thickness, 2.0);
    }

    #[test]
    fn test_clear() {
        let mut list = OverlayDrawList::new();
        list.add_quad(
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Color::RED,
            None,
            0.0,
            0.0,
        );
        list.add_text(Vec2::ZERO, "Test".to_string(), Color::WHITE, 12.0);

        assert_eq!(list.len(), 2);

        list.clear();
        assert!(list.is_empty());
    }

    #[test]
    fn test_take_commands() {
        let mut list = OverlayDrawList::new();
        list.add_quad(
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Color::RED,
            None,
            0.0,
            0.0,
        );

        let commands = list.take_commands();
        assert_eq!(commands.len(), 1);
        assert!(list.is_empty());
    }

    #[test]
    fn test_extend() {
        let mut list1 = OverlayDrawList::new();
        list1.add_quad(
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Color::RED,
            None,
            0.0,
            0.0,
        );

        let mut list2 = OverlayDrawList::new();
        list2.add_text(Vec2::ZERO, "Test".to_string(), Color::WHITE, 12.0);

        list1.extend(&list2);
        assert_eq!(list1.len(), 2);
    }

    #[test]
    fn test_mixed_commands() {
        let mut list = OverlayDrawList::new();
        list.add_quad(
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            Color::RED,
            None,
            0.0,
            0.0,
        );
        list.add_text(Vec2::new(5.0, 5.0), "Label".to_string(), Color::WHITE, 14.0);
        list.add_line(Vec2::ZERO, Vec2::new(100.0, 0.0), Color::BLUE, 1.0);
        list.add_quad(
            Vec2::new(50.0, 50.0),
            Vec2::new(20.0, 20.0),
            Color::GREEN,
            None,
            0.0,
            0.0,
        );

        assert_eq!(list.len(), 4);
        assert_eq!(list.quads().count(), 2);
        assert_eq!(list.texts().count(), 1);
        assert_eq!(list.lines().count(), 1);
    }
}
