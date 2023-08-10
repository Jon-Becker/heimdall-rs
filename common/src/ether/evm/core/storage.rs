use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Storage {
    // HashMap of [u8; 32] -> [u8; 32]
    pub storage: HashMap<[u8; 32], [u8; 32]>,
    access_set: HashSet<[u8; 32]>,
}

impl Storage {
    // Use sized HashMap of u8 to repr bytes32
    pub fn new() -> Storage {
        Storage { storage: HashMap::new(), access_set: HashSet::new() }
    }

    // stores a key:value pair in the storage map
    pub fn store(&mut self, key: [u8; 32], value: [u8; 32]) {
        self.access_set.insert(key);

        self.storage.insert(key, value);
    }

    // loads a key from the storage map
    pub fn load(&mut self, key: [u8; 32]) -> [u8; 32] {
        self.access_set.insert(key);

        // return the value associated with the key, with a null word if it doesn't exist
        match self.storage.get(&key) {
            Some(value) => *value,
            None => [0u8; 32],
        }
    }

    pub fn access_cost(&mut self, key: [u8; 32]) -> u128 {
        if self.access_set.contains(&key) {
            100
        } else {
            self.access_set.insert(key);
            2100
        }
    }

    pub fn storage_cost(&mut self, key: [u8; 32], value: [u8; 32]) -> u128 {
        if value == [0u8; 32] {
            2900 + self.access_cost(key)
        } else {
            20000 + self.access_cost(key)
        }
    }
}
