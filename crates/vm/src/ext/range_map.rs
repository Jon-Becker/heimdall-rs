use hashbrown::HashMap;
use std::ops::Range;

use crate::core::opcodes::WrappedOpcode;

/// A map that associates memory ranges with their corresponding opcodes.
/// This is used to track which opcodes are responsible for writing to specific memory locations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeMap(pub HashMap<Range<usize>, WrappedOpcode>);

impl Default for RangeMap {
    fn default() -> Self {
        Self::new()
    }
}

impl RangeMap {
    /// Creates a new empty RangeMap.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Given an offset into memory, returns the associated opcode if it exists
    pub fn get_by_offset(&self, offset: usize) -> Option<WrappedOpcode> {
        self.0.get(self.find_range(offset).expect("RangeMap::have_range is broken")).cloned()
    }

    /// Given a range, returns associated opcodes if they exist
    pub fn get_by_range(&self, offset: usize, size: usize) -> Vec<WrappedOpcode> {
        let mut memory_range: Vec<WrappedOpcode> = Vec::new();
        let mut offset: usize = offset;
        let mut size: usize = size;

        // get the memory range
        while size > 0 {
            if let Some(op) = self.get_by_offset(offset) {
                memory_range.push(op.clone());
            }
            offset += 32;
            size = size.saturating_sub(32);
        }

        memory_range
    }

    /// Associates the provided opcode with the range of memory modified by writing a `size`-byte
    /// value to `offset`.
    ///
    /// This range is exactly `[offset, offset + size - 1]`. This function ensures that any existing
    /// ranges that our new range would collide with are dealt with accordingly, that is:
    ///
    ///  - deleted, if our range completely overwrites it,
    ///  - split, if our range overwrites a subset that partitions it,
    ///  - shortened, if our range overwrites such that only one "end" of it is overwritten
    pub fn write(&mut self, offset: usize, size: usize, opcode: WrappedOpcode) {
        let range: Range<usize> =
            Range { start: offset, end: offset.saturating_add(size).saturating_sub(1) };
        let incumbents: Vec<Range<usize>> = self.affected_ranges(&range);

        if incumbents.is_empty() {
            self.0.insert(range, opcode);
        } else {
            incumbents.iter().for_each(|incumbent| {
                if incumbent.start <= range.end && incumbent.end >= range.start {
                    // Case 1: overlapping
                    if range.start <= incumbent.start && range.end >= incumbent.end {
                        // newInterval completely covers incumbent
                        // remove the existing interval
                        self.0.remove(incumbent);
                    }
                    // Case 2: shortening
                    else if range.start <= incumbent.start && range.end < incumbent.end {
                        let remainder: Range<usize> =
                            Range { start: range.end + 1, end: incumbent.end };
                        let old_opcode: WrappedOpcode = self.0.get(incumbent).cloned().unwrap();
                        self.0.remove(incumbent);
                        self.0.insert(remainder, old_opcode);
                    } else if range.start > incumbent.start && range.end >= incumbent.end {
                        let remainder: Range<usize> =
                            Range { start: incumbent.start, end: range.start - 1 };
                        let old_opcode: WrappedOpcode = self.0.get(incumbent).cloned().unwrap();
                        self.0.remove(incumbent);
                        self.0.insert(remainder, old_opcode);
                    }
                    // Case 3: splitting
                    else if range.start > incumbent.start && range.end < incumbent.end {
                        let left: Range<usize> =
                            Range { start: incumbent.start, end: range.start.saturating_sub(1) };
                        let right: Range<usize> =
                            Range { start: range.end + 1, end: incumbent.end };
                        let old_opcode: WrappedOpcode = self.0.get(incumbent).cloned().unwrap();
                        self.0.remove(incumbent);
                        self.0.insert(left, old_opcode.clone());
                        self.0.insert(right, old_opcode);
                    } else {
                        panic!("range_map::write: impossible case");
                    }
                }
                self.0.insert(range.clone(), opcode.clone());
            });
        }
    }

    fn find_range(&self, offset: usize) -> Option<&Range<usize>> {
        self.0.keys().find(|range| range.contains(&offset))
    }

    fn affected_ranges(&self, range: &Range<usize>) -> Vec<Range<usize>> {
        self.0.keys().filter(|incumbent| Self::range_collides(range, incumbent)).cloned().collect()
    }

