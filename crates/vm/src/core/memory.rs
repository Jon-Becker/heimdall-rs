#[cfg(feature = "experimental")]
use crate::ext::range_map::RangeMap;

#[cfg(feature = "experimental")]
use super::opcodes::WrappedOpcode;

/// A type alias for tracking byte ranges in memory with associated opcodes.
#[cfg(feature = "experimental")]
pub type ByteTracker = RangeMap;

/// The [`Memory`] struct represents the memory of an EVM.
#[derive(Clone, Debug)]
pub struct Memory {
    /// Vector storing memory data
    pub memory: Vec<u8>,

    #[cfg(feature = "experimental")]
    /// Byte-tracking facility, allowing bytes to be associated with the opcodes that last modified
    /// them
    pub bytes: ByteTracker,
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    /// Creates a new [`Memory`] with an empty memory vector and empty byte tracker
    pub fn new() -> Memory {
        Memory {
            memory: Vec::with_capacity(2048),
            #[cfg(feature = "experimental")]
            bytes: ByteTracker::new(),
        }
    }

    /// Gets the current size of the memory in bytes.
    ///
    /// ```
    /// use heimdall_vm::core::memory::Memory;
    ///
    /// let memory = Memory::new();
    /// assert_eq!(memory.size(), 0);
    /// ```
    pub fn size(&self) -> u128 {
        self.memory.len() as u128
    }

    /// Extends the memory to the given size, if necessary. \
    /// This is called when a memory store is performed, and the memory must be extended to fit the
    /// value.
    ///
    /// ```
    /// use heimdall_vm::core::memory::Memory;
    ///
    /// let mut memory = Memory::new();
    /// assert_eq!(memory.size(), 0);
    /// memory.extend(0, 32);
    /// assert_eq!(memory.size(), 32);
    /// ```
    pub fn extend(&mut self, offset: u128, size: u128) {
        // Calculate the new size of the memory
        let new_mem_size = (offset.saturating_add(size).saturating_add(31)) / 32 * 32;

        // If the new memory size is greater than the current size, extend the memory
        if new_mem_size > self.size() {
            let byte_difference = (new_mem_size - self.size()) as usize;
            self.memory.resize(self.memory.len().saturating_add(byte_difference), 0u8);
        }
    }

    /// Store the given bytes in the memory at the given offset, with a fixed size.
    /// May extend the memory if necessary.
    ///
    /// ```
    /// use heimdall_vm::core::memory::Memory;
    ///
    /// let mut memory = Memory::new();
    /// memory.store(0, 32, &[0xff]);
    /// assert_eq!(memory.read(0, 32), vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff]);
    /// ```
    pub fn store(&mut self, mut offset: usize, mut size: usize, value: &[u8]) {
        // Cap offset and size to 2**16
        offset = offset.min(65536);
        size = size.min(65536);

        let value_len = value.len();

        // Truncate or extend value to the desired size
        let value: Vec<u8> = if value_len >= size {
            value[..size].to_vec()
        } else {
            let mut value = value.to_vec();

            // prepend null bytes until the value is the desired size
            // ex, ff with size 4 -> 00 00 00 ff
            let null_bytes = vec![0u8; size - value_len];
            value.splice(0..0, null_bytes);
            value
        };

        // Extend the memory to allocate for the new space
        self.extend(offset as u128, size as u128);

        // Store the value in memory by replacing bytes in the memory
        self.memory.splice(offset..offset.saturating_add(size), value);
    }

    /// Stores a value in memory and records the opcode that performed the store operation
    ///
    /// This method is similar to `store()` but additionally records which opcode
    /// was responsible for the memory store operation when the experimental feature
    /// is enabled.
    ///
    /// # Arguments
    /// * `offset` - The byte offset in memory where the value will be stored
    /// * `size` - The size of the value in bytes
    /// * `value` - The value to store in memory
    /// * `opcode` - The opcode that performed the store operation (only used with experimental
    ///   feature)
    pub fn store_with_opcode(
        &mut self,
        offset: usize,
        size: usize,
        value: &[u8],
        #[cfg(feature = "experimental")] opcode: WrappedOpcode,
    ) {
        self.store(offset, size, value);
        #[cfg(feature = "experimental")]
        self.bytes.write(offset, size, opcode);
    }

    /// Read the given number of bytes from the memory at the given offset.
    /// If the offset + size is greater than the current size of the memory, null bytes will be
    /// appended to the value.
    ///
    /// ```
    /// use heimdall_vm::core::memory::Memory;
    ///
    /// let mut memory = Memory::new();
    /// memory.store(0, 32, &[0xff]);
    /// assert_eq!(memory.read(0, 32), vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff]);
    /// ```
    pub fn read(&self, offset: usize, size: usize) -> Vec<u8> {
        // Cap size to 2**16 and offset to 2**16 for optimization
        let size = size.min(65536);
        let offset = offset.min(65536);

        // If the offset + size will be out of bounds, append null bytes until the size is met
        if offset.saturating_add(size) > self.size() as usize {
            let mut value = Vec::with_capacity(size);

            if offset <= self.size() as usize {
                value.extend_from_slice(&self.memory[offset..]);
            }

            value.resize(size, 0u8);
            value
        } else {
            self.memory[offset..offset.saturating_add(size)].to_vec()
        }
    }

