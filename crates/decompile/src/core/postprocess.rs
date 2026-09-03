use hashbrown::HashMap;
use std::time::Instant;

use alloy::primitives::U256;
use tracing::debug;

use crate::{
    interfaces::AnalyzedFunction,
    utils::postprocessors::{
        arithmetic_postprocessor, bitwise_mask_postprocessor, detect_string_storage_getter,
        eliminate_dead_variables, inline_single_use_variables, memory_postprocessor,
        normalize_typed_returns, storage_inference_postprocessor, storage_postprocessor,
        structure_control_flow, transient_postprocessor, type_cleanup_postprocessor,
        variable_postprocessor, IrFunctionPostprocessor, IrPostprocessor,
    },
    Error,
};

use super::{
    analyze::AnalyzerType,
    ir::{BinaryOp, Expr, Statement},
};

fn find_expression(
    statements: &[Statement],
    mut predicate: impl FnMut(&Expr) -> bool,
) -> Option<Expr> {
    let mut found = None;
    for statement in statements {
        let mut statement = statement.clone();
        statement.visit_exprs_mut(&mut |expr| {
            if found.is_none() && predicate(expr) {
                found = Some(expr.clone());
            }
        });
        if found.is_some() {
            break;
        }
    }
    found
}

fn is_storage_access(expr: &Expr) -> bool {
    matches!(expr, Expr::StorageAccess(_))
}

fn path_has_packed_width(path: &super::ir::StoragePath, width: u16) -> bool {
    match path {
        super::ir::StoragePath::PackedField { parent, bit_width, .. } => {
            *bit_width == width || path_has_packed_width(parent, width)
        }
        super::ir::StoragePath::Mapping { parent, .. } |
        super::ir::StoragePath::DynamicArray { parent, .. } |
        super::ir::StoragePath::Field { parent, .. } => path_has_packed_width(parent, width),
        super::ir::StoragePath::Slot { .. } => false,
    }
}

fn has_binary_literal(statements: &[Statement], op: BinaryOp, literal: U256) -> bool {
    find_expression(statements, |expr| {
        matches!(
            expr,
            Expr::Binary { op: candidate, lhs, rhs }
                if *candidate == op &&
                    (matches!(&**lhs, Expr::Literal(value) if *value == literal) ||
                     matches!(&**rhs, Expr::Literal(value) if *value == literal))
        )
    })
    .is_some()
}

/// State shared between postprocessors
#[derive(Debug, Clone, Default)]
pub(crate) struct PostprocessorState {
    /// Preimages captured when SHA3 executes, keyed by the unresolved hash expression.
    pub keccak_preimages: HashMap<Expr, Vec<Vec<Expr>>>,
    /// Parent Keccak histories used to isolate conditional branches.
    pub keccak_preimage_scopes: Vec<HashMap<Expr, Vec<Vec<Expr>>>>,
    /// Symbolic values written to constant memory offsets, used for Keccak preimages.
    pub symbolic_memory: HashMap<U256, Expr>,
    /// Parent memory states used to prevent writes from leaking out of conditional branches.
    pub symbolic_memory_scopes: Vec<HashMap<U256, Expr>>,
    /// A mapping from memory locations to their corresponding variable names
    pub memory_map: HashMap<Expr, Expr>,
    /// A mapping which holds the last assigned value for a given variable
    pub variable_map: HashMap<Expr, Expr>,
    /// A mapping which holds inferred types for memory variables
    pub memory_type_map: HashMap<String, String>,
    /// Canonical root slots and their generated source names.
    pub storage_roots: HashMap<Expr, String>,
    /// Type hints associated with canonical root slots.
    pub storage_type_hints: HashMap<Expr, String>,
    /// Generated variable names mapped back to their physical base slots.
    pub storage_root_slots: HashMap<String, Expr>,
    /// A mapping from storage locations to their corresponding variable names
    pub storage_map: HashMap<Expr, Expr>,
    /// A mapping which holds inferred types for storage variables
    pub storage_type_map: HashMap<String, String>,
    /// A mapping from transient storage locations to their corresponding variable names
    pub transient_map: HashMap<Expr, Expr>,
    /// A mapping which holds inferred types for transient storage variables
    pub transient_type_map: HashMap<String, String>,
    /// An optional field which holds the storage location if the function is a public getter
    pub maybe_getter_for: Option<Expr>,
    /// Current conditional nesting depth during flat-statement iteration.
    pub conditional_depth: usize,
}

