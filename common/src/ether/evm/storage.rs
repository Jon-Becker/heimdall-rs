use std::collections::HashMap;

#[derive(Clone, Debug)]
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
        value = value.replace("0x", "");
        if value.len() % 2 == 0 {

            // extend the key to 32 bytes
            if key.len() < 64 {
                key.insert_str(0, &"00".repeat(32 - key.len() / 2));
            }

            // extend the value to 32 bytes
            if value.len() < 64 {
                value.insert_str(0, &"00".repeat(32 - (value.len() / 2)));
            }

            // store the key:value pair
            self.storage.insert(key, value);
        }
    }

    // loads a key from the storage map
    pub fn load(&self, mut key: String) -> String {
        
        // extend the key to 32 bytes
        if key.len() < 64 {
            key.insert_str(0, &"00".repeat(32 - key.len() / 2));
        }

        // return the value associated with the key, with a null word if it doesn't exist
        return match self.storage.get(&key) {
            Some(value) => value.clone(),
            None => String::from("0000000000000000000000000000000000000000000000000000000000000000"),
        };
    }

}