use super::{Component, Entity, Registry, Storage};

pub trait Query<'a>: Sized + Iterator {
    fn fetch(reg: &'a Registry) -> Self;
}

pub trait QueryMut<'a>: Sized + Iterator {
    fn fetch_mut(reg: &'a mut Registry) -> Self;
}

pub struct Query1<'a, T> {
    store: Option<&'a Storage<T>>,
    pos: usize,
}

impl<'a, T> Iterator for Query1<'a, T>
where
    T: Component,
{
    type Item = (Entity, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(store) = self.store {
            while self.pos < store.ids.len() {
                let ent_index = self.pos;
                self.pos += 1;

                if let Some(slot) = store.ids[ent_index] {
                    let comp = store.comps.get(slot);
                    return Some((Entity(ent_index as u64), comp));
                }
            }
        }
        None
    }
}

impl<'a, T> Query<'a> for Query1<'a, T>
where
    T: Component,
{
    fn fetch(reg: &'a Registry) -> Self {
        let store = reg.get_storage::<T>();
        Self { store, pos: 0 }
    }
}

macro_rules! query_impl {
    ($name:ident, $old: ident, $new: ident, $($ty:ident),+) => {
        pub struct $name<'a, $new, $($ty),+>
        where
            $new: Component,
            $( $ty: Component ),+
        {
            inner: $old<'a, $($ty),+>,
            store: Option<&'a Storage<$new>>,
        }

        impl<'a, $new, $($ty),+> Iterator for $name<'a, $new, $($ty),+>
        where
            $new: Component,
            $( $ty: Component ),+
        {
            type Item = (Entity, &'a $new, $(&'a $ty),+);

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(store) = self.store {
                    #[allow(non_snake_case)]
                    while let Some((ent, $($ty),+)) = self.inner.next() {
                        let ent_idx = ent.0 as usize;
                        if store.ids.len() <= ent_idx {
                            continue;
                        }
                        if let Some(slot) = store.ids[ent_idx] {
                            let comp = store.comps.get(slot);
                            return Some((ent, comp, $($ty),+));
                        }
                    }
                }
                None
            }
        }

        impl<'a, $new, $($ty),+> Query<'a> for $name<'a, $new, $($ty),+>
        where
            $new: Component,
            $( $ty: Component ),+
        {
            fn fetch(reg: &'a Registry) -> Self {
                let inner = $old::fetch(reg);
                let store = reg.get_storage::<$new>();
                Self {
                    inner, store
                }
            }
        }
    }
}

query_impl!(Query2, Query1, A, B);
query_impl!(Query3, Query2, A, B, C);
query_impl!(Query4, Query3, A, B, C, D);

pub trait QueryDef<'a> {
    type Query: Query<'a>;
    fn make(reg: &'a Registry) -> Self::Query;
}

macro_rules! querydef_impl {
    ($name:ident, $($ty:ident),+) => {
        impl<'a, $($ty),+> QueryDef<'a> for ($($ty),+,)
            where $( $ty: Component ),+
        {
            type Query = $name<'a, $($ty),+>;
            fn make(reg: &'a Registry) -> Self::Query {
                $name::fetch(reg)
            }
        }
    };
}

querydef_impl!(Query1, A);
querydef_impl!(Query2, A, B);
querydef_impl!(Query3, A, B, C);
querydef_impl!(Query4, A, B, C, D);

// Special implementation for single element
impl<'a, T> QueryDef<'a> for T
where
    T: Component,
{
    type Query = Query1<'a, T>;

    fn make(reg: &'a Registry) -> Self::Query {
        Query1::fetch(reg)
    }
}

// Mutable query implementations
pub struct Query1Mut<'a, T> {
    store: Option<&'a mut Storage<T>>,
    pos: usize,
}

impl<'a, T> Iterator for Query1Mut<'a, T>
where
    T: Component,
{
    type Item = (Entity, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(store) = self.store.as_mut() {
            while self.pos < store.ids.len() {
                let ent_index = self.pos;
                self.pos += 1;

                if let Some(slot) = store.ids[ent_index] {
                    // SAFETY: We're only handing out one mutable reference at a time
                    // through the iterator, and we never revisit the same entity
                    let comp = unsafe {
                        let ptr = store.comps.get_mut(slot) as *mut T;
                        &mut *ptr
                    };
                    return Some((Entity(ent_index as u64), comp));
                }
            }
        }
        None
    }
}

