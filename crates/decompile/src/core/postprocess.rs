use hashbrown::HashMap;
use std::time::Instant;

use eyre::eyre;
use heimdall_common::utils::strings::find_balanced_encapsulator;
use tracing::debug;

use crate::{
    interfaces::AnalyzedFunction,
    utils::{
        constants::STORAGE_ACCESS_REGEX,
        postprocessors::{
            arithmetic_postprocessor, bitwise_mask_postprocessor, memory_postprocessor,
            storage_postprocessor, transient_postprocessor, variable_postprocessor, Postprocessor,
        },
    },
    Error,
};

use super::analyze::AnalyzerType;

/// State shared between postprocessors
#[derive(Debug, Clone, Default)]
pub(crate) struct PostprocessorState {
    /// A mapping from memory locations to their corresponding variable names
    pub memory_map: HashMap<String, String>,
    /// A mapping which holds the last assigned value for a given variable
    pub variable_map: HashMap<String, String>,
    /// A mapping which holds inferred types for memory variables
    pub memory_type_map: HashMap<String, String>,
    /// A mapping from storage locations to their corresponding variable names
    pub storage_map: HashMap<String, String>,
    /// A mapping which holds inferred types for storage variables
    pub storage_type_map: HashMap<String, String>,
    /// A mapping from transient storage locations to their corresponding variable names
    pub transient_map: HashMap<String, String>,
    /// A mapping which holds inferred types for transient storage variables
    pub transient_type_map: HashMap<String, String>,
    /// An optional field which holds the storage location if the function is a public getter
    pub maybe_getter_for: Option<String>,
}

/// The [`PostprocessorOrchestrator`] is responsible for managing the cleanup of
/// generated code from [`AnalyzedFunction`]s passed into [`PostprocessorOrchestrator::postprocess`]
///
/// Depending on [`AnalyzerType`], different postprocessors will be registered and run on the
/// [`AnalyzedFunction`]
pub(crate) struct PostprocessOrchestrator {
    /// The type of postprocessor to use. this is taken from the analyzer
    typ: AnalyzerType,
    /// A list of registered postprocessors
    postprocessors: Vec<Postprocessor>,
    /// The state shared between postprocessors
    state: PostprocessorState,
}

impl PostprocessOrchestrator {
    /// Build a new postprocessor with the given analyzer type
    pub(crate) fn new(typ: AnalyzerType) -> Result<Self, Error> {
        let mut orchestrator =
            Self { typ, postprocessors: Vec::new(), state: PostprocessorState::default() };
        orchestrator.register_postprocessors()?;
        Ok(orchestrator)
    }

    /// Register heuristics for the given function and trace
    pub(crate) fn register_postprocessors(&mut self) -> Result<(), Error> {
        match self.typ {
            AnalyzerType::Solidity => {
                self.postprocessors.push(Postprocessor::new(bitwise_mask_postprocessor));
                self.postprocessors.push(Postprocessor::new(arithmetic_postprocessor));
                self.postprocessors.push(Postprocessor::new(memory_postprocessor));
                self.postprocessors.push(Postprocessor::new(storage_postprocessor));
                self.postprocessors.push(Postprocessor::new(transient_postprocessor));
                self.postprocessors.push(Postprocessor::new(variable_postprocessor));
            }
            AnalyzerType::Yul => {}
            _ => {}
        };

        Ok(())
    }

