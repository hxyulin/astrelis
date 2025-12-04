//! Resource management for the engine.
//!
//! Resources are shared data that can be accessed by plugins and game systems.
//! They provide a type-safe way to store and retrieve arbitrary data.

use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Marker trait for types that can be stored as engine resources.
///
/// Resources must be `Send + Sync` to allow safe access across threads.
pub trait Resource: Send + Sync + 'static {}

// Blanket implementation for all eligible types
impl<T: Send + Sync + 'static> Resource for T {}

/// Type-erased resource storage.
struct ResourceEntry {
    data: Box<dyn Any + Send + Sync>,
    type_name: &'static str,
}

/// Container for engine resources.
///
/// Resources are stored by type and can be accessed through typed getters.
/// Each resource type can only have one instance.
///
/// # Example
///
/// ```
/// use astrelis::resource::Resources;
///
/// struct GameConfig {
///     title: String,
///     max_fps: u32,
/// }
///
/// let mut resources = Resources::new();
/// resources.insert(GameConfig {
///     title: "My Game".to_string(),
///     max_fps: 60,
/// });
///
/// let config = resources.get::<GameConfig>().unwrap();
/// assert_eq!(config.title, "My Game");
/// ```
#[derive(Default)]
pub struct Resources {
    storage: HashMap<TypeId, ResourceEntry>,
}

impl Resources {
    /// Create a new empty resource container.
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    /// Insert a resource, replacing any existing resource of the same type.
    ///
    /// Returns the previous resource if one existed.
    pub fn insert<R: Resource>(&mut self, resource: R) -> Option<R> {
        let type_id = TypeId::of::<R>();
        let type_name = std::any::type_name::<R>();

        let entry = ResourceEntry {
            data: Box::new(resource),
            type_name,
        };

        self.storage.insert(type_id, entry).and_then(|old| {
            old.data.downcast::<R>().ok().map(|b| *b)
        })
    }

    /// Get a reference to a resource.
    pub fn get<R: Resource>(&self) -> Option<&R> {
        let type_id = TypeId::of::<R>();
        self.storage
            .get(&type_id)
            .and_then(|entry| entry.data.downcast_ref())
    }

    /// Get a mutable reference to a resource.
    pub fn get_mut<R: Resource>(&mut self) -> Option<&mut R> {
        let type_id = TypeId::of::<R>();
        self.storage
            .get_mut(&type_id)
            .and_then(|entry| entry.data.downcast_mut())
    }

    /// Remove a resource and return it.
    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let type_id = TypeId::of::<R>();
        self.storage.remove(&type_id).and_then(|entry| {
            entry.data.downcast::<R>().ok().map(|b| *b)
        })
    }

    /// Check if a resource exists.
    pub fn contains<R: Resource>(&self) -> bool {
        let type_id = TypeId::of::<R>();
        self.storage.contains_key(&type_id)
    }

    /// Get the number of resources.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Check if there are no resources.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Clear all resources.
    pub fn clear(&mut self) {
        self.storage.clear();
    }

    /// Get or insert a resource with a default value.
    pub fn get_or_insert_with<R: Resource>(&mut self, f: impl FnOnce() -> R) -> &mut R {
        let type_id = TypeId::of::<R>();

        if !self.storage.contains_key(&type_id) {
            self.insert(f());
        }

        self.get_mut::<R>().unwrap()
    }

    /// Get or insert a resource with its default value.
    pub fn get_or_default<R: Resource + Default>(&mut self) -> &mut R {
        self.get_or_insert_with(R::default)
    }

    /// List all resource type names (for debugging).
    pub fn type_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.storage.values().map(|entry| entry.type_name)
    }
}

impl std::fmt::Debug for Resources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resources")
            .field("count", &self.storage.len())
            .field("types", &self.type_names().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut resources = Resources::new();
        resources.insert(42i32);
        resources.insert("hello".to_string());

        assert_eq!(*resources.get::<i32>().unwrap(), 42);
        assert_eq!(resources.get::<String>().unwrap(), "hello");
    }

    #[test]
    fn test_get_mut() {
        let mut resources = Resources::new();
        resources.insert(vec![1, 2, 3]);

        resources.get_mut::<Vec<i32>>().unwrap().push(4);
        assert_eq!(resources.get::<Vec<i32>>().unwrap(), &vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_replace() {
        let mut resources = Resources::new();
        resources.insert(10i32);
        let old = resources.insert(20i32);

        assert_eq!(old, Some(10));
        assert_eq!(*resources.get::<i32>().unwrap(), 20);
    }

    #[test]
    fn test_remove() {
        let mut resources = Resources::new();
        resources.insert(42i32);

        let removed = resources.remove::<i32>();
        assert_eq!(removed, Some(42));
        assert!(resources.get::<i32>().is_none());
    }

    #[test]
    fn test_contains() {
        let mut resources = Resources::new();
        assert!(!resources.contains::<i32>());

        resources.insert(42i32);
        assert!(resources.contains::<i32>());
    }

    #[test]
    fn test_get_or_default() {
        let mut resources = Resources::new();

        let val = resources.get_or_default::<Vec<i32>>();
        val.push(1);

        assert_eq!(resources.get::<Vec<i32>>().unwrap(), &vec![1]);
    }

    #[test]
    fn test_get_or_insert_with() {
        let mut resources = Resources::new();
        let mut called = false;

        resources.get_or_insert_with(|| {
            called = true;
            42i32
        });
        assert!(called);

        called = false;
        resources.get_or_insert_with(|| {
            called = true;
            100i32
        });
        assert!(!called); // Should not be called again
        assert_eq!(*resources.get::<i32>().unwrap(), 42);
    }
}
