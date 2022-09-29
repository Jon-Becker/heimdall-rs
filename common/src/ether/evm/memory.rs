#[derive(Clone, Debug)]
pub struct Memory {
    pub memory: String,
}

impl Memory {
    // Since bytearrays aren't supported by the Rust standard library,
    // we're gonna use a String to represent the bytearray.
    pub fn new() -> Memory {
        Memory {
            memory: String::new(),
        }
    }

    // get the size of the memory in bytes
    pub fn size(&self) -> u128 {
        return (self.memory.len() / 2) as u128;
    }

    pub fn extend(&mut self, offset: u128, size: u128) {
        // calculate the new size of the memory
        let r = (offset + size) % 32;
        let new_mem_size: u128;
        if r == 0 {
            new_mem_size = offset + size;
        } else {
            new_mem_size = offset + size + 32 - r;
        }

        let mut byte_difference = 0;
        if self.size() <= new_mem_size {
            byte_difference = new_mem_size - self.size();
        }

        // for every missing byte, append a null byte
        if byte_difference > 0 {
            self.memory.push_str(&"00".repeat(byte_difference as usize));
        }
    }

    // stores a bytearray in the memory at offset
    pub fn store(&mut self, mut offset: usize, size: usize, mut value: String) {
        if value.len() % 2 == 0 {
            // cap offset to 2**16 for ozptimization
            if offset > 65536 {
                offset = 65536;
            }

            // extend the value to size bytes
            if value.len() / 2 < size {
                value.insert_str(0, &"00".repeat(size - value.len() / 2));
            }

            // extend the memory to allocate for the new space
            // byte offset is the str offset where we start writing
            self.extend(offset as u128, size as u128);

            // reduce the value to size bytes
            value = value.get(value.len() - (size * 2)..).unwrap().to_string();

            // store the value in memory by replacing bytes in the memory
            self.memory
                .replace_range((offset * 2)..(offset * 2) + value.len(), &value);
        }
    }

    // read a value from the memory at the given offset, with a fixed size
    pub fn read(&self, mut offset: usize, size: usize) -> String {
        // cap offset to 2**16 for optimization
        if offset > 65536 {
            offset = 65536;
        }

        // if the offset + size will be out of bounds, append null bytes until the size is met
        if offset + size > self.size() as usize {
            let mut value = String::new();

            if offset <= self.size() as usize {
                value = self.memory[(offset * 2)..].to_string();
            }

            value.push_str(&"00".repeat(size - value.len() / 2));
            value
        } else {
            self.memory[(offset * 2)..(offset * 2) + size * 2].to_string()
        }
    }
}