/// The [`PostprocessOrchestrator`] is responsible for managing the cleanup of
/// generated code from [`AnalyzedFunction`]s passed into [`PostprocessOrchestrator::postprocess`]
///
/// Depending on [`AnalyzerType`], different passes will be registered and run on the
/// [`AnalyzedFunction`]
pub(crate) struct PostprocessOrchestrator {
    /// The type of postprocessor to use. this is taken from the analyzer
    typ: AnalyzerType,
    /// Structured passes run before lowering to source text.
    ir_passes: Vec<IrPostprocessor>,
    /// Function-wide structured passes run after statement-local passes.
    ir_function_passes: Vec<IrFunctionPostprocessor>,
    /// The state shared between postprocessors
    state: PostprocessorState,
}

impl PostprocessOrchestrator {
    /// Build a new postprocessor with the given analyzer type
    pub(crate) fn new(typ: AnalyzerType) -> Result<Self, Error> {
        let mut orchestrator = Self {
            typ,
            ir_passes: Vec::new(),
            ir_function_passes: Vec::new(),
            state: PostprocessorState::default(),
        };
        orchestrator.register_passes()?;
        Ok(orchestrator)
    }

    /// Register passes for the given analyzer type
    pub(crate) fn register_passes(&mut self) -> Result<(), Error> {
        match self.typ {
            AnalyzerType::Solidity => {
                self.ir_passes.push(bitwise_mask_postprocessor);
                self.ir_passes.push(arithmetic_postprocessor);
                self.ir_passes.push(memory_postprocessor);
                self.ir_passes.push(storage_postprocessor);
                self.ir_passes.push(transient_postprocessor);
                self.ir_passes.push(variable_postprocessor);
                self.ir_passes.push(type_cleanup_postprocessor);

                self.ir_function_passes.push(normalize_typed_returns);
                self.ir_function_passes.push(inline_single_use_variables);
                self.ir_function_passes.push(eliminate_dead_variables);
                self.ir_function_passes.push(structure_control_flow);
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
            storage_roots: self.state.storage_roots.clone(),
            storage_type_hints: self.state.storage_type_hints.clone(),
            storage_root_slots: self.state.storage_root_slots.clone(),
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

        // Storage inference must run before getter detection and before memory accesses are
        // renamed.
        if self.typ == AnalyzerType::Solidity {
            for statement in &mut function.statements {
                storage_inference_postprocessor(statement, &mut state)?;
            }
        }

        // A direct storage return is a getter even when mapping/array keys are function arguments.
        if function.view {
            if let Some(root) = function.statements.iter().find_map(|statement| match statement {
                Statement::Return(Expr::StorageAccess(path)) => Some(path.root().clone()),
                _ => None,
            }) {
                state.maybe_getter_for = Some(root);
            }
        }

        if let Some(returns) = function.returns.as_deref() {
            let hint = if returns.starts_with("string") {
                Some("string")
            } else if returns.starts_with("bytes") && returns != "bytes32" {
                Some("bytes")
            } else {
                None
            };
            if let Some(hint) = hint {
                for statement in &function.statements {
                    let mut statement = statement.clone();
                    statement.visit_exprs_mut(&mut |expr| {
                        if let Expr::StorageAccess(path) = expr {
                            state.storage_type_hints.insert(path.root().clone(), hint.to_string());
                        }
                    });
                }
            }
        }

        detect_string_storage_getter(function, &mut state)?;

        // Detect simple getters and Solidity's RLP-backed string pattern directly on the IR.
        if !function.payable && (function.pure || function.view) && function.arguments.is_empty() {
            let returned_storage = function.statements.iter().find_map(|statement| {
                let Statement::Return(value) = statement else { return None };
                find_expression(&[Statement::Return(value.clone())], is_storage_access)
            });

            if let Some(storage) = returned_storage {
                if let Expr::StorageAccess(path) = &storage {
                    state.maybe_getter_for = Some(path.root().clone());
                }

                if has_binary_literal(&function.statements, BinaryOp::Mul, U256::from(0x100)) &&
                    (has_binary_literal(&function.statements, BinaryOp::BitAnd, U256::from(1)) ||
                        find_expression(&function.statements, |expr| {
                            matches!(expr, Expr::StorageAccess(path) if path_has_packed_width(path, 8))
                        })
                        .is_some())
                {
                    function.returns = Some(String::from("string memory"));
                    function.statements = vec![Statement::Return(Expr::Call {
                        callee: "string".to_string(),
                        args: vec![Expr::Call {
                            callee: "rlp.encodePacked".to_string(),
                            args: vec![storage],
                        }],
                    })];
                }
            }
        }

        // Transform structured statements before lowering to source text.
        for pass in &self.ir_passes {
            for statement in &mut function.statements {
                pass(statement, &mut state)?;
            }
        }

        for pass in &self.ir_function_passes {
            pass(function, &mut state)?;
        }

        function.render_statements();

        // wherever storage_map contains a value that doesnt exist in storage_type_map, add it with
        // a default value
        state.storage_map.iter().for_each(|(_, value)| {
            let rendered = value.render();
            let storage_var_name = rendered.split('[').next().unwrap_or(&rendered);
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
        state.transient_map.iter().for_each(|(_, value)| {
            let rendered = value.render();
            let storage_var_name = rendered.split('[').next().unwrap_or(&rendered);
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
        if let Some(getter_for) = self.state.maybe_getter_for.as_ref() {
            function.maybe_getter_for = self
                .state
                .storage_root_slots
                .iter()
                .find_map(|(name, slot)| (slot == getter_for).then(|| name.clone()));
        }

        debug!(
            "postprocessing for '{}' completed in {:?}",
            function.selector,
            start_postprocess_time.elapsed()
        );

        Ok(self.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ir::StoragePath;

    #[test]
    fn links_string_getter_to_canonical_storage_root() {
        let root = Expr::Literal(U256::from(2));
        let mut function = AnalyzedFunction::new("06fdde03", false);
        function.analyzer_type = AnalyzerType::Solidity;
        function.view = true;
        function.payable = false;
        function.returns = Some("string memory".to_string());
        function.statements = vec![
            Statement::Expression(Expr::StorageAccess(Box::new(StoragePath::PackedField {
                parent: Box::new(StoragePath::Slot { slot: Box::new(root.clone()) }),
                bit_offset: 0,
                bit_width: 8,
            }))),
            Statement::Return(Expr::StorageAccess(Box::new(StoragePath::DynamicArray {
                parent: Box::new(StoragePath::Slot { slot: Box::new(root) }),
                index: Box::new(Expr::identifier("index")),
            }))),
        ];
        let mut orchestrator = PostprocessOrchestrator::new(AnalyzerType::Solidity).unwrap();
        orchestrator.postprocess(&mut function).unwrap();
        assert_eq!(function.maybe_getter_for.as_deref(), Some("store_a"));
    }

    #[test]
    fn recovers_mapping_through_full_pipeline() {
        let mut function = AnalyzedFunction::new("00000000", false);
        function.analyzer_type = AnalyzerType::Solidity;
        function.statements = vec![
            Statement::Assign {
                target: Expr::index("memory", Expr::Literal(U256::ZERO)),
                value: Expr::identifier("msg.sender"),
            },
            Statement::Assign {
                target: Expr::index("memory", Expr::Literal(U256::from(32))),
                value: Expr::Literal(U256::from(5)),
            },
            Statement::Return(Expr::StorageAccess(Box::new(StoragePath::Slot {
                slot: Box::new(Expr::Keccak {
                    offset: Box::new(Expr::Literal(U256::ZERO)),
                    size: Box::new(Expr::Literal(U256::from(64))),
                    preimage: None,
                }),
            }))),
        ];

        let mut orchestrator = PostprocessOrchestrator::new(AnalyzerType::Solidity).unwrap();
        let state = orchestrator.postprocess(&mut function).unwrap();
        assert_eq!(function.logic, vec!["return storage_map_a[msg.sender];"]);
        assert_eq!(
            state.storage_type_map.get("storage_map_a"),
            Some(&"mapping(address => bytes32)".to_string())
        );
    }
}
