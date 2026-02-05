//! Text editor with selection and cursor management.
//!
//! This module provides text editing capabilities for text input widgets:
//! - Cursor positioning and movement
//! - Text selection
//! - Insert and delete operations
//! - Hit testing (screen position to cursor position)
//! - Selection rectangle generation
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::*;
//!
//! let mut editor = TextEditor::new("Hello, World!");
//!
//! // Move cursor
//! editor.move_cursor_end();
//!
//! // Insert text at cursor
//! editor.insert_char('!');
//!
//! // Select text
//! editor.select(0, 5); // Select "Hello"
//!
//! // Delete selection
//! editor.delete_selection();
//! ```

use astrelis_core::math::Vec2;

/// Text cursor position and state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextCursor {
    /// Byte offset in the string (UTF-8 byte index)
    pub position: usize,
    /// Visual X coordinate on screen (for vertical movement)
    pub visual_x: f32,
    /// Line number (0-indexed)
    pub line: usize,
    /// Column in the line (character index, not byte)
    pub column: usize,
}

impl TextCursor {
    /// Create a new cursor at position 0.
    pub fn new() -> Self {
        Self {
            position: 0,
            visual_x: 0.0,
            line: 0,
            column: 0,
        }
    }

    /// Create a cursor at a specific byte position.
    pub fn at_position(position: usize) -> Self {
        Self {
            position,
            visual_x: 0.0,
            line: 0,
            column: 0,
        }
    }
}

impl Default for TextCursor {
    fn default() -> Self {
        Self::new()
    }
}

/// Text selection range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSelection {
    /// Selection start (byte offset)
    pub start: usize,
    /// Selection end (byte offset)
    pub end: usize,
}

impl TextSelection {
    /// Create a new selection.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Get the selection as (min, max) to ensure start <= end.
    pub fn range(&self) -> (usize, usize) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// Get the selection length in bytes.
    pub fn len(&self) -> usize {
        let (min, max) = self.range();
        max - min
    }

    /// Check if the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Check if the selection contains a byte position.
    pub fn contains(&self, position: usize) -> bool {
        let (min, max) = self.range();
        position >= min && position < max
    }
}

/// Text editor with cursor and selection management.
pub struct TextEditor {
    /// Text content
    text: String,
    /// Cursor position
    cursor: TextCursor,
    /// Optional selection
    selection: Option<TextSelection>,
    /// Undo/redo history (simple version)
    history: Vec<String>,
    /// Current position in history
    history_position: usize,
}

