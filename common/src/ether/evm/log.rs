use ethers::prelude::U256;

#[derive(Clone, Debug)]
pub struct Log {
    pub index: u128,
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}

impl Log {
    // Implements a new log with the given index and "emits"
    // the log at the given index.
    pub fn new(index: u128, topics: Vec<U256>, data: &[u8]) -> Log {
        Log { index: index, topics: topics, data: data.to_vec() }
    }
}
