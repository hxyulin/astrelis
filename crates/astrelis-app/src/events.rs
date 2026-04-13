//! Double-buffered typed event queues.
//!
//! [`Events<T>`] provides a per-type event channel stored as a resource.
//! Events are readable for up to two frames (current + previous), which
//! prevents system ordering from affecting event visibility.

use std::slice;

/// A double-buffered event queue for a single event type.
///
/// Each frame the framework calls [`swap`](Events::swap) to rotate
/// buffers: the current buffer becomes the previous buffer (still
/// readable), and the write buffer is cleared.
///
/// Systems call [`send`](Events::send) to enqueue events and
/// [`read`](Events::read) to iterate over events from both the
/// current and previous frames.
pub struct Events<T> {
    /// Events sent during the current frame.
    current: Vec<T>,
    /// Events from the previous frame (still readable this frame).
    previous: Vec<T>,
}

impl<T> Events<T> {
    /// Creates an empty event queue.
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }

    /// Sends an event into the current frame's buffer.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Returns an iterator over all readable events (previous + current).
    pub fn read(&self) -> EventReader<'_, T> {
        EventReader {
            previous: self.previous.iter(),
            current: self.current.iter(),
        }
    }

    /// Swaps the buffers: current becomes previous, previous is cleared.
    ///
    /// Called by the framework at the start of each frame's `PreUpdate` phase.
    pub(crate) fn swap(&mut self) {
        std::mem::swap(&mut self.current, &mut self.previous);
        self.current.clear();
    }

    /// Returns the number of events in the current frame's buffer.
    pub fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }

    /// Returns `true` if there are no readable events.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.previous.is_empty()
    }
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over events from both the previous and current frame.
pub struct EventReader<'a, T> {
    previous: slice::Iter<'a, T>,
    current: slice::Iter<'a, T>,
}

impl<'a, T> Iterator for EventReader<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.previous.next().or_else(|| self.current.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.previous.len() + self.current.len();
        (len, Some(len))
    }
}

impl<T> ExactSizeIterator for EventReader<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_read() {
        let mut events = Events::new();
        events.send(1);
        events.send(2);
        let collected: Vec<_> = events.read().copied().collect();
        assert_eq!(collected, vec![1, 2]);
    }

    #[test]
    fn events_readable_across_swap() {
        let mut events = Events::new();
        events.send(1);
        events.swap();
        events.send(2);
        // Both frames' events should be readable.
        let collected: Vec<_> = events.read().copied().collect();
        assert_eq!(collected, vec![1, 2]);
    }

    #[test]
    fn events_cleared_after_two_swaps() {
        let mut events = Events::new();
        events.send(1);
        events.swap();
        events.swap();
        assert!(events.is_empty());
    }
}
