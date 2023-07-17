use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Storage {
    // HashMap of [u8; 32] -> [u8; 32]
    pub storage: HashMap<[u8; 32], [u8; 32]>,
}

impl Storage {
    // Use sized HashMap of u8 to repr bytes32
    pub fn new() -> Storage {
        Storage { storage: HashMap::new() }
    }

    // stores a key:value pair in the storage map
    pub fn store(&mut self, key: [u8; 32], value: [u8; 32]) {
        self.storage.insert(key, value);
    }

    // loads a key from the storage map
    pub fn load(&self, key: [u8; 32]) -> [u8; 32] {
        // return the value associated with the key, with a null word if it doesn't exist
        return match self.storage.get(&key) {
            Some(value) => *value,
            None => [0u8; 32],
        }
    }
}
