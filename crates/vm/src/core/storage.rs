use hashbrown::{HashMap, HashSet};

use alloy::primitives::U256;

/// The [`Storage`] struct represents the storage of a contract. \
/// \
/// We keep track of the storage as a HashMap, as well as a HashSet of keys that have been accessed
/// for gas calculation purposes.
#[derive(Clone, Debug)]
pub struct Storage {
    /// The persistent storage of the contract, mapping 256-bit keys to 256-bit values.
    ///
    /// This represents the permanent state storage that persists between transactions.
    pub storage: HashMap<U256, U256>,

    /// The transient storage of the contract, mapping 256-bit keys to 256-bit values.
    ///
    /// This represents temporary storage that only persists for the duration of a transaction
    /// (introduced in EIP-1153).
    pub transient: HashMap<U256, U256>,

    /// A set of storage keys that have been accessed during execution.
    ///
    /// This is used for gas calculation purposes, as accessing a "cold" storage slot
    /// costs more gas than accessing a "warm" one.
    access_set: HashSet<U256>,
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage {
    /// Creates a new [`Storage`] struct.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let storage = Storage::new();
    /// ```
    pub fn new() -> Storage {
        Storage { storage: HashMap::new(), access_set: HashSet::new(), transient: HashMap::new() }
    }

    /// Store a key-value pair in the storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let mut storage = Storage::new();
    /// storage.store(U256::from(1), U256::from(2));
    ///
    /// assert_eq!(storage.storage.get(&U256::from(1)), Some(&U256::from(2)));
    /// ```
    pub fn store(&mut self, key: U256, value: U256) {
        self.access_set.insert(key);

        self.storage.insert(key, value);
    }

    /// Store a key-value pair in the transient storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let mut storage = Storage::new();
    /// storage.tstore(U256::from(1), U256::from(1));
    ///
    /// assert_eq!(storage.transient.get(&U256::from(1)), Some(&U256::from(1)));
    /// ```
    pub fn tstore(&mut self, key: U256, value: U256) {
        self.access_set.insert(key);

        self.transient.insert(key, value);
    }

    /// Load a value from the storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let mut storage = Storage::new();
    /// storage.store(U256::from(1), U256::from(1));
    ///
    /// assert_eq!(storage.load(U256::from(1)), U256::from(1));
    /// ```
    pub fn load(&mut self, key: U256) -> U256 {
        self.access_set.insert(key);

        // return the value associated with the key, with a null word if it doesn't exist
        match self.storage.get(&key) {
            Some(value) => *value,
            None => U256::ZERO,
        }
    }

    /// Load a value from the storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let mut storage = Storage::new();
    /// storage.tstore(U256::from(1), U256::from(1));
    ///
    /// assert_eq!(storage.tload(U256::from(1)), U256::from(1));
    /// ```
    pub fn tload(&mut self, key: U256) -> U256 {
        // return the value associated with the key, with a null word if it doesn't exist
        match self.transient.get(&key) {
            Some(value) => *value,
            None => U256::ZERO,
        }
    }

    /// calculate the cost of accessing a key in storage
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let mut storage = Storage::new();
    ///
    /// // key `U256::from(1)` is not warm, so the cost should be 2100
    /// assert_eq!(storage.access_cost(U256::from(1)), 2100);
    /// storage.store(U256::from(1), U256::from(1));
    ///
    /// // key `U256::from(1)` is warm, so the cost should be 100
    /// assert_eq!(storage.access_cost(U256::from(1)), 100);
    /// ```
    pub fn access_cost(&mut self, key: U256) -> u128 {
        if self.access_set.contains(&key) {
            100
        } else {
            self.access_set.insert(key.to_owned());
            2100
        }
    }

    /// calculate the cost of storing a key-value pair in storage
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    /// use alloy::primitives::U256;
    ///
    /// let mut storage = Storage::new();
    ///
    /// // value `U256::from(1)` is zero, i.e. clearing a key, so the cost should be 2900 + self.access_cost(key)
    /// assert_eq!(storage.storage_cost(U256::from(1), U256::ZERO), 5000);
    /// storage.store(U256::from(1), U256::from(1));
    ///
    /// // value `U256::from(1)` is not zero, so the cost should be 20000 + self.access_cost(key)
    /// assert_eq!(storage.storage_cost(U256::from(1), U256::from(2)), 20100);
    /// ```
    pub fn storage_cost(&mut self, key: U256, value: U256) -> u128 {
        if value == U256::ZERO {
            2900 + self.access_cost(key)
        } else {
            20000 + self.access_cost(key)
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::U256;

    use crate::core::storage::Storage;

    #[test]
    fn test_sstore_sload() {
        let mut storage = Storage::new();

        storage.store(U256::from(1), U256::from(1));
        assert_eq!(storage.load(U256::from(1),), U256::from(1),);

        storage.store(U256::from(256), U256::MAX);
        assert_eq!(storage.load(U256::from(256)), U256::MAX,);

        assert_eq!(storage.load(U256::from(2)), U256::ZERO,);
    }

    #[test]
    fn test_storage_access_cost_cold() {
        let mut storage = Storage::new();
        assert_eq!(storage.access_cost(U256::from(1)), 2100);
    }

    #[test]
    fn test_storage_access_cost_warm() {
        let mut storage = Storage::new();
        storage.load(U256::from(1));
        assert_eq!(storage.access_cost(U256::from(1)), 100);
    }

    #[test]
    fn test_storage_storage_cost_cold() {
        let mut storage = Storage::new();
        assert_eq!(storage.storage_cost(U256::from(1), U256::from(1),), 22100);
    }

    #[test]
    fn test_storage_storage_cost_cold_zero() {
        let mut storage = Storage::new();
        assert_eq!(storage.storage_cost(U256::from(1), U256::ZERO,), 5000);
    }

    #[test]
    fn test_storage_storage_cost_warm() {
        let mut storage = Storage::new();
        storage.store(U256::from(1), U256::from(1));
        assert_eq!(storage.storage_cost(U256::from(1), U256::from(1),), 20100);
    }

    #[test]
    fn test_storage_storage_cost_warm_zero() {
        let mut storage = Storage::new();
        storage.store(U256::from(1), U256::from(1));
        assert_eq!(storage.storage_cost(U256::from(1), U256::ZERO,), 3000);
    }
}
