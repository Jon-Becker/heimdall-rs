use hashbrown::HashMap;
use std::time::Instant;

use alloy::primitives::U256;
use tracing::debug;

use crate::{
    interfaces::AnalyzedFunction,
    utils::postprocessors::{
        arithmetic_postprocessor, bitwise_mask_postprocessor, eliminate_dead_variables,
        memory_postprocessor, storage_inference_postprocessor, storage_postprocessor,
        transient_postprocessor, variable_postprocessor, IrFunctionPostprocessor, IrPostprocessor,
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
                self.ir_passes.push(storage_inference_postprocessor);
                self.ir_passes.push(bitwise_mask_postprocessor);
                self.ir_passes.push(arithmetic_postprocessor);
                self.ir_passes.push(memory_postprocessor);
                self.ir_passes.push(storage_postprocessor);
                self.ir_passes.push(transient_postprocessor);
                self.ir_passes.push(variable_postprocessor);

                self.ir_function_passes.push(eliminate_dead_variables);
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

        // Detect simple getters and Solidity's RLP-backed string pattern directly on the IR.
        if !function.payable && (function.pure || function.view) && function.arguments.is_empty() {
            let returned_storage = function.statements.iter().find_map(|statement| {
                let Statement::Return(value) = statement else { return None };
                find_expression(&[Statement::Return(value.clone())], is_storage_access)
            });

            if let Some(storage) = returned_storage {
                state.maybe_getter_for = Some(storage.clone());

                if has_binary_literal(&function.statements, BinaryOp::Mul, U256::from(0x100)) &&
                    has_binary_literal(&function.statements, BinaryOp::BitAnd, U256::from(1))
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
            function.maybe_getter_for = self.state.storage_map.get(getter_for).map(Expr::render);
        }

        debug!(
            "postprocessing for '{}' completed in {:?}",
            function.selector,
            start_postprocess_time.elapsed()
        );

        Ok(self.state.clone())
    }
}
