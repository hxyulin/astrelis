use crate::profiling::profile_function;
use std::{mem::MaybeUninit, num::NonZeroU64};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexSlot(NonZeroU64);

impl IndexSlot {
    pub fn new(generation: u32, idx: u32) -> Self {
        profile_function!();
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

    pub fn iter(&self) -> SparseSetIter<'_, T> {
        SparseSetIter { set: self, idx: 0 }
    }
}

pub struct SparseSetIter<'a, T> {
    set: &'a SparseSet<T>,
    idx: usize,
}

impl<'a, T> Iterator for SparseSetIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.set.free.contains(&(self.idx as u32)) {
            self.idx += 1;
        }
        if self.idx >= self.set.vec.len() {
            return None;
        }
        self.idx += 1;
        // SAFETY: The data isn't freeed, which means it is initialized
        Some(unsafe { self.set.vec[self.idx - 1].data.assume_init_ref() })
    }
}

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
}
