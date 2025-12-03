use crate::profiling::profile_function;
use std::{mem::MaybeUninit, num::NonZeroU64};

/// A generational index that combines generation and index in a single NonZeroU64.
///
/// This allows `Option<IndexSlot>` to be the same size as `IndexSlot` due to niche optimization.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, bytemuck::NoUninit)]
pub struct IndexSlot(NonZeroU64);

impl IndexSlot {
    /// Create a new index slot with the given generation and index.
    pub fn new(generation: u32, idx: u32) -> Self {
        profile_function!();
        Self(unsafe {
            NonZeroU64::new(((generation as u64) << 32) | (idx as u64 + 1)).unwrap_unchecked()
        })
    }

    /// Get the generation component of this index slot.
    pub fn generation(&self) -> u32 {
        (self.0.get() >> 32) as u32
    }

    /// Get the index component of this index slot.
    pub fn index(&self) -> u32 {
        (self.0.get() & u32::MAX as u64) as u32 - 1
    }
}

/// An entry in the sparse set containing generation and data.
pub struct Entry<T> {
    generation: u32,
    data: MaybeUninit<T>,
}

impl<T> Entry<T> {
    /// Create a new entry with generation 0.
    pub const fn new(data: T) -> Self {
        Self {
            generation: 0,
            data: MaybeUninit::new(data),
        }
    }
}

/// A sparse set data structure with generational indices.
///
/// This provides O(1) insertion, removal, and lookup with safe generational indices
/// that detect use-after-free bugs.
///
/// # Example
/// ```
/// use astrelis_core::alloc::sparse_set::{SparseSet, IndexSlot};
///
/// let mut set = SparseSet::<u32>::new();
/// let idx = set.push(42);
/// assert_eq!(*set.get(idx), 42);
///
/// set.remove(idx);
/// // Using idx again would panic due to generation mismatch
/// ```
pub struct SparseSet<T> {
    vec: Vec<Entry<T>>,
    free: Vec<u32>,
}

impl<T> SparseSet<T> {
    /// Create a new empty sparse set.
    pub const fn new() -> Self {
        Self {
            vec: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Create a new sparse set with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vec: Vec::with_capacity(capacity),
            free: Vec::new(),
        }
    }

    /// Insert a value into the sparse set and return its index slot.
    pub fn push(&mut self, data: T) -> IndexSlot {
        profile_function!();
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

    /// Get a reference to the value at the given index slot.
    ///
    /// # Panics
    /// Panics if the index is out of bounds or the generation doesn't match (use-after-free).
    pub fn get(&self, idx: IndexSlot) -> &T {
        profile_function!();
        let entry = self.vec.get(idx.index() as usize).unwrap();
        assert_eq!(
            entry.generation,
            idx.generation(),
            "invalid generation, use after free!"
        );
        unsafe { entry.data.assume_init_ref() }
    }

    /// Get a mutable reference to the value at the given index slot.
    ///
    /// # Panics
    /// Panics if the index is out of bounds or the generation doesn't match (use-after-free).
    pub fn get_mut(&mut self, idx: IndexSlot) -> &mut T {
        profile_function!();
        let entry = self.vec.get_mut(idx.index() as usize).unwrap();
        assert_eq!(
            entry.generation,
            idx.generation(),
            "invalid generation, use after free!"
        );
        unsafe { entry.data.assume_init_mut() }
    }

    /// Try to get a reference to the value at the given index slot.
    ///
    /// Returns `None` if the index is out of bounds or the generation doesn't match.
    pub fn try_get(&self, idx: IndexSlot) -> Option<&T> {
        profile_function!();
        let entry = self.vec.get(idx.index() as usize)?;
        if entry.generation != idx.generation() {
            return None;
        }
        Some(unsafe { entry.data.assume_init_ref() })
    }

    /// Try to get a mutable reference to the value at the given index slot.
    ///
    /// Returns `None` if the index is out of bounds or the generation doesn't match.
    pub fn try_get_mut(&mut self, idx: IndexSlot) -> Option<&mut T> {
        profile_function!();
        let entry = self.vec.get_mut(idx.index() as usize)?;
        if entry.generation != idx.generation() {
            return None;
        }
        Some(unsafe { entry.data.assume_init_mut() })
    }

    /// Remove and return the value at the given index slot.
    ///
    /// # Panics
    /// Panics if the index is out of bounds or the generation doesn't match (use-after-free).
    pub fn remove(&mut self, idx: IndexSlot) -> T {
        profile_function!();
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

    /// Returns the number of elements in the sparse set (including freed slots).
    pub fn capacity(&self) -> usize {
        self.vec.len()
    }

    /// Returns the number of active (non-freed) elements in the sparse set.
    pub fn len(&self) -> usize {
        self.vec.len() - self.free.len()
    }

    /// Returns `true` if the sparse set contains no active elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all elements from the sparse set.
    pub fn clear(&mut self) {
        self.vec.clear();
        self.free.clear();
    }

    /// Iterate over all active elements in the sparse set.
    pub fn iter(&self) -> SparseSetIter<'_, T> {
        SparseSetIter { set: self, idx: 0 }
    }

    /// Iterate mutably over all active elements in the sparse set.
    pub fn iter_mut(&mut self) -> SparseSetIterMut<'_, T> {
        SparseSetIterMut { set: self, idx: 0 }
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over references to active elements in a sparse set.
pub struct SparseSetIter<'a, T> {
    set: &'a SparseSet<T>,
    idx: usize,
}

impl<'a, T> Iterator for SparseSetIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.set.vec.len() && self.set.free.contains(&(self.idx as u32)) {
            self.idx += 1;
        }
        if self.idx >= self.set.vec.len() {
            return None;
        }
        self.idx += 1;
        // SAFETY: The data isn't freed, which means it is initialized
        Some(unsafe { self.set.vec[self.idx - 1].data.assume_init_ref() })
    }
}

