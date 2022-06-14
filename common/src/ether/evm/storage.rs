use std::collections::HashMap;

pub struct Storage {
    pub storage: HashMap<String, String>
}


impl Storage {

    // Since bytearrays aren't supported by the Rust standard library,
    // we're gonna use a String to represent the k:v pairs.
    pub fn new() -> Storage {
        Storage { storage: HashMap::new() }
    }

    // stores a key:value pair in the storage map
    pub fn store(&mut self, mut key: String, mut value: String) {
        if value.len() % 2 == 0 {

            // extend the key to 32 bytes
            key.insert_str(0, &"00".repeat(32 - key.len() / 2));

            // extend the value to 32 bytes
            value.insert_str(0, &"00".repeat(32 - value.len() / 2));

            // store the key:value pair
            self.storage.insert(key, value);
        }
    }

    // loads a key from the storage map
    pub fn load(&self, mut key: String) -> String {
        
        // extend the key to 32 bytes
        key.insert_str(0, &"00".repeat(32 - key.len() / 2));

        // return the value associated with the key, with a null word if it doesn't exist
        return match self.storage.get(&key) {
            Some(value) => value.clone(),
            None => String::from("0000000000000000000000000000000000000000000000000000000000000000"),
        };
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sstore_sload() {
        let mut storage = Storage::new();

        storage.store(String::from("01"), String::from("01"));
        assert_eq!(storage.load(String::from("01")), String::from("0000000000000000000000000000000000000000000000000000000000000001"));

        storage.store(String::from("ff"), String::from("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff"));
        assert_eq!(storage.load(String::from("ff")), String::from("11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff"));

        assert_eq!(storage.load(String::from("00")), String::from("0000000000000000000000000000000000000000000000000000000000000000"));
    }

}