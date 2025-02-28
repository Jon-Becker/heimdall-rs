use alloy::primitives::U256;

/// The [`Log`] struct represents a log emitted by a `LOG0-LOG4` opcode.
#[derive(Clone, Debug)]
pub struct Log {
    /// The index position of the log in the transaction
    pub index: u128,

    /// The log topics (up to 4 for LOG0-LOG4)
    pub topics: Vec<U256>,

    /// The raw data contained in the log
    pub data: Vec<u8>,
}

impl Log {
    /// Creates a new [`Log`] with the given log index, topics, and hex data.
    pub fn new(index: u128, topics: Vec<U256>, data: &[u8]) -> Log {
        Log { index, topics, data: data.to_vec() }
    }
}
