//! Component trait and columnar storage.

use std::any::{Any, TypeId};

use slotmap::SecondaryMap;

use crate::node::NodeId;
use crate::scene::Scene;

/// Marker for data attachable to scene nodes.
///
/// Blanket-implemented for every `Send + Sync + 'static` type — any
/// plain struct works as a component with no derive or registration.
pub trait Component: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Component for T {}

/// Object-safe interface over one per-type component column.
pub(crate) trait ComponentColumn: Send + Sync {
    /// Removes `id`'s component from this column, if present.
    fn remove(&mut self, id: NodeId);
    /// Upcast for typed downcasting.
    fn as_any(&self) -> &dyn Any;
    /// Upcast for typed downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Component> ComponentColumn for SecondaryMap<NodeId, T> {
    fn remove(&mut self, id: NodeId) {
        SecondaryMap::remove(self, id);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Scene {
    fn column<T: Component>(&self) -> Option<&SecondaryMap<NodeId, T>> {
        self.columns
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref()
    }

    fn column_mut<T: Component>(&mut self) -> Option<&mut SecondaryMap<NodeId, T>> {
        self.columns
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut()
    }

    /// Attaches `value` to node `id`, replacing and returning any
    /// existing `T`. Returns `None` (without storing) if `id` is stale.
    ///
    /// The column for `T` is created on first insert.
    pub fn insert<T: Component>(&mut self, id: NodeId, value: T) -> Option<T> {
        if !self.nodes.contains_key(id) {
            return None;
        }
        let column = self
            .columns
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(SecondaryMap::<NodeId, T>::new()));
        column
            .as_any_mut()
            .downcast_mut::<SecondaryMap<NodeId, T>>()
            .expect("column type matches TypeId key")
            .insert(id, value)
    }

    /// Node `id`'s `T` component, if the node is live and has one.
    pub fn get<T: Component>(&self, id: NodeId) -> Option<&T> {
        self.column::<T>()?.get(id)
    }

    /// Mutable access to node `id`'s `T` component.
    pub fn get_mut<T: Component>(&mut self, id: NodeId) -> Option<&mut T> {
        self.column_mut::<T>()?.get_mut(id)
    }

    /// Detaches and returns node `id`'s `T` component.
    pub fn remove<T: Component>(&mut self, id: NodeId) -> Option<T> {
        self.column_mut::<T>()?.remove(id)
    }

    /// Iterates all `(NodeId, &T)` pairs — O(number of `T` components),
    /// independent of total node count.
    pub fn iter<T: Component>(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.column::<T>().into_iter().flatten()
    }

    /// Iterates all `(NodeId, &mut T)` pairs.
    pub fn iter_mut<T: Component>(&mut self) -> impl Iterator<Item = (NodeId, &mut T)> {
        self.column_mut::<T>().into_iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::Scene;

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    #[derive(Debug, PartialEq)]
    struct Tag;

    #[test]
    fn insert_get_remove_roundtrip() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        assert_eq!(scene.insert(id, Health(10)), None);
        assert_eq!(scene.get::<Health>(id), Some(&Health(10)));
        scene.get_mut::<Health>(id).unwrap().0 = 20;
        assert_eq!(scene.remove::<Health>(id), Some(Health(20)));
        assert_eq!(scene.get::<Health>(id), None);
    }

    #[test]
    fn insert_replaces_and_returns_old_value() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.insert(id, Health(1));
        assert_eq!(scene.insert(id, Health(2)), Some(Health(1)));
        assert_eq!(scene.get::<Health>(id), Some(&Health(2)));
    }

    #[test]
    fn stale_id_is_rejected() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.despawn(id);
        assert_eq!(scene.insert(id, Health(1)), None);
        assert_eq!(scene.get::<Health>(id), None);
        assert_eq!(scene.remove::<Health>(id), None);
    }

    #[test]
    fn missing_column_returns_none_and_empty_iter() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        assert_eq!(scene.get::<Health>(id), None);
        assert_eq!(scene.iter::<Health>().count(), 0);
    }

    #[test]
    fn iter_yields_only_nodes_with_component() {
        let mut scene = Scene::new();
        let a = scene.spawn().with(Health(1)).id();
        let _no_health = scene.spawn().with(Tag).id();
        let b = scene.spawn().with(Health(2)).id();
        let mut found: Vec<_> = scene.iter::<Health>().map(|(id, h)| (id, h.0)).collect();
        found.sort();
        let mut expected = vec![(a, 1), (b, 2)];
        expected.sort();
        assert_eq!(found, expected);
    }

    #[test]
    fn iter_mut_mutates_in_place() {
        let mut scene = Scene::new();
        let id = scene.spawn().with(Health(1)).id();
        for (_, h) in scene.iter_mut::<Health>() {
            h.0 += 10;
        }
        assert_eq!(scene.get::<Health>(id), Some(&Health(11)));
    }

    #[test]
    fn despawn_clears_all_columns_for_subtree() {
        let mut scene = Scene::new();
        let parent = scene.spawn().with(Health(1)).id();
        let _child = scene.spawn_child(parent).with(Health(2)).with(Tag).id();
        scene.despawn(parent);
        assert_eq!(scene.iter::<Health>().count(), 0);
        assert_eq!(scene.iter::<Tag>().count(), 0);
    }
}
