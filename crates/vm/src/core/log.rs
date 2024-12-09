use alloy::primitives::U256;

/// The [`Log`] struct represents a log emitted by a `LOG0-LOG4` opcode.
#[derive(Clone, Debug)]
pub struct Log {
    pub index: u128,
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}

impl Log {
    /// Creates a new [`Log`] with the given log index, topics, and hex data.
    pub fn new(index: u128, topics: Vec<U256>, data: &[u8]) -> Log {
        Log { index, topics, data: data.to_vec() }
    }
}
