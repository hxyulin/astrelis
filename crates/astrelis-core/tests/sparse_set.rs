//! SparseSet generational handle tests.
//!
//! These tests verify correct behavior of the generational SparseSet,
//! including generation counter increments, use-after-free detection,
//! and memory reuse.

use astrelis_core::alloc::sparse_set::{IndexSlot, SparseSet};

#[test]
fn test_push_and_get() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    assert_eq!(*set.get(idx), 42);

    let idx2 = set.push(100);
    assert_eq!(*set.get(idx2), 100);

    // Original value should still be accessible
    assert_eq!(*set.get(idx), 42);
}

#[test]
fn test_push_returns_correct_index() {
    let mut set = SparseSet::<i32>::new();

    let idx0 = set.push(0);
    assert_eq!(idx0.index(), 0);
    assert_eq!(idx0.generation(), 0);

    let idx1 = set.push(1);
    assert_eq!(idx1.index(), 1);
    assert_eq!(idx1.generation(), 0);
}

#[test]
fn test_get_mut() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    *set.get_mut(idx) = 100;

    assert_eq!(*set.get(idx), 100);
}

#[test]
fn test_try_get_valid() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    assert_eq!(set.try_get(idx), Some(&42));
}

#[test]
fn test_try_get_invalid_returns_none() {
    let set = SparseSet::<i32>::new();

    // Invalid index should return None
    let invalid = IndexSlot::new(0, 999);
    assert_eq!(set.try_get(invalid), None);
}

#[test]
fn test_remove() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    let value = set.remove(idx);

    assert_eq!(value, 42);
}

#[test]
#[should_panic(expected = "invalid generation")]
fn test_use_after_free_panics() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    set.remove(idx);

    // This should panic because the generation doesn't match
    let _ = set.get(idx);
}

#[test]
fn test_try_get_after_remove_returns_none() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    set.remove(idx);

    // try_get should return None instead of panicking
    assert_eq!(set.try_get(idx), None);
}

#[test]
fn test_generation_increment() {
    let mut set = SparseSet::new();

    // Push, remove, push again at same index
    let idx1 = set.push(1);
    assert_eq!(idx1.generation(), 0);

    set.remove(idx1);

    let idx2 = set.push(2);
    assert_eq!(idx2.index(), idx1.index()); // Same slot
    assert_eq!(idx2.generation(), 1); // Incremented generation

    // Old handle should be invalid
    assert_eq!(set.try_get(idx1), None);

    // New handle should work
    assert_eq!(*set.get(idx2), 2);
}

#[test]
fn test_multiple_generation_increments() {
    let mut set = SparseSet::new();

    let idx0 = set.push(0);
    assert_eq!(idx0.generation(), 0);

    set.remove(idx0);
    let idx1 = set.push(1);
    assert_eq!(idx1.generation(), 1);

    set.remove(idx1);
    let idx2 = set.push(2);
    assert_eq!(idx2.generation(), 2);

    set.remove(idx2);
    let idx3 = set.push(3);
    assert_eq!(idx3.generation(), 3);

    // Only the latest generation should work
    assert_eq!(set.try_get(idx0), None);
    assert_eq!(set.try_get(idx1), None);
    assert_eq!(set.try_get(idx2), None);
    assert_eq!(*set.get(idx3), 3);
}

#[test]
fn test_slot_reuse() {
    let mut set = SparseSet::new();

    // Create and remove multiple values
    let idx1 = set.push(1);
    let idx2 = set.push(2);
    let idx3 = set.push(3);

    // Indices should be 0, 1, 2
    assert_eq!(idx1.index(), 0);
    assert_eq!(idx2.index(), 1);
    assert_eq!(idx3.index(), 2);

    // Remove middle element
    set.remove(idx2);

    // Next push should reuse the freed slot
    let idx4 = set.push(4);
    assert_eq!(idx4.index(), 1); // Reused slot 1
    assert_eq!(idx4.generation(), 1); // But with incremented generation
}

#[test]
fn test_with_capacity() {
    let set = SparseSet::<i32>::with_capacity(100);
    assert_eq!(set.len(), 0);
}

#[test]
fn test_len_and_capacity() {
    let mut set = SparseSet::new();

    assert_eq!(set.len(), 0);

    set.push(1);
    assert_eq!(set.len(), 1);

    set.push(2);
    assert_eq!(set.len(), 2);

    let idx = set.push(3);
    assert_eq!(set.len(), 3);

    set.remove(idx);
    assert_eq!(set.len(), 2); // Length decreases after removal
}

