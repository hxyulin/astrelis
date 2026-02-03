//! Versioned value wrapper for cache invalidation and change tracking.

/// Versioned value that auto-bumps version on changes.
///
/// Used to track value changes for cache invalidation and
/// automatic dirty marking.
///
/// # Example
/// ```ignore
/// let mut value = Versioned::new("Hello".to_string());
/// assert_eq!(value.version(), 0);
///
/// value.set("World".to_string());
/// assert_eq!(value.version(), 1); // Auto-incremented
/// ```
#[derive(Debug, Clone)]
pub struct Versioned<T> {
    value: T,
    version: u32,
}

impl<T> Versioned<T> {
    /// Create a new versioned value.
    pub fn new(value: T) -> Self {
        Self { value, version: 0 }
    }

    /// Get the current value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Get a mutable reference to the value without bumping the version.
    ///
    /// Use this when you need to modify the value but will handle version
    /// tracking manually.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Get the current version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Check if this value is newer than a cached version.
    pub fn is_newer_than(&self, cached_version: u32) -> bool {
        self.version != cached_version
    }

    /// Set the value without checking for changes, always bumping the version.
    pub fn set_unchecked(&mut self, new_value: T) {
        self.value = new_value;
        self.version = self.version.wrapping_add(1);
    }
}

impl<T: PartialEq> Versioned<T> {
    /// Set new value, incrementing version if changed.
    ///
    /// Returns `true` if the value changed.
    pub fn set(&mut self, new_value: T) -> bool {
        if self.value != new_value {
            self.value = new_value;
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }
}

impl<T: Default> Default for Versioned<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// Convenience impls for Versioned<String>

impl Versioned<String> {
    /// Get the string content as a str slice.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl From<String> for Versioned<String> {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for Versioned<String> {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl AsRef<str> for Versioned<String> {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for Versioned<String> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_versioned_string() {
        let mut value = Versioned::new("Hello".to_string());
        assert_eq!(value.version(), 0);
        assert_eq!(value.get(), "Hello");

        assert!(value.set("World".to_string()));
        assert_eq!(value.version(), 1);
        assert_eq!(value.get(), "World");

        assert!(!value.set("World".to_string()));
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn test_versioned_is_newer() {
        let mut value = Versioned::new("Test".to_string());
        assert!(!value.is_newer_than(0));

        value.set("New".to_string());
        assert!(value.is_newer_than(0));
        assert!(!value.is_newer_than(1));
    }

    #[test]
    fn test_versioned_numeric() {
        let mut value = Versioned::new(42);
        assert_eq!(*value.get(), 42);
        assert_eq!(value.version(), 0);

        assert!(value.set(100));
        assert_eq!(*value.get(), 100);
        assert_eq!(value.version(), 1);

        assert!(!value.set(100));
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn test_versioned_set_unchecked() {
        let mut value = Versioned::new(42);
        value.set_unchecked(42); // Same value but version still bumps
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn test_versioned_from_str() {
        let value: Versioned<String> = "hello".into();
        assert_eq!(value.as_str(), "hello");
        assert_eq!(value.version(), 0);
    }

    #[test]
    fn test_versioned_display() {
        let value = Versioned::new("test".to_string());
        assert_eq!(format!("{}", value), "test");
    }
}