impl TextEditor {
    /// Create a new text editor with initial text.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            text: text.clone(),
            cursor: TextCursor::new(),
            selection: None,
            history: vec![text],
            history_position: 0,
        }
    }

    /// Get the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the cursor position.
    pub fn cursor(&self) -> &TextCursor {
        &self.cursor
    }

    /// Get the selection, if any.
    pub fn selection(&self) -> Option<&TextSelection> {
        self.selection.as_ref()
    }

    /// Check if there's an active selection.
    pub fn has_selection(&self) -> bool {
        self.selection.as_ref().is_some_and(|sel| !sel.is_empty())
    }

    /// Set the cursor position (byte offset).
    pub fn set_cursor(&mut self, position: usize) {
        self.cursor.position = position.min(self.text.len());
        self.clear_selection();
        self.update_cursor_position();
    }

    /// Move cursor to start of text.
    pub fn move_cursor_start(&mut self) {
        self.set_cursor(0);
    }

    /// Move cursor to end of text.
    pub fn move_cursor_end(&mut self) {
        self.set_cursor(self.text.len());
    }

    /// Move cursor left by one character.
    pub fn move_cursor_left(&mut self) {
        if self.cursor.position > 0 {
            // Move to previous UTF-8 character boundary
            let mut pos = self.cursor.position - 1;
            while pos > 0 && !self.text.is_char_boundary(pos) {
                pos -= 1;
            }
            self.set_cursor(pos);
        }
    }

    /// Move cursor right by one character.
    pub fn move_cursor_right(&mut self) {
        if self.cursor.position < self.text.len() {
            // Move to next UTF-8 character boundary
            let mut pos = self.cursor.position + 1;
            while pos < self.text.len() && !self.text.is_char_boundary(pos) {
                pos += 1;
            }
            self.set_cursor(pos);
        }
    }

    /// Select text range.
    pub fn select(&mut self, start: usize, end: usize) {
        self.selection = Some(TextSelection::new(
            start.min(self.text.len()),
            end.min(self.text.len()),
        ));
        self.cursor.position = end.min(self.text.len());
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        self.select(0, self.text.len());
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        // Delete selection first if any
        if self.has_selection() {
            self.delete_selection();
        }

        // Insert character
        self.text.insert(self.cursor.position, c);
        self.cursor.position += c.len_utf8();
        self.update_cursor_position();
        self.push_history();
    }

    /// Insert a string at the cursor position.
    pub fn insert_str(&mut self, s: &str) {
        // Delete selection first if any
        if self.has_selection() {
            self.delete_selection();
        }

        // Insert string
        self.text.insert_str(self.cursor.position, s);
        self.cursor.position += s.len();
        self.update_cursor_position();
        self.push_history();
    }

    /// Delete character before cursor (backspace).
    pub fn delete_char(&mut self) {
        if self.has_selection() {
            self.delete_selection();
        } else if self.cursor.position > 0 {
            // Find previous character boundary
            let mut pos = self.cursor.position - 1;
            while pos > 0 && !self.text.is_char_boundary(pos) {
                pos -= 1;
            }

            self.text.drain(pos..self.cursor.position);
            self.cursor.position = pos;
            self.update_cursor_position();
            self.push_history();
        }
    }

    /// Delete character after cursor (delete key).
    pub fn delete_char_forward(&mut self) {
        if self.has_selection() {
            self.delete_selection();
        } else if self.cursor.position < self.text.len() {
            // Find next character boundary
            let mut pos = self.cursor.position + 1;
            while pos < self.text.len() && !self.text.is_char_boundary(pos) {
                pos += 1;
            }

            self.text.drain(self.cursor.position..pos);
            self.update_cursor_position();
            self.push_history();
        }
    }

    /// Delete the current selection.
    pub fn delete_selection(&mut self) {
        if let Some(sel) = self.selection {
            let (start, end) = sel.range();
            self.text.drain(start..end);
            self.cursor.position = start;
            self.clear_selection();
            self.update_cursor_position();
            self.push_history();
        }
    }

    /// Get selected text.
    pub fn selected_text(&self) -> Option<&str> {
        self.selection.as_ref().map(|sel| {
            let (start, end) = sel.range();
            &self.text[start..end]
        })
    }

    /// Replace selected text with new text.
    pub fn replace_selection(&mut self, text: &str) {
        if self.has_selection() {
            self.delete_selection();
        }
        self.insert_str(text);
    }

    /// Undo last operation.
    pub fn undo(&mut self) -> bool {
        if self.history_position > 0 {
            self.history_position -= 1;
            self.text = self.history[self.history_position].clone();
            self.cursor.position = self.cursor.position.min(self.text.len());
            self.clear_selection();
            self.update_cursor_position();
            true
        } else {
            false
        }
    }

    /// Redo last undone operation.
    pub fn redo(&mut self) -> bool {
        if self.history_position < self.history.len() - 1 {
            self.history_position += 1;
            self.text = self.history[self.history_position].clone();
            self.cursor.position = self.cursor.position.min(self.text.len());
            self.clear_selection();
            self.update_cursor_position();
            true
        } else {
            false
        }
    }

    /// Hit test to find cursor position from screen coordinates.
    ///
    /// This is a simplified version that assumes monospace font.
    /// For real usage, you'd need glyph position information from the text shaper.
    pub fn hit_test(&self, _pos: Vec2, _char_width: f32) -> usize {
        // Simplified: just return current cursor position
        // In a real implementation, this would:
        // 1. Find the line at the Y coordinate
        // 2. Find the character at the X coordinate within that line
        // 3. Return the byte offset of that character
        self.cursor.position
    }

    /// Get selection rectangles for rendering.
    ///
    /// This is a simplified version that returns a single rectangle.
    /// For real usage with multi-line selections, you'd need line layout information.
    pub fn selection_rects(
        &self,
        _line_height: f32,
        _char_width: f32,
    ) -> Vec<(f32, f32, f32, f32)> {
        if let Some(sel) = self.selection {
            if !sel.is_empty() {
                let (start, end) = sel.range();
                // Simplified: single rectangle
                // In a real implementation, this would generate rectangles per line
                vec![(
                    start as f32 * _char_width,
                    0.0,
                    (end - start) as f32 * _char_width,
                    _line_height,
                )]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    // Private helper methods

    fn update_cursor_position(&mut self) {
        // Update line and column based on cursor position
        let text_before = &self.text[..self.cursor.position];
        self.cursor.line = text_before.matches('\n').count();

        // Find column (character index in current line)
        if let Some(line_start) = text_before.rfind('\n') {
            let line_text = &text_before[line_start + 1..];
            self.cursor.column = line_text.chars().count();
        } else {
            self.cursor.column = text_before.chars().count();
        }
    }

    fn push_history(&mut self) {
        // Truncate history if we've undone and then made changes
        self.history.truncate(self.history_position + 1);

        // Add current state to history
        self.history.push(self.text.clone());
        self.history_position = self.history.len() - 1;

        // Limit history size
        const MAX_HISTORY: usize = 100;
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
            self.history_position -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_cursor_default() {
        let cursor = TextCursor::default();
        assert_eq!(cursor.position, 0);
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.column, 0);
    }

    #[test]
    fn test_text_selection_range() {
        let sel = TextSelection::new(5, 10);
        assert_eq!(sel.range(), (5, 10));
        assert_eq!(sel.len(), 5);
        assert!(!sel.is_empty());

        // Reversed selection
        let sel = TextSelection::new(10, 5);
        assert_eq!(sel.range(), (5, 10));
        assert_eq!(sel.len(), 5);
    }

    #[test]
    fn test_text_selection_contains() {
        let sel = TextSelection::new(5, 10);
        assert!(sel.contains(5));
        assert!(sel.contains(7));
        assert!(!sel.contains(10)); // End is exclusive
        assert!(!sel.contains(3));
    }

    #[test]
    fn test_editor_new() {
        let editor = TextEditor::new("Hello");
        assert_eq!(editor.text(), "Hello");
        assert_eq!(editor.cursor().position, 0);
        assert!(!editor.has_selection());
    }

    #[test]
    fn test_editor_insert_char() {
        let mut editor = TextEditor::new("Hello");
        editor.move_cursor_end();
        editor.insert_char('!');
        assert_eq!(editor.text(), "Hello!");
        assert_eq!(editor.cursor().position, 6);
    }

    #[test]
    fn test_editor_insert_str() {
        let mut editor = TextEditor::new("Hello");
        editor.move_cursor_end();
        editor.insert_str(", World");
        assert_eq!(editor.text(), "Hello, World");
    }

    #[test]
    fn test_editor_delete_char() {
        let mut editor = TextEditor::new("Hello");
        editor.move_cursor_end();
        editor.delete_char();
        assert_eq!(editor.text(), "Hell");
        assert_eq!(editor.cursor().position, 4);
    }

    #[test]
    fn test_editor_delete_char_forward() {
        let mut editor = TextEditor::new("Hello");
        editor.set_cursor(0);
        editor.delete_char_forward();
        assert_eq!(editor.text(), "ello");
        assert_eq!(editor.cursor().position, 0);
    }

    #[test]
    fn test_editor_selection() {
        let mut editor = TextEditor::new("Hello, World");
        editor.select(0, 5);
        assert!(editor.has_selection());
        assert_eq!(editor.selected_text(), Some("Hello"));
    }

    #[test]
    fn test_editor_delete_selection() {
        let mut editor = TextEditor::new("Hello, World");
        editor.select(0, 5);
        editor.delete_selection();
        assert_eq!(editor.text(), ", World");
        assert!(!editor.has_selection());
    }

    #[test]
    fn test_editor_replace_selection() {
        let mut editor = TextEditor::new("Hello, World");
        editor.select(7, 12);
        editor.replace_selection("Universe");
        assert_eq!(editor.text(), "Hello, Universe");
    }

    #[test]
    fn test_editor_select_all() {
        let mut editor = TextEditor::new("Hello");
        editor.select_all();
        assert_eq!(editor.selected_text(), Some("Hello"));
    }

    #[test]
    fn test_editor_cursor_movement() {
        let mut editor = TextEditor::new("Hello");

        editor.move_cursor_end();
        assert_eq!(editor.cursor().position, 5);

        editor.move_cursor_left();
        assert_eq!(editor.cursor().position, 4);

        editor.move_cursor_right();
        assert_eq!(editor.cursor().position, 5);

        editor.move_cursor_start();
        assert_eq!(editor.cursor().position, 0);
    }

    #[test]
    fn test_editor_undo_redo() {
        let mut editor = TextEditor::new("Hello");
        editor.move_cursor_end();
        editor.insert_char('!');
        assert_eq!(editor.text(), "Hello!");

        editor.undo();
        assert_eq!(editor.text(), "Hello");

        editor.redo();
        assert_eq!(editor.text(), "Hello!");
    }

    #[test]
    fn test_editor_utf8() {
        let mut editor = TextEditor::new("Hello 世界");
        editor.move_cursor_end();
        editor.insert_char('!');
        assert_eq!(editor.text(), "Hello 世界!");

        editor.delete_char();
        assert_eq!(editor.text(), "Hello 世界");

        editor.delete_char();
        assert_eq!(editor.text(), "Hello 世");
    }

    #[test]
    fn test_cursor_position_multiline() {
        let mut editor = TextEditor::new("Hello\nWorld");
        editor.set_cursor(6); // After newline
        assert_eq!(editor.cursor().line, 1);
        assert_eq!(editor.cursor().column, 0);

        editor.move_cursor_end();
        assert_eq!(editor.cursor().line, 1);
        assert_eq!(editor.cursor().column, 5);
    }
}
