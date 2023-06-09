use std::cmp::min;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub(super) start: u64,
    pub(super) end: u64, // one past the last character
}

impl Cursor {
    pub fn new(start: u64, end: u64) -> Self {
        Cursor { start, end }
    }

    pub fn start(&self) -> u64 {
        self.start
    }
    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn contains(&self, location: u64) -> bool {
        self.start <= location && location < self.end
    }

    pub fn increment(&mut self, delta: u64) {
        let width = self.end - self.start;
        self.end = self.end.saturating_add(delta);
        self.start = self.end - width;
    }

    pub fn decrement(&mut self, delta: u64) {
        let width = self.end - self.start;
        self.start = self.start.saturating_sub(delta);
        self.end = self.start + width;
    }

    pub fn grow(&mut self) {
        self.end = self.end.saturating_add(1);
    }

    pub fn shrink(&mut self) {
        if self.end > self.start + 1 {
            self.end -= 1;
        }
    }

    pub fn skip_right(&mut self) {
        assert!(self.start <= self.end);

        let width = self.end - self.start;

        self.end = self.end.saturating_add(width);
        self.start = self.end - width;
    }

    pub fn skip_left(&mut self) {
        assert!(self.start <= self.end);

        let width = self.end - self.start;

        self.start = self.start.saturating_sub(width);
        self.end = self.start + width;
    }

    // Ensures that self is within `range`. If the range is smaller than the
    // current size of the cursor, sets the cursor to the range. Otherwise,
    // maintains the size of the cursor and moves it the smallest amount
    // necessary to fit within the range.
    pub fn clamp(&mut self, range: Range<u64>) {
        let width = min(self.end - self.start, range.end - range.start);
        if self.end > range.end {
            self.end = range.end;
            self.start = self.end - width;
        } else if self.start < range.start {
            self.start = range.start;
            self.end = self.start + width;
        };
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    fn test_new() {
        let c = Cursor::new(123, 456);
        assert_eq!(c.start, 123);
        assert_eq!(c.end, 456);
    }

    #[test]
    fn test_from() {
        let c = Cursor::new(123, 456);
        assert_eq!(c.start, 123);
        assert_eq!(c.end, 456);
        let c = Cursor::new(456, 789);
        assert_eq!(c.start, 456);
        assert_eq!(c.end, 789);
    }

    #[test]
    fn test_accessors() {
        let c = Cursor {
            start: 100,
            end: 200,
        };
        assert_eq!(c.start(), 100);
        assert_eq!(c.end(), 200);
    }

    #[test]
    fn test_contains() {
        let c = Cursor {
            start: 100,
            end: 200,
        };
        assert!(!c.contains(0));
        assert!(!c.contains(99));
        assert!(c.contains(100));
        assert!(c.contains(101));
        assert!(c.contains(150));
        assert!(c.contains(199));
        assert!(!c.contains(200));
        assert!(!c.contains(201));
        assert!(!c.contains(u64::MAX));
    }

    #[test]
    fn test_increment() {
        let mut c = Cursor { start: 0, end: 0 };
        c.increment(1);
        assert_eq!(c, Cursor::new(1, 1));
        c.increment(5);
        assert_eq!(c, Cursor::new(6, 6));
        c.increment(u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX, u64::MAX));

        let mut c = Cursor { start: 0, end: 1 };
        c.increment(1);
        assert_eq!(c, Cursor::new(1, 2));
        c.increment(10);
        assert_eq!(c, Cursor::new(11, 12));
        c.increment(u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX - 1, u64::MAX));

