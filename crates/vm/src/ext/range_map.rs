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
        self.find_range(offset).and_then(|range| self.0.get(range)).cloned()
    }

    /// Given a range, returns associated opcodes if they exist
    pub fn get_by_range(&self, offset: usize, size: usize) -> Vec<WrappedOpcode> {
        let mut memory_range: Vec<WrappedOpcode> = Vec::new();
        let mut offset: usize = offset;
        let mut size: usize = size;

        while size > 0 {
            if let Some(op) = self.get_by_offset(offset) {
                memory_range.push(op.clone());
            }
            size = size.saturating_sub(32);
            if size > 0 {
                let Some(next) = offset.checked_add(32) else { break };
                offset = next;
            }
        }

        memory_range
    }

    /// Associates the provided opcode with the range of memory modified by writing a `size`-byte
    /// value to `offset`.
    ///
    /// Ranges use the half-open convention `[offset, offset + size)`.
    /// Existing entries are shortened or split where the new write overlaps them.
    pub fn write(&mut self, offset: usize, size: usize, opcode: WrappedOpcode) {
        let range = offset..offset.saturating_add(size);
        if range.is_empty() {
            return;
        }

        for incumbent in self.affected_ranges(&range) {
            let Some(old_opcode) = self.0.remove(&incumbent) else { continue };
            if incumbent.start < range.start {
                self.0.insert(incumbent.start..range.start, old_opcode.clone());
            }
            if incumbent.end > range.end {
                self.0.insert(range.end..incumbent.end, old_opcode);
            }
        }
        self.0.insert(range, opcode);
    }

    fn find_range(&self, offset: usize) -> Option<&Range<usize>> {
        self.0.keys().find(|range| range.contains(&offset))
    }

    fn affected_ranges(&self, range: &Range<usize>) -> Vec<Range<usize>> {
        self.0.keys().filter(|incumbent| Self::range_collides(range, incumbent)).cloned().collect()
    }

    fn range_collides(incoming: &Range<usize>, incumbent: &Range<usize>) -> bool {
        incoming.start < incumbent.end && incumbent.start < incoming.end
    }
}

#[cfg(test)]
mod tests {
    use crate::{core::opcodes::WrappedOpcode, ext::range_map::RangeMap};

    fn op(opcode: u8) -> WrappedOpcode {
        WrappedOpcode::new(opcode, vec![])
    }

    fn origins(map: &RangeMap, start: usize, end: usize) -> Vec<Option<u8>> {
        (start..end).map(|offset| map.get_by_offset(offset).map(|op| op.opcode)).collect()
    }

    #[test]
    fn writes_track_first_and_last_byte_and_leave_gaps_unmapped() {
        let mut map = RangeMap::new();
        map.write(2, 3, op(1));
        map.write(7, 1, op(2));
        assert_eq!(
            origins(&map, 0, 9),
            vec![None, None, Some(1), Some(1), Some(1), None, None, Some(2), None]
        );
        assert_eq!(map.get_by_offset(usize::MAX), None);
    }

    #[test]
    fn overlapping_writes_replace_only_their_bytes() {
        for (offset, size, expected) in [
            (0, 10, vec![Some(2); 10]),
            (
                0,
                5,
                vec![
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(1),
                    Some(1),
                    Some(1),
                    None,
                    None,
                ],
            ),
            (
                5,
                5,
                vec![
                    None,
                    None,
                    Some(1),
                    Some(1),
                    Some(1),
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(2),
                    Some(2),
                ],
            ),
            (
                4,
                2,
                vec![None, None, Some(1), Some(1), Some(2), Some(2), Some(1), Some(1), None, None],
            ),
        ] {
            let mut map = RangeMap::new();
            map.write(2, 6, op(1));
            map.write(offset, size, op(2));
            assert_eq!(origins(&map, 0, 10), expected);
        }
    }

    #[test]
    fn adjacent_and_empty_writes_preserve_existing_origins() {
        let mut map = RangeMap::new();
        map.write(0, 2, op(1));
        map.write(2, 2, op(2));
        map.write(1, 0, op(3));
        assert_eq!(origins(&map, 0, 5), vec![Some(1), Some(1), Some(2), Some(2), None]);
    }

    #[test]
    fn one_write_can_overlap_multiple_previous_writes() {
        let mut map = RangeMap::new();
        map.write(0, 4, op(1));
        map.write(6, 4, op(2));
        map.write(2, 6, op(3));
        assert_eq!(
            origins(&map, 0, 10),
            vec![
                Some(1),
                Some(1),
                Some(3),
                Some(3),
                Some(3),
                Some(3),
                Some(3),
                Some(3),
                Some(2),
                Some(2)
            ]
        );
    }

    #[test]
    fn range_lookup_skips_unmapped_words() {
        let mut map = RangeMap::new();
        map.write(32, 32, op(1));
        map.write(64, 1, op(2));
        assert_eq!(map.get_by_range(0, 65), vec![op(1), op(2)]);
    }

    #[test]
    fn tracked_memory_origin_matches_the_actual_write_location() {
        let mut memory = crate::core::memory::Memory::new();
        memory.store_with_opcode(100_000, 1, &[42], op(1));
        assert_eq!(memory.origin(65_536), Some(op(1)));
        assert_eq!(memory.origin(100_000), None);
    }
}
