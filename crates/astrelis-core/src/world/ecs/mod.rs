use std::{
    any::{Any, TypeId},
    collections::HashMap,
    mem::MaybeUninit,
    num::NonZeroU64,
};

mod query;
pub use query::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity(u64);

pub struct Registry {
    next: u64,
    comps: HashMap<TypeId, Box<dyn Any>>,
}

pub trait Component: Any + Default {}

impl Registry {
    pub fn new() -> Self {
        Self {
            next: 0,
            comps: HashMap::new(),
        }
    }

    pub fn query<'a, Q>(&'a self) -> Option<<Q as QueryDef<'a>>::Query>
    where
        Q: QueryDef<'a>,
    {
        Q::make(self)
    }

    pub fn new_entity(&mut self) -> Entity {
        self.next += 1;
        Entity(self.next - 1)
    }

    fn get_or_create_storage<T: Component>(&mut self) -> &mut Storage<T> {
        let ty = T::default().type_id();
        if self.comps.contains_key(&ty) {
            self.comps
                .get_mut(&T::default().type_id())
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
        let storage = self.get_or_create_storage::<T>();
        storage.insert(ent, comp);
    }

    pub fn get_component<T: Component>(&self, ent: Entity) -> Option<&T> {
        let storage = self.get_storage::<T>()?;
        storage.get(ent)
    }

    pub fn get_storage<T: Component>(&self) -> Option<&Storage<T>> {
        let registry = self.comps.get(&T::default().type_id())?;
        Some(registry.as_ref().downcast_ref::<Storage<T>>().unwrap())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexSlot(NonZeroU64);

impl IndexSlot {
    pub const fn new(generation: u32, idx: u32) -> Self {
        Self(unsafe {
            NonZeroU64::new(((generation as u64) << 32) | (idx as u64 + 1)).unwrap_unchecked()
        })
    }

    pub fn generation(&self) -> u32 {
        (self.0.get() >> 32) as u32
    }

    pub fn index(&self) -> u32 {
        (self.0.get() & u32::MAX as u64) as u32 - 1
    }
}

pub struct Entry<T> {
    generation: u32,
    data: MaybeUninit<T>,
}

impl<T> Entry<T> {
    pub const fn new(data: T) -> Self {
        Self {
            generation: 0,
            data: MaybeUninit::new(data),
        }
    }
}

pub struct SparseSet<T> {
    vec: Vec<Entry<T>>,
    free: Vec<u32>,
}

impl<T> SparseSet<T> {
    pub const fn new() -> Self {
        Self {
            vec: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn push(&mut self, data: T) -> IndexSlot {
        if let Some(idx) = self.free.pop() {
            let entry = self.vec.get_mut(idx as usize).unwrap();
            entry.data = MaybeUninit::new(data);
            IndexSlot::new(entry.generation, idx)
        } else {
            let idx = self.vec.len();
            self.vec.push(Entry::new(data));
            IndexSlot::new(0, idx as u32)
        }
    }

    pub fn get(&self, idx: IndexSlot) -> &T {
        let entry = self.vec.get(idx.index() as usize).unwrap();
        assert_eq!(
            entry.generation,
            idx.generation(),
            "invalid generation, use after free!"
        );
        unsafe { entry.data.assume_init_ref() }
    }

    pub fn remove(&mut self, idx: IndexSlot) -> T {
        let index = idx.index();
        let entry = self.vec.get_mut(index as usize).unwrap();
        assert_eq!(
            entry.generation,
            idx.generation(),
            "invalid generation, use after free!"
        );
        let data = unsafe { entry.data.assume_init_read() };
        entry.generation += 1;
        entry.data = MaybeUninit::uninit();
        self.free.push(index);
        data
    }
}

// TODO: replace with static_assertions crate
const _IDX_SLOT_SIZE_CHECK: usize = size_of::<IndexSlot>() - size_of::<Option<IndexSlot>>();

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
        let idx = self.ids.get(ent.0 as usize)?.as_ref()?;
        // If idx exists, it should exist in comps
        Some(self.comps.get(*idx))
    }

    pub fn insert(&mut self, ent: Entity, comp: T) {
        let idx = self.comps.push(comp);
        let ent = ent.0 as usize;
        if ent >= self.ids.len() {
            // TODO: Maybe optimize performance, resize by a fixed size
            self.ids.resize(ent + 1, None);
        }
        self.ids[ent] = Some(idx);
    }

    pub fn remove(&mut self, ent: Entity) -> Option<T> {
        let idx = self.ids.remove(ent.0 as usize)?;
        Some(self.comps.remove(idx))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sparse_set_push() {
        let mut set = SparseSet::<u8>::new();
        let idx = set.push(15);
        assert_eq!(idx.generation(), 0);
        assert_eq!(idx.index(), 0);
        assert_eq!(*set.get(idx), 15);
    }

    #[test]
    #[should_panic]
    fn test_sparse_set_uaf() {
        let mut set = SparseSet::<u8>::new();
        let _ = set.push(15);
        // Create index slot with invalid generation
        let idx = IndexSlot::new(1, 0);
        let _ = set.get(idx);
    }

    #[test]
    fn test_sparse_set_remove() {
        let mut set = SparseSet::<u8>::new();
        let idx = set.push(15);
        set.remove(idx.clone());
        let new_idx = set.push(45);
        assert_eq!(idx.index(), new_idx.index());
        assert_ne!(idx.generation(), new_idx.generation());
    }

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
    fn test_query1() {
        let mut reg = Registry::new();

        // create three entities: only two get C1
        let e0 = reg.new_entity();
        let _e1 = reg.new_entity();
        let e2 = reg.new_entity();

        reg.add_component(e0, C1(10));
        reg.add_component(e2, C1(30));

        // query for all (Entity, &C1) pairs
        let qs = reg.query::<C1>().expect("should get a Query1<C1>");
        let results: Vec<_> = qs.collect();

        // We expect exactly the two we inserted, in order of entity ID:
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

        let qs = reg
            .query::<(C1, C2)>()
            .expect("should get a Query2<C1, C2>");
        let results: Vec<_> = qs.collect();
        assert_eq!(results.len(), 2);
        assert_eq!(results, vec![
            (e1, &C1(11), &C2(20)),
            (e2, &C1(12), &C2(21)),
        ])
    }
}