        let mut c = Cursor {
            start: 0,
            end: 9999,
        };
        c.increment(1);
        assert_eq!(c, Cursor::new(1, 10000));
        c.increment(10);
        assert_eq!(c, Cursor::new(11, 10010));
        c.increment(u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX - 9999, u64::MAX));
    }

    #[test]
    fn test_decrement() {
        let mut c = Cursor::new(1000, 1000);
        c.decrement(1);
        assert_eq!(c, Cursor::new(999, 999));
        c.decrement(9);
        assert_eq!(c, Cursor::new(990, 990));
        c.decrement(u64::MAX);
        assert_eq!(c, Cursor::new(0, 0));

        let mut c = Cursor::new(1000, 1001);
        c.decrement(1);
        assert_eq!(c, Cursor::new(999, 1000));
        c.decrement(9);
        assert_eq!(c, Cursor::new(990, 991));
        c.decrement(u64::MAX);
        assert_eq!(c, Cursor::new(0, 1));

        let mut c = Cursor {
            start: 10000,
            end: 20000,
        };
        c.decrement(1);
        assert_eq!(c, Cursor::new(9999, 19999));
        c.decrement(9);
        assert_eq!(c, Cursor::new(9990, 19990));
        c.decrement(u64::MAX);
        assert_eq!(c, Cursor::new(0, 10000));
    }

    #[test]
    fn test_grow() {
        let mut c = Cursor::new(123, 456);
        c.grow();
        assert_eq!(c, Cursor::new(123, 457));
        c.grow();
        assert_eq!(c, Cursor::new(123, 458));

        let mut c = Cursor::new(u64::MAX - 8, u64::MAX - 2);
        c.grow();
        assert_eq!(c, Cursor::new(u64::MAX - 8, u64::MAX - 1));
        c.grow();
        assert_eq!(c, Cursor::new(u64::MAX - 8, u64::MAX));
        c.grow();
        assert_eq!(c, Cursor::new(u64::MAX - 8, u64::MAX));
    }

    #[test]
    fn test_shrink() {
        let mut c = Cursor::new(123, 456);
        c.shrink();
        assert_eq!(c, Cursor::new(123, 455));
        c.shrink();
        assert_eq!(c, Cursor::new(123, 454));

        let mut c = Cursor::new(1000, 1002);
        c.shrink();
        assert_eq!(c, Cursor::new(1000, 1001));
        c.shrink();
        assert_eq!(c, Cursor::new(1000, 1001));

        let mut c = Cursor::new(1000, 1000);
        c.shrink();
        assert_eq!(c, Cursor::new(1000, 1000));

        let mut c = Cursor::new(0, 2);
        c.shrink();
        assert_eq!(c, Cursor::new(0, 1));
        c.shrink();
        assert_eq!(c, Cursor::new(0, 1));
    }

    #[test]
    fn test_skip_right() {
        let mut c = Cursor::new(7, 10);
        c.skip_right();
        assert_eq!(c, Cursor::new(10, 13));
        c.skip_right();
        assert_eq!(c, Cursor::new(13, 16));

        let mut c = Cursor::new(100, 100);
        c.skip_right();
        assert_eq!(c, Cursor::new(100, 100));

        let mut c = Cursor::new(u64::MAX - 5, u64::MAX - 3);
        c.skip_right();
        assert_eq!(c, Cursor::new(u64::MAX - 3, u64::MAX - 1));
        c.skip_right();
        assert_eq!(c, Cursor::new(u64::MAX - 2, u64::MAX));
        c.skip_right();
        assert_eq!(c, Cursor::new(u64::MAX - 2, u64::MAX));
    }

    #[test]
    fn test_skip_left() {
        let mut c = Cursor::new(7, 10);
        c.skip_left();
        assert_eq!(c, Cursor::new(4, 7));
        c.skip_left();
        assert_eq!(c, Cursor::new(1, 4));
        c.skip_left();
        assert_eq!(c, Cursor::new(0, 3));
        c.skip_left();
        assert_eq!(c, Cursor::new(0, 3));

        let mut c = Cursor::new(100, 100);
        c.skip_left();
        assert_eq!(c, Cursor::new(100, 100));
    }

    #[test]
    fn test_clamp() {
        let mut c = Cursor::new(0, 1);
        c.clamp(0u64..u64::MAX);
        assert_eq!(c, Cursor::new(0, 1));
        c.clamp(1u64..u64::MAX);
        assert_eq!(c, Cursor::new(1, 2));
        c.clamp(10u64..u64::MAX);
        assert_eq!(c, Cursor::new(10, 11));
        c.clamp(u64::MAX - 1..u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX - 1, u64::MAX));
        c.clamp(0u64..100u64);
        assert_eq!(c, Cursor::new(99, 100));

        // Singleton cases -- make sure we preserve the location even if nothing
        // is selected.
        let mut c = Cursor::new(0, 1);
        c.clamp(u64::MAX..u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX, u64::MAX));
        c.clamp(100u64..100u64);
        assert_eq!(c, Cursor::new(100, 100));
        c.clamp(0u64..0u64);
        assert_eq!(c, Cursor::new(0, 0));

        // Wider than one character
        let mut c = Cursor::new(100, 200);
        c.clamp(100u64..200u64);
        assert_eq!(c, Cursor::new(100, 200));
        c.clamp(50u64..200u64);
        assert_eq!(c, Cursor::new(100, 200));
        c.clamp(100u64..250u64);
        assert_eq!(c, Cursor::new(100, 200));

        // Values near/at u64::MAX
        let mut c = Cursor::new(100, 200);
        c.clamp(u64::MAX - 150..u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX - 150, u64::MAX - 50));
        c.clamp(u64::MAX - 100..u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX - 100, u64::MAX));
        c.clamp(u64::MAX - 50..u64::MAX);
        assert_eq!(c, Cursor::new(u64::MAX - 50, u64::MAX));
        c.clamp(u64::MAX - 50..u64::MAX - 10);
        assert_eq!(c, Cursor::new(u64::MAX - 50, u64::MAX - 10));
        c.clamp(u64::MAX - 60..u64::MAX - 20);
        assert_eq!(c, Cursor::new(u64::MAX - 60, u64::MAX - 20));
        c.clamp(0u64..100u64);
        assert_eq!(c, Cursor::new(60, 100));
        c.clamp(10u64..20u64);
        assert_eq!(c, Cursor::new(10, 20));

        // Typical case: cursor at +inf, clamp back to what's on the screen.
        let mut c = Cursor::new(u64::MAX - 4, u64::MAX);
        c.clamp(128u64..256u64);
        assert_eq!(c, Cursor::new(252u64, 256u64));
    }
}

