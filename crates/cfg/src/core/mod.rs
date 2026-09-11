pub(crate) mod graph;

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use alloy::primitives::Address;
use eyre::eyre;
use heimdall_common::{ether::compiler::detect_compiler, utils::strings::StringExt};
use heimdall_vm::core::{
    context::{analyze_contextual_with_config, ContextualAnalysisConfig},
    hardfork::HardFork,
    program::Program,
    vm::VM,
};
use petgraph::{dot::Dot, Graph};
use tracing::{debug, info, warn};

use super::CfgArgs;
use crate::{
    core::graph::{build_canonical_cfg, build_legacy_cfg},
    error::Error,
};

/// The result of the cfg command. Contains the generated control flow graph.
#[derive(Debug, Clone)]
pub struct CfgResult {
    /// The generated control flow graph of the contract.
    pub graph: Graph<String, String>,
    /// Analysis coverage and uncertainty surfaced by CFG construction.
    pub diagnostics: CfgDiagnostics,
}

/// Coverage and uncertainty metrics for a generated CFG.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct CfgDiagnostics {
    /// Whether the canonical abstract-analysis pipeline produced this graph.
    pub canonical: bool,
    /// Number of canonical blocks decoded from bytecode.
    pub decoded_blocks: usize,
    /// Number of canonical blocks reached by analysis.
    pub reachable_blocks: usize,
    /// Number of context-sensitive analysis points.
    pub contextual_states: usize,
    /// Number of distinct block-level graph edges.
    pub graph_edges: usize,
    /// Number of contextual jumps whose target remains unresolved.
    pub unresolved_jumps: usize,
    /// Number of contextual points with definite stack underflow.
    pub invalid_stack_points: usize,
    /// Number of contextual jumps with at least one invalid concrete target.
    pub invalid_jump_points: usize,
    /// Number of context destinations collapsed by configured analysis bounds.
    pub collapsed_contexts: usize,
    /// Number of conditional directions proven infeasible.
    pub pruned_branches: usize,
    /// Number of block-state executions completed by contextual analysis.
    pub analysis_iterations: usize,
    /// Number of queued points omitted when the analysis budget was exhausted.
    pub budget_exhausted_points: usize,
}

impl CfgResult {
    /// Returns the control flow graph as a graphviz formatted string.
    pub fn as_dot(&self, color_edges: bool) -> String {
        let output = format!("{}", Dot::with_config(&self.graph, &[]));

        let mut output = output.replace(
            "digraph {",
            "digraph G {\n    node [shape=box, style=\"rounded\", fontname=\"Helvetica\"];\n    edge [fontname=\"Helvetica\"];",
        );

        if color_edges {
            output = output.replace("[ label = \"true\" ]", "[ color = \"green\" ]");
            output = output.replace("[ label = \"false\" ]", "[ color = \"red\" ]");
        } else {
            output = output.replace("[ label = \"true\" ]", "[]");
            output = output.replace("[ label = \"false\" ]", "[]");
        }

        output = output.replace("[ label = \"\" ]", "[]");
        output
    }
}

/// Generates a control flow graph for the target contract.
pub async fn cfg(args: CfgArgs) -> Result<CfgResult, Error> {
    let start_time = Instant::now();
    let start_hardfork_resolve = Instant::now();
    let hardfork = args.get_hardfork().await;
    debug!("resolved hardfork: {} (took {:?})", hardfork, start_hardfork_resolve.elapsed());

    let start_fetch_time = Instant::now();
    let contract_bytecode = args
        .get_bytecode()
        .await
        .map_err(|e| Error::FetchError(format!("fetching target bytecode failed: {e}")))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    if contract_bytecode.is_empty() {
        return Err(Error::Eyre(eyre!("contract bytecode is empty")))
    }

    let (_compiler, _version) = detect_compiler(&contract_bytecode);
    let result = if args.legacy {
        build_legacy_result(&args, &contract_bytecode, hardfork)?
    } else {
        build_canonical_result(&args, &contract_bytecode, hardfork)
    };

    debug!("cfg generated in {:?}", start_time.elapsed());
    info!("generated cfg successfully");
    Ok(result)
}

fn build_canonical_result(
    args: &CfgArgs,
    contract_bytecode: &[u8],
    hardfork: HardFork,
) -> CfgResult {
    info!("building canonical cfg for '{}'", args.target.truncate(64));
    let start_analysis = Instant::now();
    let program = Program::decode(contract_bytecode, hardfork);
    let analysis = analyze_contextual_with_config(
        &program,
        ContextualAnalysisConfig { max_iterations: args.max_iterations, ..Default::default() },
    );
    let graph = build_canonical_cfg(&program, &analysis);
    let diagnostics = CfgDiagnostics {
        canonical: true,
        decoded_blocks: program.blocks.len(),
        reachable_blocks: graph.node_count(),
        contextual_states: analysis.entry_states.len(),
        graph_edges: graph.edge_count(),
        unresolved_jumps: analysis.unresolved_jumps.len(),
        invalid_stack_points: analysis.invalid_stack_points.len(),
        invalid_jump_points: analysis.invalid_jump_points.len(),
        collapsed_contexts: analysis.collapsed_contexts.len(),
        pruned_branches: analysis.pruned_branches.len(),
        analysis_iterations: analysis.analysis_iterations,
        budget_exhausted_points: analysis.budget_exhausted_points.len(),
    };
    if diagnostics.unresolved_jumps > 0 ||
        diagnostics.collapsed_contexts > 0 ||
        diagnostics.budget_exhausted_points > 0
    {
        warn!(
            unresolved_jumps = diagnostics.unresolved_jumps,
            collapsed_contexts = diagnostics.collapsed_contexts,
            budget_exhausted_points = diagnostics.budget_exhausted_points,
            "canonical cfg has bounded or unresolved analysis points"
        );
    }
    debug!("canonical analysis took {:?}", start_analysis.elapsed());
    CfgResult { graph, diagnostics }
}

fn build_legacy_result(
    args: &CfgArgs,
    contract_bytecode: &[u8],
    hardfork: HardFork,
) -> Result<CfgResult, Error> {
    let mut evm = VM::new(
        contract_bytecode,
        &[],
        Address::default(),
        Address::default(),
        Address::default(),
        0,
        u128::MAX,
    )
    .with_hardfork(hardfork);

    info!("performing legacy symbolic execution on '{}'", args.target.truncate(64));
    let (map, jumpdest_count) = evm
        .symbolic_exec(
            Instant::now()
                .checked_add(Duration::from_millis(args.timeout))
                .expect("invalid timeout"),
        )
        .map_err(|e| Error::Eyre(eyre!("symbolic execution failed: {}", e)))?;
    debug!("'{}' has {} legacy branches", args.target.truncate(64), jumpdest_count);

    let mut graph = Graph::new();
    let mut seen_nodes = HashMap::new();
    build_legacy_cfg(&map, &mut graph, None, false, &mut seen_nodes)?;
    let diagnostics = CfgDiagnostics {
        canonical: false,
        reachable_blocks: graph.node_count(),
        graph_edges: graph.edge_count(),
        ..CfgDiagnostics::default()
    };
    Ok(CfgResult { graph, diagnostics })
}
