use std::collections::HashMap;

use ethers::types::U256;
use heimdall_common::ether::{
    evm::core::{log::Log, opcodes::WrappedOpcode},
    signatures::{ResolvedError, ResolvedFunction, ResolvedLog},
};

#[derive(Clone, Debug)]
pub struct Snapshot {
    // the function's 4byte selector
    pub selector: String,

    // the function's entry point in the code.
    // the entry point is the instruction the dispatcher JUMPs to when called.
    pub entry_point: u128,

    // argument structure:
    //   - key : slot operations of the argument.
    //   - value : tuple of ({slot: U256, mask: usize}, potential_types)
    pub arguments: HashMap<usize, (CalldataFrame, Vec<String>)>,

    // storage structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub storage: HashMap<U256, StorageFrame>,

    // memory structure:
    //   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    //   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub memory: HashMap<U256, StorageFrame>,

    // returns the return type for the function.
    pub returns: Option<String>,

    // holds all found events used to generate solidity error definitions
    // as well as ABI specifications.
    pub events: HashMap<U256, (Option<ResolvedLog>, Log)>,

    // holds all found custom errors used to generate solidity error definitions
    // as well as ABI specifications.
    pub errors: HashMap<U256, Option<ResolvedError>>,

    // stores the matched resolved function for this Functon
    pub resolved_function: Option<ResolvedFunction>,

    // modifiers
    pub pure: bool,
    pub view: bool,
    pub payable: bool,
}

#[derive(Clone, Debug)]
pub struct StorageFrame {
    pub value: U256,
    pub operations: WrappedOpcode,
}

#[derive(Clone, Debug)]
pub struct CalldataFrame {
    pub slot: usize,
    pub operation: String,
    pub mask_size: usize,
    pub heuristics: Vec<String>,
}