    /// Performs postprocessing
    pub(crate) fn postprocess(
        &mut self,
        function: &mut AnalyzedFunction,
    ) -> Result<PostprocessorState, Error> {
        debug!(
            "postprocessing decompiled logic for '{}' with the {} postprocessor",
            function.selector, self.typ
        );
        let start_postprocess_time = Instant::now();

        // get postprocessor state
        let mut state = PostprocessorState {
            storage_map: self.state.storage_map.clone(),
            transient_map: self.state.transient_map.clone(),
            storage_type_map: self.state.storage_type_map.clone(),
            transient_type_map: self.state.transient_type_map.clone(),
            ..Default::default()
        };

        // add the function arguments to memory_type_map
        state.memory_type_map.extend(function.arguments.iter().map(|(i, frame)| {
            (
                format!("arg{i}"),
                frame.potential_types().first().cloned().unwrap_or_else(|| String::from("bytes32")),
            )
        }));

        // add known variables to memory_type_map
        state.memory_type_map.extend([
            (String::from(".balance"), String::from("uint256")),
            (String::from(".blockhash"), String::from("bytes32")),
            (String::from(".codehash"), String::from("bytes32")),
            (String::from(".sender"), String::from("address")),
            (String::from(".origin"), String::from("address")),
            (String::from(".timestamp"), String::from("uint256")),
            (String::from(".value"), String::from("uint256")),
            (String::from(".length"), String::from("uint256")),
            (String::from(".coinbase"), String::from("address")),
            (String::from(".number"), String::from("uint256")),
            (String::from(".prevrandao"), String::from("uint256")),
            (String::from(".gaslimit"), String::from("uint256")),
            (String::from(".chainid"), String::from("uint256")),
        ]);

        // If this is a constant / getter, we can simplify it
        // Note: this can't be done with a postprocessor because it needs all lines
        if !function.payable && (function.pure || function.view) && function.arguments.is_empty() {
            // check for RLP encoding. very naive check, but it works for now
            if function.logic.iter().any(|line| line.contains("0x0100 *")) &&
                function.logic.iter().any(|line| line.contains("0x01) &"))
            {
                // find any storage accesses
                let joined = function.logic.join(" ");
                if let Some(storage_access) = STORAGE_ACCESS_REGEX.find(&joined).unwrap_or(None) {
                    let storage_access = storage_access.as_str();
                    let access_range = find_balanced_encapsulator(storage_access, ('[', ']'))
                        .map_err(|e| eyre!("failed to find access range: {e}"))?;

                    // update returns
                    function.returns = Some(String::from("string memory"));
                    function.logic = vec![format!(
                        "return string(rlp.encodePacked(storage[{}]));",
                        storage_access[access_range].to_string()
                    )]
                }
            }

            // iterate over logic, if we find a return w/ a storage variable:
            if let Some(line) = function
                .logic
                .iter()
                .find(|line| line.contains("return") && line.contains("storage"))
            {
                if let Some(storage_access) = STORAGE_ACCESS_REGEX.find(line).unwrap_or(None) {
                    let storage_access = storage_access.as_str();
                    let access_range = find_balanced_encapsulator(storage_access, ('[', ']'))
                        .map_err(|e| eyre!("failed to find access range: {e}"))?;

                    state.maybe_getter_for =
                        Some(format!("storage[{}]", &storage_access[access_range]));
                }
            }
        }

        // for each line in the function, run the postprocessors
        function.logic.iter_mut().for_each(|line| {
            self.postprocessors.iter().for_each(|heuristic| {
                heuristic.run(line, &mut state).unwrap();
            });
        });

        // wherever storage_map contains a value that doesnt exist in storage_type_map, add it with
        // a default value
        state.storage_map.iter().for_each(|(_, v)| {
            let storage_var_name = v.split('[').collect::<Vec<&str>>()[0];
            if !state.storage_type_map.contains_key(storage_var_name) {
                if storage_var_name.contains("map") {
                    state.storage_type_map.insert(
                        storage_var_name.to_string(),
                        "mapping(bytes32 => bytes32)".to_string(),
                    );
                } else {
                    state
                        .storage_type_map
                        .insert(storage_var_name.to_string(), "bytes32".to_string());
                }
            }
        });
        state.transient_map.iter().for_each(|(_, v)| {
            let storage_var_name = v.split('[').collect::<Vec<&str>>()[0];
            if !state.transient_type_map.contains_key(storage_var_name) {
                if storage_var_name.contains("map") {
                    state.transient_type_map.insert(
                        storage_var_name.to_string(),
                        "mapping(bytes32 => bytes32)".to_string(),
                    );
                } else {
                    state
                        .transient_type_map
                        .insert(storage_var_name.to_string(), "bytes32".to_string());
                }
            }
        });

        // update the state, so we can share it between functions
        self.state = state;

        // if this is a getter, replace function.maybe_getter_for with the actual getter
        if let Some(getter_for) =
            self.state.maybe_getter_for.as_ref().or(function.maybe_getter_for.as_ref())
        {
            function.maybe_getter_for = self.state.storage_map.get(getter_for).cloned();
        }

        debug!(
            "postprocessing for '{}' completed in {:?}",
            function.selector,
            start_postprocess_time.elapsed()
        );

        Ok(self.state.clone())
    }
}
