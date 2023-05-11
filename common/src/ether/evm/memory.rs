#[derive(Clone, Debug)]
pub struct Memory {
    pub memory: Vec<u8>,
}

impl Memory {
    // Repr as a [u8]
    pub fn new() -> Memory {
        Memory { memory: Vec::new() }
    }

    // get the size of the memory in bytes
    pub fn size(&self) -> u128 {
        self.memory.len() as u128
    }

    // extend the memory to a given size
    pub fn extend(&mut self, offset: u128, size: u128) {
        // Calculate the new size of the memory
        let new_mem_size = (offset + size + 31) / 32 * 32;

        // If the new memory size is greater than the current size, extend the memory
        if new_mem_size > self.size() {
            let byte_difference = (new_mem_size - self.size()) as usize;
            self.memory.resize(self.memory.len() + byte_difference, 0u8);
        }
    }

    // stores a bytearray in the memory at offset
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
        self.memory.splice(offset..offset + size, value);
    }

    // read a value from the memory at the given offset, with a fixed size
    pub fn read(&self, offset: usize, size: usize) -> Vec<u8> {
        // Cap size to 2**16 and offset to 2**16 for optimization
        let size = size.min(65536);
        let offset = offset.min(65536);

        // If the offset + size will be out of bounds, append null bytes until the size is met
        if offset + size > self.size() as usize {
            let mut value = Vec::with_capacity(size);

            if offset <= self.size() as usize {
                value.extend_from_slice(&self.memory[offset..]);
            }

            value.resize(size, 0u8);
            value
        } else {
            self.memory[offset..offset + size].to_vec()
        }
    }
}