    fn range_collides(incoming: &Range<usize>, incumbent: &Range<usize>) -> bool {
        !(incoming.start <= incumbent.start &&
            incoming.end < incumbent.end &&
            incoming.start < incumbent.start ||
            incoming.start > incumbent.start &&
                incoming.end >= incumbent.end &&
                incoming.end > incumbent.end)
    }
}

#[cfg(test)]
mod tests {
    use hashbrown::HashMap;
    use std::ops::Range;

    use crate::{core::opcodes::WrappedOpcode, ext::range_map::RangeMap};

    #[test]
    fn test_one_incumbent_and_needs_deletion() {
        /* the values of the mapping are irrelevant for the purposes of this test, so we
         * construct an arbitrary one and reuse it everywhere for simplicity */
        let some_op: WrappedOpcode = WrappedOpcode::default();
        let initial_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((8, 16), some_op.clone()), ((32, 64), some_op.clone())];

        let mut actual_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            initial_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        let offset: usize = 7;
        let size: usize = 11;
        actual_byte_tracker.write(offset, size, some_op.clone());

        let expected_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((7, 17), some_op.clone()), ((32, 64), some_op)];
        let expected_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            expected_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        assert_eq!(actual_byte_tracker, expected_byte_tracker);
    }

    #[test]
    fn test_one_incumbent_and_needs_splitting() {
        /* the values of the mapping are irrelevant for the purposes of this test, so we
         * construct an arbitrary one and reuse it everywhere for simplicity */
        let some_op: WrappedOpcode = WrappedOpcode::default();
        let initial_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((7, 18), some_op.clone()), ((32, 64), some_op.clone())];

        let mut actual_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            initial_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        let offset: usize = 8;
        let size: usize = 8;
        actual_byte_tracker.write(offset, size, some_op.clone());

        let expected_pairs: Vec<((usize, usize), WrappedOpcode)> = vec![
            ((7, 7), some_op.clone()),
            ((8, 15), some_op.clone()),
            ((16, 18), some_op.clone()),
            ((32, 64), some_op),
        ];
        let expected_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            expected_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        assert_eq!(actual_byte_tracker, expected_byte_tracker);
    }

    #[test]
    fn test_one_incumbent_and_needs_right_shortening() {
        /* the values of the mapping are irrelevant for the purposes of this test, so we
         * construct an arbitrary one and reuse it everywhere for simplicity */
        let some_op: WrappedOpcode = WrappedOpcode::default();
        let initial_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((7, 18), some_op.clone()), ((32, 64), some_op.clone())];

        let mut actual_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            initial_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        let offset: usize = 10;
        let size: usize = 14;
        actual_byte_tracker.write(offset, size, some_op.clone());

        let expected_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((7, 9), some_op.clone()), ((10, 23), some_op.clone()), ((32, 64), some_op)];
        let expected_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            expected_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        assert_eq!(actual_byte_tracker, expected_byte_tracker);
    }

    #[test]
    fn test_one_incumbent_and_needs_left_shortening() {
        /* the values of the mapping are irrelevant for the purposes of this test, so we
         * construct an arbitrary one and reuse it everywhere for simplicity */
        let some_op: WrappedOpcode = WrappedOpcode::default();
        let initial_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((7, 18), some_op.clone()), ((32, 64), some_op.clone())];

        let mut actual_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            initial_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        let offset: usize = 2;
        let size: usize = 8;
        actual_byte_tracker.write(offset, size, some_op.clone());

        let expected_pairs: Vec<((usize, usize), WrappedOpcode)> =
            vec![((2, 9), some_op.clone()), ((10, 18), some_op.clone()), ((32, 64), some_op)];
        let expected_byte_tracker: RangeMap = RangeMap(HashMap::from_iter(
            expected_pairs.iter().cloned().map(|((a, b), v)| (Range { start: a, end: b }, v)),
        ));

        assert_eq!(actual_byte_tracker, expected_byte_tracker);
    }

    #[test]
    fn test_range_collides() {
        let range: Range<usize> = Range { start: 0, end: 10 };
        let incumbent: Range<usize> = Range { start: 5, end: 15 };

        assert!(RangeMap::range_collides(&range, &incumbent));
    }

    #[test]
    fn test_range_does_not_collide() {
        let range: Range<usize> = Range { start: 0, end: 10 };
        let incumbent: Range<usize> = Range { start: 11, end: 15 };

        assert!(!RangeMap::range_collides(&range, &incumbent));
    }
}
