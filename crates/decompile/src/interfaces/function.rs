use hashbrown::{HashMap, HashSet};

use alloy::primitives::U256;
use heimdall_common::ether::signatures::ResolvedFunction;
use heimdall_vm::core::{opcodes::WrappedOpcode, types::byte_size_to_type};

use crate::core::analyze::AnalyzerType;

/// The [`AnalyzedFunction`] struct represents a function that has been analyzed by the decompiler.
#[derive(Clone, Debug)]
pub(crate) struct AnalyzedFunction {
    /// the function's 4byte selector
    pub selector: String,

    /// argument structure:
    ///   - key : slot operations of the argument.
    ///   - value : tuple of ({slot: U256, mask: usize}, potential_types)
    pub arguments: HashMap<usize, CalldataFrame>,

    /// memory structure:
    ///   - key : slot of the argument. I.E: slot 0 is CALLDATALOAD(4).
    ///   - value : tuple of ({value: U256, operation: WrappedOpcode})
    pub memory: HashMap<U256, StorageFrame>,

    /// returns the return type for the function.
    pub returns: Option<String>,

    /// holds function logic to be written to the output solidity file.
    pub logic: Vec<String>,

    /// holds all found event selectors found
    pub events: HashSet<U256>,

    /// holds all found custom error selectors found
    pub errors: HashSet<U256>,

    /// stores the matched resolved function for this Functon
    pub resolved_function: Option<ResolvedFunction>,

    /// stores decompiler notices
    pub notices: Vec<String>,

    /// modifiers
    pub pure: bool,
    pub view: bool,
    pub payable: bool,

    /// whether this is the fallback function for the contract
    pub fallback: bool,

    /// the analyzer type used to analyze this function
    pub analyzer_type: AnalyzerType,

    /// the underlying storage variable, if this is a public getter
    pub maybe_getter_for: Option<String>,

    /// optional constant value for this function
    pub constant_value: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct StorageFrame {
    pub operation: WrappedOpcode,
    pub value: U256,
}

#[derive(Clone, Debug)]
pub(crate) struct CalldataFrame {
    pub arg_op: String,
    pub mask_size: usize,
    pub heuristics: HashSet<TypeHeuristic>,
}

impl CalldataFrame {
    /// Get the potential types for the given argument
    pub(crate) fn potential_types(&self) -> Vec<String> {
        // get all potential types that can fit in self.mask_size
        byte_size_to_type(self.mask_size).1.to_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub(crate) enum TypeHeuristic {
    Numeric,
    Bytes,
    Boolean,
}

impl AnalyzedFunction {
    pub(crate) fn new(selector: &str, fallback: bool) -> Self {
        AnalyzedFunction {
            selector: if fallback { "00000000".to_string() } else { selector.to_string() },
            arguments: HashMap::new(),
            memory: HashMap::new(),
            returns: None,
            logic: Vec::new(),
            events: HashSet::new(),
            errors: HashSet::new(),
            resolved_function: None,
            notices: Vec::new(),
            pure: true,
            view: true,
            payable: true,
            analyzer_type: AnalyzerType::Abi,
            fallback,
            maybe_getter_for: None,
            constant_value: None,
        }
    }

    /// Whether this is a constant or not
    pub(crate) fn is_constant(&self) -> bool {
        self.pure && self.arguments.is_empty()
    }

    /// Gets the inputs for a range of memory
    pub(crate) fn get_memory_range(&self, _offset: U256, _size: U256) -> Vec<StorageFrame> {
        let mut memory_slice: Vec<StorageFrame> = Vec::new();

        // Safely convert U256 to usize
        let mut offset: usize = std::cmp::min(_offset.try_into().unwrap_or(0), 2048);
        let mut size: usize = std::cmp::min(_size.try_into().unwrap_or(0), 2048);

        // get the memory range
        while size > 0 {
            if let Some(memory) = self.memory.get(&U256::from(offset)) {
                memory_slice.push(memory.clone());
            }
            offset += 32;
            size = size.saturating_sub(32);
        }

        memory_slice
    }

    /// Get the arguments in a sorted vec
    pub(crate) fn sorted_arguments(&self) -> Vec<(usize, CalldataFrame)> {
        let mut arguments: Vec<_> = self.arguments.clone().into_iter().collect();
        arguments.sort_by(|x, y| x.0.cmp(&y.0));
        arguments
    }
}
