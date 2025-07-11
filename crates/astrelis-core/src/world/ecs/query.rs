use super::{Component, Entity, Registry, Storage};

pub trait Query<'a>: Sized + Iterator {
    fn fetch(reg: &'a Registry) -> Self;
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