    /// Calculate the current memory cost
    ///
    /// ```
    /// use heimdall_vm::core::memory::Memory;
    ///
    /// let mut memory = Memory::new();
    /// memory.store(0, 32, &[0xff]);
    /// assert_eq!(memory.memory_cost(), 3);
    /// ```
    pub fn memory_cost(&self) -> u128 {
        // Calculate the new size of the memory
        let memory_word_size = (self.size().saturating_add(31)) / 32;
        ((memory_word_size.pow(2)) / 512).saturating_add(3 * memory_word_size)
    }

    /// calculate the memory cost of extending the memory to a given size
    ///
    /// ```
    /// use heimdall_vm::core::memory::Memory;
    ///
    /// let mut memory = Memory::new();
    /// memory.store(0, 32, &[0xff]);
    /// assert_eq!(memory.expansion_cost(0, 32), 0);
    /// assert_eq!(memory.expansion_cost(0, 64), 3);
    /// ```
    pub fn expansion_cost(&self, offset: usize, size: usize) -> u128 {
        // Calculate the new size of the memory
        let new_memory_word_size = ((offset.saturating_add(size).saturating_add(31)) / 32) as u128;
        let new_memory_cost =
            ((new_memory_word_size.pow(2)) / 512).saturating_add(3 * new_memory_word_size);
        if new_memory_cost < self.memory_cost() {
            0
        } else {
            new_memory_cost - self.memory_cost()
        }
    }

    #[cfg(feature = "experimental")]
    /// Given an offset into memory, returns the opcode that last modified it (if it has been
    /// modified at all)
    ///
    /// Due to the nature of `WrappedOpcode`, this allows the entire Cfg branch to be traversed.
    pub fn origin(&self, byte: usize) -> Option<WrappedOpcode> {
        self.bytes.get_by_offset(byte)
    }
}

#[cfg(test)]
mod tests {
    use heimdall_common::utils::strings::decode_hex;

    use super::*;

    #[test]
    fn test_mstore_simple() {
        let mut memory = Memory::new();
        memory.store(
            0,
            32,
            &decode_hex("00000000000000000000000000000000000000000000000000000000000000ff")
                .expect("failed to decode hex"),
        );
        assert_eq!(
            memory.memory,
            decode_hex("00000000000000000000000000000000000000000000000000000000000000ff")
                .expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mstore_extend() {
        let mut memory = Memory::new();
        memory.store(0, 32, &[0xff]);
        assert_eq!(
            memory.memory,
            decode_hex("00000000000000000000000000000000000000000000000000000000000000ff")
                .expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mstore_offset() {
        let mut memory = Memory::new();
        memory.store(4, 32, &[0xff]);
        assert_eq!(
            memory.memory,
            decode_hex("0000000000000000000000000000000000000000000000000000000000000000000000ff00000000000000000000000000000000000000000000000000000000").expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mstore_large_nonstandard_offset() {
        let mut memory = Memory::new();
        memory.store(34, 32, &[0xff]);
        assert_eq!(
            memory.memory,
            decode_hex("0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000").expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mstore8() {
        let mut memory = Memory::new();
        memory.store(0, 1, &[0xff]);
        assert_eq!(
            memory.memory,
            decode_hex("ff00000000000000000000000000000000000000000000000000000000000000")
                .expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mstore_large_offser() {
        let mut memory = Memory::new();
        memory.store(255, 32, &[0xff]);
        assert_eq!(
            memory.memory,
            decode_hex("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff00").expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mload_simple() {
        let mut memory = Memory::new();
        memory.store(
            0,
            32,
            &decode_hex("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")
                .expect("failed to decode hex"),
        );
        assert_eq!(
            memory.read(0, 32),
            decode_hex("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")
                .expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mload_pad_one() {
        let mut memory = Memory::new();
        memory.store(
            0,
            32,
            &decode_hex("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")
                .expect("failed to decode hex"),
        );
        assert_eq!(
            memory.read(1, 32),
            decode_hex("223344556677889900aabbccddeeff11223344556677889900aabbccddeeff00")
                .expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_mload_pad_large() {
        let mut memory = Memory::new();
        memory.store(
            0,
            32,
            &decode_hex("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")
                .expect("failed to decode hex"),
        );
        assert_eq!(
            memory.read(31, 32),
            decode_hex("ff00000000000000000000000000000000000000000000000000000000000000")
                .expect("failed to decode hex"),
        );
    }

    #[test]
    fn test_memory_cost() {
        let mut memory = Memory::new();
        memory.store(
            0,
            32,
            &decode_hex("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")
                .expect("failed to decode hex"),
        );
        assert_eq!(memory.memory_cost(), 3);
    }

    #[test]
    fn test_memory_cost_2() {
        let mut memory = Memory::new();
        memory.store(
            32 * 32,
            32,
            &decode_hex("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")
                .expect("failed to decode hex"),
        );
        assert_eq!(memory.memory_cost(), 101);
    }

    #[test]
    fn test_expansion_cost() {
        let memory = Memory::new();
        assert_eq!(memory.expansion_cost(0, 32), 3);
    }

    #[test]
    fn test_expansion_cost_2() {
        let memory = Memory::new();
        assert_eq!(memory.expansion_cost(32 * 32, 32), 101);
    }
}