/// Iterator over mutable references to active elements in a sparse set.
pub struct SparseSetIterMut<'a, T> {
    set: &'a mut SparseSet<T>,
    idx: usize,
}

impl<'a, T> Iterator for SparseSetIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.set.vec.len() && self.set.free.contains(&(self.idx as u32)) {
            self.idx += 1;
        }
        if self.idx >= self.set.vec.len() {
            return None;
        }
        self.idx += 1;
        // SAFETY: The data isn't freed, which means it is initialized
        // We extend the lifetime here, but the iterator ensures exclusive access
        Some(unsafe { &mut *(self.set.vec[self.idx - 1].data.as_mut_ptr()) })
    }
}

// Ensure niche optimization works
static_assertions::assert_eq_size!(IndexSlot, Option<IndexSlot>);

#[cfg(test)]
mod tests {
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
    #[should_panic(expected = "invalid generation, use after free!")]
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
        set.remove(idx);
        let new_idx = set.push(45);
        assert_eq!(idx.index(), new_idx.index());
        assert_ne!(idx.generation(), new_idx.generation());
    }

    #[test]
    fn test_sparse_set_iter() {
        let mut set = SparseSet::<u8>::new();

        for i in 0..100 {
            set.push(i);
        }
        set.remove(IndexSlot::new(0, 0));
        set.remove(IndexSlot::new(0, 1));
        let iter_collected: Vec<_> = set.iter().collect();
        assert_eq!(iter_collected.len(), 98);
        for i in 2..100 {
            assert_eq!(iter_collected[i - 2], &(i as u8));
        }
    }

    #[test]
    fn test_sparse_set_try_get() {
        let mut set = SparseSet::<u8>::new();
        let idx = set.push(42);
        assert_eq!(set.try_get(idx), Some(&42));

        set.remove(idx);
        assert_eq!(set.try_get(idx), None);
    }

    #[test]
    fn test_sparse_set_len() {
        let mut set = SparseSet::<u8>::new();
        assert_eq!(set.len(), 0);
        assert!(set.is_empty());

        let idx1 = set.push(1);
        let idx2 = set.push(2);
        assert_eq!(set.len(), 2);

        set.remove(idx1);
        assert_eq!(set.len(), 1);

        set.remove(idx2);
        assert_eq!(set.len(), 0);
        assert!(set.is_empty());
    }
}
