#[derive(Clone, Debug)]
pub struct Memory {
    pub memory: String
}


impl Memory {

    // Since bytearrays aren't supported by the Rust standard library,
    // we're gonna use a String to represent the bytearray.
    pub fn new() -> Memory {
        Memory { memory: String::new() }
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
        }
        else {
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
    pub fn store(&mut self, offset: usize, size: usize, mut value: String) {
        if  value.len() % 2 == 0 {

            // extend the value to 32 bytes
            value.insert_str(0, &"00".repeat(size - value.len() / 2));
            
            // extend the memory to allocate for the new space
            // byte offset is the str offset where we start writing
            self.extend(offset as u128, size as u128);

            // store the value in memory by replacing bytes in the memory
            self.memory.replace_range((offset*2)..(offset*2) + value.len(), &value)

        }
    }

    // read a value from the memory at the given offset, with a fixed size
    pub fn read(&self, offset: usize, size: usize) -> String {

        // if the offset + size will be out of bounds, append null bytes until the size is met
        if offset + size > self.size() as usize {
            let mut value = String::new();

            if offset <= self.size() as usize {
                value = self.memory[(offset*2)..].to_string();
            }
            
            value.push_str(&"00".repeat(size - value.len() / 2));
            value
        }
        else {
            self.memory[(offset*2)..(offset*2) + size*2].to_string()
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mstore() {
        let mut memory = Memory::new();

        memory.store(4, 32, String::from("ff"));
        assert_eq!(memory.memory, "0000000000000000000000000000000000000000000000000000000000000000000000ff00000000000000000000000000000000000000000000000000000000");
    
        let mut memory = Memory::new();
        memory.store(0, 32, String::from("ff"));
        assert_eq!(memory.memory, "00000000000000000000000000000000000000000000000000000000000000ff");

        let mut memory = Memory::new();
        memory.store(34, 32, String::from("ff"));
        assert_eq!(memory.memory, "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff000000000000000000000000000000000000000000000000000000000000");
    
        let mut memory = Memory::new();
        memory.store(0, 1, String::from("ff"));
        assert_eq!(memory.memory, "ff00000000000000000000000000000000000000000000000000000000000000");
    
        let mut memory = Memory::new();
        memory.store(255, 32, String::from("ff"));
        assert_eq!(memory.memory, "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff00");
    }

    #[test]
    fn test_mload() {
        let mut memory = Memory::new();
        memory.store(0, 32, String::from("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff"));
    
        assert_eq!(memory.read(0, 32), "11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff");
        assert_eq!(memory.read(1, 32), "223344556677889900aabbccddeeff11223344556677889900aabbccddeeff00");
        assert_eq!(memory.read(31, 32), "ff00000000000000000000000000000000000000000000000000000000000000");
    }

}