#[derive(Debug)]
pub struct CursorStack {
    cursors: Vec<Cursor>,
    undo_depth: usize,
}

impl CursorStack {
    pub fn new(cursor: Cursor) -> Self {
        CursorStack {
            cursors: vec![cursor],
            undo_depth: 0,
        }
    }

    pub fn push(&mut self, cursor: Cursor) {
        self.cursors.push(cursor);
    }

    fn top_index(&self) -> usize {
        // We should never allow cursors to go empty.
        assert!(!self.cursors.is_empty());
        let index = self.cursors.len() - 1;
        // We should never allow the undo depth to exceed the stack size.
        assert!(self.undo_depth <= index);
        index - self.undo_depth
    }

    pub fn top(&self) -> Cursor {
        self.cursors[self.top_index()].clone()
    }

    pub fn top_mut(&mut self) -> &mut Cursor {
        let index = self.top_index();
        self.cursors.get_mut(index).unwrap()
    }

    pub fn undo(&mut self) {
        if self.undo_depth < self.cursors.len() - 1 {
            self.undo_depth += 1;
        }
    }

    pub fn redo(&mut self) {
        self.undo_depth = self.undo_depth.saturating_sub(1);
    }

    pub fn set(&mut self, cursor: Cursor) {
        self.cursors.truncate(self.top_index());
        self.undo_depth = 0;
        self.cursors.push(cursor);
    }
}

#[cfg(test)]
mod cursor_stack_tests {
    use super::*;

    #[test]
    fn test_new() {
        let stack = CursorStack::new(Cursor::new(0, 1));
        assert_eq!(stack.cursors, vec![Cursor::new(0, 1)]);
    }

    #[test]
    fn test_push_top_undo_redo() {
        let mut stack = CursorStack::new(Cursor::new(0, 1));
        assert_eq!(stack.top(), Cursor::new(0, 1));

        stack.push(Cursor::new(1, 2));
        assert_eq!(stack.top(), Cursor::new(1, 2));
        stack.push(Cursor::new(3, 4));
        assert_eq!(stack.top(), Cursor::new(3, 4));

        stack.undo();
        assert_eq!(stack.top(), Cursor::new(1, 2));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(0, 1));
        stack.undo();
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(0, 1));
        stack.redo();
        assert_eq!(stack.top(), Cursor::new(1, 2));
        stack.redo();
        assert_eq!(stack.top(), Cursor::new(3, 4));
        stack.redo();
        stack.redo();
        stack.redo();
        assert_eq!(stack.top(), Cursor::new(3, 4));
    }

    #[test]
    fn test_set() {
        let mut stack = CursorStack::new(Cursor::new(0, 1));
        assert_eq!(stack.top(), Cursor::new(0, 1));
        stack.set(Cursor::new(1, 2));
        assert_eq!(stack.top(), Cursor::new(1, 2));
        stack.set(Cursor::new(2, 3));
        assert_eq!(stack.top(), Cursor::new(2, 3));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(2, 3));

        stack.push(Cursor::new(3, 4));
        stack.push(Cursor::new(5, 6));
        stack.set(Cursor::new(5, 16));
        assert_eq!(stack.top(), Cursor::new(5, 16));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(3, 4));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(2, 3));

        stack.set(Cursor::new(2, 12));
        assert_eq!(stack.top(), Cursor::new(2, 12));
        stack.redo();
        assert_eq!(stack.top(), Cursor::new(2, 12));
        assert_eq!(stack.cursors.len(), 1);
    }

    #[test]
    fn test_top_mut() {
        let mut stack = CursorStack::new(Cursor::new(0, 1));
        assert_eq!(stack.top(), Cursor::new(0, 1));
        stack.top_mut().grow();
        assert_eq!(stack.top(), Cursor::new(0, 2));
        stack.top_mut().grow();
        assert_eq!(stack.top(), Cursor::new(0, 3));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(0, 3));

        stack.push(Cursor::new(1, 2));
        stack.push(Cursor::new(2, 3));
        stack.top_mut().grow();
        assert_eq!(stack.top(), Cursor::new(2, 4));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(1, 2));
        stack.undo();
        assert_eq!(stack.top(), Cursor::new(0, 3));

        stack.top_mut().grow();
        assert_eq!(stack.top(), Cursor::new(0, 4));
        stack.redo();
        assert_eq!(stack.top(), Cursor::new(1, 2));
        stack.redo();
        assert_eq!(stack.top(), Cursor::new(2, 4));
    }
}
