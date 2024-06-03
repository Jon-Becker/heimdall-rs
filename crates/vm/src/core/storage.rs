use std::collections::{HashMap, HashSet};

/// The [`Storage`] struct represents the storage of a contract. \
/// \
/// We keep track of the storage as a HashMap, as well as a HashSet of keys that have been accessed
/// for gas calculation purposes.
#[derive(Clone, Debug)]
pub struct Storage {
    pub storage: HashMap<[u8; 32], [u8; 32]>,
    pub transient: HashMap<[u8; 32], [u8; 32]>,
    access_set: HashSet<[u8; 32]>,
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
    ///
    /// let mut storage = Storage::new();
    /// storage.store([1u8; 32], [2u8; 32]);
    ///
    /// assert_eq!(storage.storage.get(&[1u8; 32]), Some(&[2u8; 32]));
    /// ```
    pub fn store(&mut self, key: [u8; 32], value: [u8; 32]) {
        self.access_set.insert(key);

        self.storage.insert(key, value);
    }

    /// Store a key-value pair in the transient storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    ///
    /// let mut storage = Storage::new();
    /// storage.tstore([1u8; 32], [2u8; 32]);
    ///
    /// assert_eq!(storage.transient.get(&[1u8; 32]), Some(&[2u8; 32]));
    /// ```
    pub fn tstore(&mut self, key: [u8; 32], value: [u8; 32]) {
        self.access_set.insert(key);

        self.transient.insert(key, value);
    }

    /// Load a value from the storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    ///
    /// let mut storage = Storage::new();
    /// storage.store([1u8; 32], [2u8; 32]);
    ///
    /// assert_eq!(storage.load([1u8; 32]), [2u8; 32]);
    /// ```
    pub fn load(&mut self, key: [u8; 32]) -> [u8; 32] {
        self.access_set.insert(key);

        // return the value associated with the key, with a null word if it doesn't exist
        match self.storage.get(&key) {
            Some(value) => *value,
            None => [0u8; 32],
        }
    }

    /// Load a value from the storage map.
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    ///
    /// let mut storage = Storage::new();
    /// storage.tstore([1u8; 32], [2u8; 32]);
    ///
    /// assert_eq!(storage.tload([1u8; 32]), [2u8; 32]);
    /// ```
    pub fn tload(&mut self, key: [u8; 32]) -> [u8; 32] {
        // return the value associated with the key, with a null word if it doesn't exist
        match self.transient.get(&key) {
            Some(value) => *value,
            None => [0u8; 32],
        }
    }

    /// calculate the cost of accessing a key in storage
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    ///
    /// let mut storage = Storage::new();
    ///
    /// // key `[1u8; 32]` is not warm, so the cost should be 2100
    /// assert_eq!(storage.access_cost([1u8; 32]), 2100);
    /// storage.store([1u8; 32], [2u8; 32]);
    ///
    /// // key `[1u8; 32]` is warm, so the cost should be 100
    /// assert_eq!(storage.access_cost([1u8; 32]), 100);
    /// ```
    pub fn access_cost(&mut self, key: [u8; 32]) -> u128 {
        if self.access_set.contains(&key) {
            100
        } else {
            self.access_set.insert(key);
            2100
        }
    }

    /// calculate the cost of storing a key-value pair in storage
    ///
    /// ```
    /// use heimdall_vm::core::storage::Storage;
    ///
    /// let mut storage = Storage::new();
    ///
    /// // value `[0u8; 32]` is zero, i.e. clearing a key, so the cost should be 2900 + self.access_cost(key)
    /// assert_eq!(storage.storage_cost([1u8; 32], [0u8; 32]), 5000);
    /// storage.store([1u8; 32], [2u8; 32]);
    ///
    /// // value `[2u8; 32]` is not zero, so the cost should be 20000 + self.access_cost(key)
    /// assert_eq!(storage.storage_cost([1u8; 32], [2u8; 32]), 20100);
    /// ```
    pub fn storage_cost(&mut self, key: [u8; 32], value: [u8; 32]) -> u128 {
        if value == [0u8; 32] {
            2900 + self.access_cost(key)
        } else {
            20000 + self.access_cost(key)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::storage::Storage;

    #[test]
    fn test_sstore_sload() {
        let mut storage = Storage::new();

        storage.store(
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ],
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ],
        );
        assert_eq!(
            storage.load([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1
            ]),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1
            ]
        );

        storage.store(
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 255,
            ],
            [
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 2, 3, 4, 5, 6,
                7, 8, 2, 1,
            ],
        );
        assert_eq!(
            storage.load([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 255
            ]),
            [
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 2, 3, 4, 5, 6,
                7, 8, 2, 1
            ],
        );

        assert_eq!(
            storage.load([
                255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 255
            ]),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0
            ]
        );
    }

    #[test]
    fn test_storage_access_cost_cold() {
        let mut storage = Storage::new();
        assert_eq!(
            storage.access_cost([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1
            ]),
            2100
        );
    }

    #[test]
    fn test_storage_access_cost_warm() {
        let mut storage = Storage::new();
        storage.load([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ]);
        assert_eq!(
            storage.access_cost([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1
            ]),
            100
        );
    }

    #[test]
    fn test_storage_storage_cost_cold() {
        let mut storage = Storage::new();
        assert_eq!(
            storage.storage_cost(
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ],
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ]
            ),
            22100
        );
    }

    #[test]
    fn test_storage_storage_cost_cold_zero() {
        let mut storage = Storage::new();
        assert_eq!(
            storage.storage_cost(
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ],
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0
                ]
            ),
            5000
        );
    }

    #[test]
    fn test_storage_storage_cost_warm() {
        let mut storage = Storage::new();
        storage.store(
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ],
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ],
        );
        assert_eq!(
            storage.storage_cost(
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ],
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ]
            ),
            20100
        );
    }

    #[test]
    fn test_storage_storage_cost_warm_zero() {
        let mut storage = Storage::new();
        storage.store(
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ],
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ],
        );
        assert_eq!(
            storage.storage_cost(
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1
                ],
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0
                ]
            ),
            3000
        );
    }
}
