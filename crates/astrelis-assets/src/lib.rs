use std::any::{Any, TypeId};

use astrelis_core::alloc::{HashMap, sparse_set::IndexSlot};

pub struct Handle<T: Any> {
    slot: IndexSlot,
    _marker: std::marker::PhantomData<T>,
}

pub struct AssetManager {
    /// Maps TypeId to a Boxed Any containing a SparseSet of assets of that type
    assets: HashMap<TypeId, Box<dyn Any>>,
}
