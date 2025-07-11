use crate::{
    alloc::{IndexSlot, SparseSet},
    profiling::profile_function,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt::Debug,
};

mod component;
mod query;
mod transform;
pub use component::*;
pub use query::*;
pub use transform::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity(u64);

pub struct Registry {
    next: u64,
    comps: HashMap<TypeId, Box<dyn Any>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            next: 0,
            comps: HashMap::new(),
        }
    }

    pub fn spawn<C: ComponentRef>(&mut self, comps: C) -> Entity {
        let ent = self.new_entity();
        comps.add_components(ent, self);
        ent
    }

    pub fn query<'a, Q>(&'a self) -> <Q as QueryDef<'a>>::Query
    where
        Q: QueryDef<'a>,
    {
        profile_function!();
        Q::make(self)
    }

    pub fn new_entity(&mut self) -> Entity {
        profile_function!();
        self.next += 1;
        Entity(self.next - 1)
    }

    fn get_or_create_storage<T: Component>(&mut self) -> &mut Storage<T> {
        profile_function!();
        let ty = TypeId::of::<T>();
        if self.comps.contains_key(&ty) {
            self.comps
                .get_mut(&ty)
                .unwrap()
                .as_mut()
                .downcast_mut::<Storage<T>>()
                .unwrap()
        } else {
            self.comps.insert(ty.clone(), Box::new(Storage::<T>::new()));
            self.comps
                .get_mut(&ty)
                .unwrap()
                .as_mut()
                .downcast_mut()
                .unwrap()
        }
    }

    pub fn add_component<T: Component>(&mut self, ent: Entity, comp: T) {
        profile_function!();
        let storage = self.get_or_create_storage::<T>();
        storage.insert(ent, comp);
    }

    pub fn get_component<T: Component>(&self, ent: Entity) -> Option<&T> {
        profile_function!();
        let storage = self.get_storage::<T>()?;
        storage.get(ent)
    }

    pub fn remove_component<T: Component>(&mut self, ent: Entity) -> Option<T> {
        profile_function!();
        let storage = self.get_storage_mut::<T>()?;
        storage.remove(ent)
    }

    pub fn get_storage<T: Component>(&self) -> Option<&Storage<T>> {
        profile_function!();
        let registry = self.comps.get(&TypeId::of::<T>())?;
        Some(registry.as_ref().downcast_ref::<Storage<T>>().unwrap())
    }

    pub fn get_storage_mut<T: Component>(&mut self) -> Option<&mut Storage<T>> {
        profile_function!();
        let registry = self.comps.get_mut(&TypeId::of::<T>())?;
        Some(registry.as_mut().downcast_mut::<Storage<T>>().unwrap())
    }
}

pub struct Storage<T> {
    ids: Vec<Option<IndexSlot>>,
    comps: SparseSet<T>,
}

impl<T> Storage<T> {
    pub const fn new() -> Self {
        Self {
            ids: Vec::new(),
            comps: SparseSet::new(),
        }
    }
    /// Gets a component based on the entity
    /// This is constant time
    pub fn get(&self, ent: Entity) -> Option<&T> {
        profile_function!();
        let idx = self.ids.get(ent.0 as usize)?.as_ref()?;
        // If idx exists, it should exist in comps
        Some(self.comps.get(*idx))
    }

    pub fn insert(&mut self, ent: Entity, comp: T) {
        profile_function!();
        let idx = self.comps.push(comp);
        let ent = ent.0 as usize;
        if ent >= self.ids.len() {
            let mut new_len = self.ids.len();
            while new_len <= ent {
                // + 1 to account for edge case of 1 * 3 / 2 = 1
                new_len = new_len * 3 / 2 + 1;
            }
            self.ids.resize(new_len, None);
        }
        self.ids[ent] = Some(idx);
    }

    pub fn remove(&mut self, ent: Entity) -> Option<T> {
        profile_function!();
        let idx = self.ids.remove(ent.0 as usize)?;
        Some(self.comps.remove(idx))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_storage_insert() {
        let mut storage = Storage::<u8>::new();
        storage.insert(Entity(10), 42);
        storage.insert(Entity(410), 59);
        assert_eq!(storage.get(Entity(10)), Some(&42));
        assert_eq!(storage.get(Entity(410)), Some(&59));
    }

    #[test]
    fn test_storage_remove() {
        let mut storage = Storage::<u8>::new();
        storage.insert(Entity(10), 42);
        assert_eq!(storage.get(Entity(10)), Some(&42));
        let comp = storage.remove(Entity(10));
        assert_eq!(comp, Some(42));
        assert_eq!(storage.get(Entity(10)), None);
    }

    impl Component for u8 {}

    #[test]
    fn test_registry_create() {
        let mut registry = Registry::new();
        let ent = registry.new_entity();
        registry.add_component(ent, 42u8);
        let comp = registry.get_component::<u8>(ent);
        assert_eq!(comp, Some(&42));
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    struct C1(u32);
    impl Component for C1 {}

    #[derive(Debug, Default, PartialEq, Eq)]
    struct C2(u32);
    impl Component for C2 {}

    #[test]
    fn test_spawn1() {
        let mut reg = Registry::new();

        let e0 = reg.spawn(C1(13));
        let e1 = reg.spawn((C2(11),));
        let res0: Vec<_> = reg.query::<C1>().collect();
        assert_eq!(res0.len(), 1);
        assert_eq!(res0[0], (e0, &C1(13)));

        let res1: Vec<_> = reg.query::<C2>().collect();
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0], (e1, &C2(11)));
    }

    #[test]
    fn test_query1() {
        let mut reg = Registry::new();

        let e0 = reg.new_entity();
        let _e1 = reg.new_entity();
        let e2 = reg.new_entity();

        reg.add_component(e0, C1(10));
        reg.add_component(e2, C1(30));

        let qs = reg.query::<C1>();
        let results: Vec<_> = qs.collect();

        assert_eq!(results.len(), 2);
        assert_eq!(results, vec![(e0, &C1(10)), (e2, &C1(30))]);
    }

    #[test]
    fn test_query2() {
        let mut reg = Registry::new();

        let e0 = reg.new_entity();
        let e1 = reg.new_entity();
        let e2 = reg.new_entity();
        let e3 = reg.new_entity();

        reg.add_component(e0, C1(10));
        reg.add_component(e1, C1(11));
        reg.add_component(e1, C2(20));
        reg.add_component(e2, C1(12));
        reg.add_component(e2, C2(21));
        reg.add_component(e3, C2(22));

        let qs = reg.query::<(C1, C2)>();
        let results: Vec<_> = qs.collect();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results,
            vec![(e1, &C1(11), &C2(20)), (e2, &C1(12), &C2(21)),]
        )
    }
}
