use ethers::{prelude::U256, abi::AbiEncode};


#[derive(Clone, Debug)]
pub struct Log {
    pub index: u128,
    pub topics: Vec<String>,
    pub data: String,
}


impl Log {

    // Implements a new log with the given index and "emits"
    // the log at the given index.
    pub fn new(index: u128, topics: Vec<U256>, data: String) -> Log {

        let mut topics_as_strings = Vec::new();
        
        for topic in topics {
            topics_as_strings.push(topic.encode_hex().replace("0x", ""));
        }

        Log {
            index: index,
            topics: topics_as_strings,
            data: data,
        }
    }

    
}