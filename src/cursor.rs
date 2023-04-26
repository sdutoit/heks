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
mod tests {
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

    // TODO: grow, shrink, skip_right, skip_left

    #[test]
    fn test_clamp() {
        let mut c = Cursor::new(0, 1);
        c.clamp(0u64..u64::MAX);
        assert_eq!(c, Cursor::new(0, 1));

        let mut c = Cursor::new(0, u64::MAX);
        c.clamp(123u64..456u64);
        assert_eq!(c, Cursor::new(123, 456));

        let mut c = Cursor::new(u64::MAX - 1, u64::MAX);
        c.clamp(123u64..456u64);
        assert_eq!(c, Cursor::new(455, 456));

        // TODO: lots more cases!
    }
}