impl<'a, T> QueryMut<'a> for Query1Mut<'a, T>
where
    T: Component,
{
    fn fetch_mut(reg: &'a mut Registry) -> Self {
        let store = reg.get_storage_mut::<T>();
        Self { store, pos: 0 }
    }
}

// Mutable query for 2 components
pub struct Query2Mut<'a, A, B>
where
    A: Component,
    B: Component,
{
    store_a: Option<*mut Storage<A>>,
    store_b: Option<*mut Storage<B>>,
    pos: usize,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

impl<'a, A, B> Iterator for Query2Mut<'a, A, B>
where
    A: Component,
    B: Component,
{
    type Item = (Entity, &'a mut A, &'a mut B);

    fn next(&mut self) -> Option<Self::Item> {
        let (store_a, store_b) = (self.store_a?, self.store_b?);

        let min_len = unsafe { (*store_a).ids.len().min((*store_b).ids.len()) };

        while self.pos < min_len {
            let ent_index = self.pos;
            self.pos += 1;

            let (slot_a, slot_b) = unsafe {
                (
                    (&*store_a).ids.get(ent_index).and_then(|s| *s),
                    (&*store_b).ids.get(ent_index).and_then(|s| *s),
                )
            };

            if let (Some(slot_a), Some(slot_b)) = (slot_a, slot_b) {
                let (comp_a, comp_b) = unsafe {
                    (
                        &mut *((&mut *store_a).comps.get_mut(slot_a) as *mut A),
                        &mut *((&mut *store_b).comps.get_mut(slot_b) as *mut B),
                    )
                };
                return Some((Entity(ent_index as u64), comp_a, comp_b));
            }
        }
        None
    }
}

impl<'a, A, B> QueryMut<'a> for Query2Mut<'a, A, B>
where
    A: Component,
    B: Component,
{
    fn fetch_mut(reg: &'a mut Registry) -> Self {
        Self {
            store_a: reg.get_storage_mut::<A>().map(|s| s as *mut Storage<A>),
            store_b: reg.get_storage_mut::<B>().map(|s| s as *mut Storage<B>),
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

// Mutable query for 3 components
pub struct Query3Mut<'a, A, B, C>
where
    A: Component,
    B: Component,
    C: Component,
{
    store_a: Option<*mut Storage<A>>,
    store_b: Option<*mut Storage<B>>,
    store_c: Option<*mut Storage<C>>,
    pos: usize,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

impl<'a, A, B, C> Iterator for Query3Mut<'a, A, B, C>
where
    A: Component,
    B: Component,
    C: Component,
{
    type Item = (Entity, &'a mut A, &'a mut B, &'a mut C);

    fn next(&mut self) -> Option<Self::Item> {
        let (store_a, store_b, store_c) = (self.store_a?, self.store_b?, self.store_c?);

        let min_len = unsafe {
            (*store_a)
                .ids
                .len()
                .min((*store_b).ids.len())
                .min((*store_c).ids.len())
        };

        while self.pos < min_len {
            let ent_index = self.pos;
            self.pos += 1;

            let (slot_a, slot_b, slot_c) = unsafe {
                (
                    (&*store_a).ids.get(ent_index).and_then(|s| *s),
                    (&*store_b).ids.get(ent_index).and_then(|s| *s),
                    (&*store_c).ids.get(ent_index).and_then(|s| *s),
                )
            };

            if let (Some(slot_a), Some(slot_b), Some(slot_c)) = (slot_a, slot_b, slot_c) {
                let (comp_a, comp_b, comp_c) = unsafe {
                    (
                        &mut *((&mut *store_a).comps.get_mut(slot_a) as *mut A),
                        &mut *((&mut *store_b).comps.get_mut(slot_b) as *mut B),
                        &mut *((&mut *store_c).comps.get_mut(slot_c) as *mut C),
                    )
                };
                return Some((Entity(ent_index as u64), comp_a, comp_b, comp_c));
            }
        }
        None
    }
}

impl<'a, A, B, C> QueryMut<'a> for Query3Mut<'a, A, B, C>
where
    A: Component,
    B: Component,
    C: Component,
{
    fn fetch_mut(reg: &'a mut Registry) -> Self {
        Self {
            store_a: reg.get_storage_mut::<A>().map(|s| s as *mut Storage<A>),
            store_b: reg.get_storage_mut::<B>().map(|s| s as *mut Storage<B>),
            store_c: reg.get_storage_mut::<C>().map(|s| s as *mut Storage<C>),
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

// Mutable query for 4 components
pub struct Query4Mut<'a, A, B, C, D>
where
    A: Component,
    B: Component,
    C: Component,
    D: Component,
{
    store_a: Option<*mut Storage<A>>,
    store_b: Option<*mut Storage<B>>,
    store_c: Option<*mut Storage<C>>,
    store_d: Option<*mut Storage<D>>,
    pos: usize,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

impl<'a, A, B, C, D> Iterator for Query4Mut<'a, A, B, C, D>
where
    A: Component,
    B: Component,
    C: Component,
    D: Component,
{
    type Item = (Entity, &'a mut A, &'a mut B, &'a mut C, &'a mut D);

    fn next(&mut self) -> Option<Self::Item> {
        let (store_a, store_b, store_c, store_d) =
            (self.store_a?, self.store_b?, self.store_c?, self.store_d?);

        let min_len = unsafe {
            (*store_a)
                .ids
                .len()
                .min((*store_b).ids.len())
                .min((*store_c).ids.len())
                .min((*store_d).ids.len())
        };

        while self.pos < min_len {
            let ent_index = self.pos;
            self.pos += 1;

            let (slot_a, slot_b, slot_c, slot_d) = unsafe {
                (
                    (&*store_a).ids.get(ent_index).and_then(|s| *s),
                    (&*store_b).ids.get(ent_index).and_then(|s| *s),
                    (&*store_c).ids.get(ent_index).and_then(|s| *s),
                    (&*store_d).ids.get(ent_index).and_then(|s| *s),
                )
            };

            if let (Some(slot_a), Some(slot_b), Some(slot_c), Some(slot_d)) =
                (slot_a, slot_b, slot_c, slot_d)
            {
                let (comp_a, comp_b, comp_c, comp_d) = unsafe {
                    (
                        &mut *((&mut *store_a).comps.get_mut(slot_a) as *mut A),
                        &mut *((&mut *store_b).comps.get_mut(slot_b) as *mut B),
                        &mut *((&mut *store_c).comps.get_mut(slot_c) as *mut C),
                        &mut *((&mut *store_d).comps.get_mut(slot_d) as *mut D),
                    )
                };
                return Some((Entity(ent_index as u64), comp_a, comp_b, comp_c, comp_d));
            }
        }
        None
    }
}

impl<'a, A, B, C, D> QueryMut<'a> for Query4Mut<'a, A, B, C, D>
where
    A: Component,
    B: Component,
    C: Component,
    D: Component,
{
    fn fetch_mut(reg: &'a mut Registry) -> Self {
        Self {
            store_a: reg.get_storage_mut::<A>().map(|s| s as *mut Storage<A>),
            store_b: reg.get_storage_mut::<B>().map(|s| s as *mut Storage<B>),
            store_c: reg.get_storage_mut::<C>().map(|s| s as *mut Storage<C>),
            store_d: reg.get_storage_mut::<D>().map(|s| s as *mut Storage<D>),
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

pub trait QueryDefMut<'a> {
    type Query: QueryMut<'a>;
    fn make_mut(reg: &'a mut Registry) -> Self::Query;
}

macro_rules! querydef_mut_impl {
    ($name:ident, $($ty:ident),+) => {
        impl<'a, $($ty),+> QueryDefMut<'a> for ($($ty),+,)
            where $( $ty: Component ),+
        {
            type Query = $name<'a, $($ty),+>;
            fn make_mut(reg: &'a mut Registry) -> Self::Query {
                $name::fetch_mut(reg)
            }
        }
    };
}

querydef_mut_impl!(Query1Mut, A);
querydef_mut_impl!(Query2Mut, A, B);
querydef_mut_impl!(Query3Mut, A, B, C);
querydef_mut_impl!(Query4Mut, A, B, C, D);

impl<'a, T> QueryDefMut<'a> for T
where
    T: Component,
{
    type Query = Query1Mut<'a, T>;

    fn make_mut(reg: &'a mut Registry) -> Self::Query {
        Query1Mut::fetch_mut(reg)
    }
}
