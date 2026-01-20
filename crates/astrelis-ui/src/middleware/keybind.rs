//! Keybind registry for middleware shortcuts.
//!
//! Provides a system for registering and matching keyboard shortcuts
//! that can trigger middleware actions.

use astrelis_winit::event::KeyCode;
use bitflags::bitflags;

bitflags! {
    /// Keyboard modifier flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Modifiers: u8 {
        /// No modifiers pressed.
        const NONE = 0;
        /// Shift key is pressed.
        const SHIFT = 1 << 0;
        /// Control key is pressed (Cmd on macOS).
        const CTRL = 1 << 1;
        /// Alt key is pressed (Option on macOS).
        const ALT = 1 << 2;
        /// Super/Meta key (Windows key, Cmd on macOS).
        const SUPER = 1 << 3;
    }
}

impl Modifiers {
    /// Create modifiers from individual key states.
    pub fn from_keys(shift: bool, ctrl: bool, alt: bool, super_key: bool) -> Self {
        let mut mods = Modifiers::NONE;
        if shift {
            mods |= Modifiers::SHIFT;
        }
        if ctrl {
            mods |= Modifiers::CTRL;
        }
        if alt {
            mods |= Modifiers::ALT;
        }
        if super_key {
            mods |= Modifiers::SUPER;
        }
        mods
    }
}

/// A keyboard shortcut definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Keybind {
    /// The key that triggers this keybind.
    pub key: KeyCode,
    /// Required modifier keys.
    pub modifiers: Modifiers,
    /// Human-readable description of what this keybind does.
    pub description: &'static str,
}

impl Keybind {
    /// Create a new keybind.
    pub fn new(key: KeyCode, modifiers: Modifiers, description: &'static str) -> Self {
        Self {
            key,
            modifiers,
            description,
        }
    }

    /// Create a keybind with no modifiers.
    pub fn key(key: KeyCode, description: &'static str) -> Self {
        Self::new(key, Modifiers::NONE, description)
    }

    /// Create a keybind with Ctrl modifier.
    pub fn ctrl(key: KeyCode, description: &'static str) -> Self {
        Self::new(key, Modifiers::CTRL, description)
    }

    /// Create a keybind with Shift modifier.
    pub fn shift(key: KeyCode, description: &'static str) -> Self {
        Self::new(key, Modifiers::SHIFT, description)
    }

    /// Create a keybind with Ctrl+Shift modifiers.
    pub fn ctrl_shift(key: KeyCode, description: &'static str) -> Self {
        Self::new(key, Modifiers::CTRL | Modifiers::SHIFT, description)
    }

    /// Check if this keybind matches the given key and modifiers.
    pub fn matches(&self, key: KeyCode, modifiers: Modifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }

    /// Format this keybind as a human-readable string.
    pub fn to_string_short(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.contains(Modifiers::CTRL) {
            #[cfg(target_os = "macos")]
            parts.push("⌘");
            #[cfg(not(target_os = "macos"))]
            parts.push("Ctrl");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            #[cfg(target_os = "macos")]
            parts.push("⌥");
            #[cfg(not(target_os = "macos"))]
            parts.push("Alt");
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            #[cfg(target_os = "macos")]
            parts.push("⇧");
            #[cfg(not(target_os = "macos"))]
            parts.push("Shift");
        }
        if self.modifiers.contains(Modifiers::SUPER) {
            #[cfg(target_os = "macos")]
            parts.push("⌘");
            #[cfg(not(target_os = "macos"))]
            parts.push("Win");
        }

        parts.push(key_code_name(self.key));

        parts.join("+")
    }
}

/// Registry of keybinds for middlewares.
#[derive(Debug, Default)]
pub struct KeybindRegistry {
    /// Registered keybinds: (middleware_name, keybind, priority)
    keybinds: Vec<(&'static str, Keybind, i32)>,
}

impl KeybindRegistry {
    /// Create a new empty keybind registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a keybind for a middleware.
    ///
    /// Priority determines which middleware handles conflicts (higher wins).
    pub fn register(&mut self, middleware: &'static str, keybind: Keybind, priority: i32) {
        self.keybinds.push((middleware, keybind, priority));
        // Sort by priority descending so higher priority keybinds are checked first
        self.keybinds.sort_by(|a, b| b.2.cmp(&a.2));
    }

    /// Unregister all keybinds for a middleware.
    pub fn unregister(&mut self, middleware: &str) {
        self.keybinds.retain(|(name, _, _)| *name != middleware);
    }

    /// Find all keybinds that match the given key and modifiers.
    ///
    /// Returns matches in priority order (highest first).
    pub fn find_matches(&self, key: KeyCode, modifiers: Modifiers) -> Vec<(&str, &Keybind)> {
        self.keybinds
            .iter()
            .filter(|(_, keybind, _)| keybind.matches(key, modifiers))
            .map(|(name, keybind, _)| (*name, keybind))
            .collect()
    }

    /// Get all registered keybinds for a middleware.
    pub fn get_keybinds(&self, middleware: &'static str) -> Vec<&Keybind> {
        self.keybinds
            .iter()
            .filter(|(name, _, _)| *name == middleware)
            .map(|(_, keybind, _)| keybind)
            .collect()
    }