#[test]
fn test_clear() {
    let mut set = SparseSet::new();

    set.push(1);
    set.push(2);
    set.push(3);

    set.clear();
    assert_eq!(set.len(), 0);
}

#[test]
fn test_iteration() {
    let mut set = SparseSet::new();

    set.push(10);
    set.push(20);
    set.push(30);

    let values: Vec<_> = set.iter().copied().collect();
    assert_eq!(values, vec![10, 20, 30]);
}

#[test]
fn test_iteration_with_removed_elements() {
    let mut set = SparseSet::new();

    set.push(10);
    let idx = set.push(20);
    set.push(30);

    set.remove(idx); // Remove middle element

    let values: Vec<_> = set.iter().copied().collect();
    assert_eq!(values, vec![10, 30]); // Should skip removed element
}

#[test]
fn test_iter_mut() {
    let mut set = SparseSet::new();

    set.push(10);
    set.push(20);
    set.push(30);

    // Double all values
    for val in set.iter_mut() {
        *val *= 2;
    }

    let values: Vec<_> = set.iter().copied().collect();
    assert_eq!(values, vec![20, 40, 60]);
}

#[test]
fn test_complex_interleaved_operations() {
    let mut set = SparseSet::new();

    let idx1 = set.push(1);
    let idx2 = set.push(2);
    let idx3 = set.push(3);

    assert_eq!(*set.get(idx1), 1);
    assert_eq!(*set.get(idx2), 2);
    assert_eq!(*set.get(idx3), 3);

    // Remove idx2
    set.remove(idx2);

    // Add new element (should reuse idx2's slot)
    let idx4 = set.push(4);
    assert_eq!(idx4.index(), idx2.index());

    // Old idx2 should be invalid
    assert_eq!(set.try_get(idx2), None);

    // New idx4 should work
    assert_eq!(*set.get(idx4), 4);

    // idx1 and idx3 should still work
    assert_eq!(*set.get(idx1), 1);
    assert_eq!(*set.get(idx3), 3);
}

#[test]
fn test_index_slot_equality() {
    let idx1 = IndexSlot::new(0, 5);
    let idx2 = IndexSlot::new(0, 5);
    let idx3 = IndexSlot::new(1, 5); // Different generation
    let idx4 = IndexSlot::new(0, 6); // Different index

    assert_eq!(idx1, idx2);
    assert_ne!(idx1, idx3);
    assert_ne!(idx1, idx4);
}

#[test]
fn test_different_types() {
    let mut set_i32 = SparseSet::<i32>::new();
    let mut set_string = SparseSet::<String>::new();

    let idx_i32 = set_i32.push(42);
    let idx_string = set_string.push("hello".to_string());

    assert_eq!(*set_i32.get(idx_i32), 42);
    assert_eq!(*set_string.get(idx_string), "hello");
}

#[test]
fn test_stress_many_insertions() {
    let mut set = SparseSet::new();
    let mut indices = Vec::new();

    // Insert many elements
    for i in 0..1000 {
        indices.push(set.push(i));
    }

    // Verify all are accessible
    for (i, &idx) in indices.iter().enumerate() {
        assert_eq!(*set.get(idx), i);
    }

    assert_eq!(set.len(), 1000);
}

#[test]
fn test_stress_many_removals_and_reuses() {
    let mut set = SparseSet::new();

    // Insert 100 elements
    let mut indices = Vec::new();
    for i in 0..100 {
        indices.push(set.push(i));
    }

    // Remove every other element
    for i in (0..100).step_by(2) {
        set.remove(indices[i]);
    }

    assert_eq!(set.len(), 50);

    // Add 50 more elements (should reuse slots)
    for i in 100..150 {
        set.push(i);
    }

    assert_eq!(set.len(), 100);
}

#[test]
#[should_panic]
fn test_remove_already_removed_panics() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    set.remove(idx);

    // Removing again should panic
    set.remove(idx);
}

#[test]
#[should_panic]
fn test_get_mut_after_remove_panics() {
    let mut set = SparseSet::new();

    let idx = set.push(42);
    set.remove(idx);

    // get_mut should panic after remove
    let _ = set.get_mut(idx);
}