    /// Get all registered keybinds.
    pub fn all_keybinds(&self) -> impl Iterator<Item = (&'static str, &Keybind)> {
        self.keybinds.iter().map(|(name, keybind, _)| (*name, keybind))
    }

    /// Clear all registered keybinds.
    pub fn clear(&mut self) {
        self.keybinds.clear();
    }
}

/// Get a human-readable name for a key code.
fn key_code_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::Escape => "Esc",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::Backquote => "`",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Digit0 => "0",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::Backspace => "Backspace",
        KeyCode::Tab => "Tab",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyW => "W",
        KeyCode::KeyE => "E",
        KeyCode::KeyR => "R",
        KeyCode::KeyT => "T",
        KeyCode::KeyY => "Y",
        KeyCode::KeyU => "U",
        KeyCode::KeyI => "I",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Backslash => "\\",
        KeyCode::KeyA => "A",
        KeyCode::KeyS => "S",
        KeyCode::KeyD => "D",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "'",
        KeyCode::Enter => "Enter",
        KeyCode::KeyZ => "Z",
        KeyCode::KeyX => "X",
        KeyCode::KeyC => "C",
        KeyCode::KeyV => "V",
        KeyCode::KeyB => "B",
        KeyCode::KeyN => "N",
        KeyCode::KeyM => "M",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::Space => "Space",
        KeyCode::ArrowUp => "↑",
        KeyCode::ArrowDown => "↓",
        KeyCode::ArrowLeft => "←",
        KeyCode::ArrowRight => "→",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PgUp",
        KeyCode::PageDown => "PgDn",
        KeyCode::Insert => "Ins",
        KeyCode::Delete => "Del",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybind_creation() {
        let kb = Keybind::key(KeyCode::F12, "Toggle inspector");
        assert_eq!(kb.key, KeyCode::F12);
        assert_eq!(kb.modifiers, Modifiers::NONE);
        assert_eq!(kb.description, "Toggle inspector");
    }

    #[test]
    fn test_keybind_with_modifiers() {
        let kb = Keybind::ctrl_shift(KeyCode::KeyI, "Open inspector");
        assert!(kb.modifiers.contains(Modifiers::CTRL));
        assert!(kb.modifiers.contains(Modifiers::SHIFT));
        assert!(!kb.modifiers.contains(Modifiers::ALT));
    }

    #[test]
    fn test_keybind_matching() {
        let kb = Keybind::ctrl(KeyCode::KeyS, "Save");

        assert!(kb.matches(KeyCode::KeyS, Modifiers::CTRL));
        assert!(!kb.matches(KeyCode::KeyS, Modifiers::NONE));
        assert!(!kb.matches(KeyCode::KeyS, Modifiers::CTRL | Modifiers::SHIFT));
        assert!(!kb.matches(KeyCode::KeyA, Modifiers::CTRL));
    }

    #[test]
    fn test_registry_operations() {
        let mut registry = KeybindRegistry::new();

        registry.register("inspector", Keybind::key(KeyCode::F12, "Toggle"), 100);
        registry.register("inspector", Keybind::key(KeyCode::F5, "Freeze"), 100);
        registry.register("profiler", Keybind::key(KeyCode::F11, "Profile"), 50);

        // Find matches
        let matches = registry.find_matches(KeyCode::F12, Modifiers::NONE);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "inspector");

        // Get keybinds for middleware
        let inspector_binds = registry.get_keybinds("inspector");
        assert_eq!(inspector_binds.len(), 2);

        // Unregister
        registry.unregister("inspector");
        assert!(registry.get_keybinds("inspector").is_empty());
        assert_eq!(registry.get_keybinds("profiler").len(), 1);
    }

    #[test]
    fn test_registry_priority_ordering() {
        let mut registry = KeybindRegistry::new();

        // Register same key with different priorities
        registry.register("low", Keybind::key(KeyCode::F1, "Low priority"), 10);
        registry.register("high", Keybind::key(KeyCode::F1, "High priority"), 100);
        registry.register("medium", Keybind::key(KeyCode::F1, "Medium priority"), 50);

        let matches = registry.find_matches(KeyCode::F1, Modifiers::NONE);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].0, "high");
        assert_eq!(matches[1].0, "medium");
        assert_eq!(matches[2].0, "low");
    }

    #[test]
    fn test_keybind_to_string() {
        let simple = Keybind::key(KeyCode::F12, "Test");
        assert_eq!(simple.to_string_short(), "F12");

        let _with_ctrl = Keybind::ctrl(KeyCode::KeyS, "Save");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(_with_ctrl.to_string_short(), "Ctrl+S");

        let _with_shift = Keybind::ctrl_shift(KeyCode::KeyZ, "Redo");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(_with_shift.to_string_short(), "Ctrl+Shift+Z");
    }

    #[test]
    fn test_modifiers_bitflags() {
        let mods = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(mods.contains(Modifiers::CTRL));
        assert!(mods.contains(Modifiers::SHIFT));
        assert!(!mods.contains(Modifiers::ALT));
    }
}
