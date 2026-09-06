use alloy::primitives::U256;
use eyre::{OptionExt, Result};
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::{
    core::{
        context::{ContextualCfg, ContextualEdgeKind},
        opcodes::{opcode_name, JUMPDEST},
        program::{BlockId, Program},
    },
    ext::exec::VMTrace,
};
use petgraph::{matrix_graph::NodeIndex, Graph};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Convert canonical contextual analysis into a block-level control-flow graph.
///
/// Multiple calling contexts are intentionally projected onto one canonical basic-block node.
/// Context-distinct duplicate edges are collapsed while retaining distinct true/false edge kinds.
pub(crate) fn build_canonical_cfg(
    program: &Program,
    analysis: &ContextualCfg,
) -> Graph<String, String> {
    let reachable = analysis.entry_states.keys().map(|point| point.block).collect::<BTreeSet<_>>();
    let mut graph = Graph::new();
    let nodes = program
        .blocks
        .iter()
        .filter(|block| reachable.contains(&block.id))
        .map(|block| {
            let label = program
                .block_instructions(block.id)
                .iter()
                .map(format_instruction)
                .collect::<Vec<_>>()
                .join("\n") +
                "\n";
            (block.id, graph.add_node(label))
        })
        .collect::<BTreeMap<BlockId, _>>();

    let edges = analysis
        .edges
        .iter()
        .map(|edge| (edge.source.block, edge.target.block, edge.kind))
        .collect::<BTreeSet<_>>();
    for (source, target, kind) in edges {
        let (Some(&source), Some(&target)) = (nodes.get(&source), nodes.get(&target)) else {
            continue
        };
        let label = match kind {
            ContextualEdgeKind::ConditionalFalse => "false",
            ContextualEdgeKind::ConditionalTrue => "true",
            ContextualEdgeKind::Fallthrough | ContextualEdgeKind::Jump => "",
        };
        graph.add_edge(source, target, label.to_owned());
    }

    graph
}

fn format_instruction(instruction: &heimdall_vm::core::program::DecodedInstruction) -> String {
    let operand = instruction
        .push_value()
        .map(|value| format!(" {}", encode_hex_reduced(value)))
        .unwrap_or_default();
    format!(
        "{} {}{}",
        encode_hex_reduced(U256::from(instruction.pc)),
        opcode_name(instruction.opcode),
        operand
    )
}

/// Convert a legacy recursive symbolic execution [`VMTrace`] into a graph.
pub(crate) fn build_legacy_cfg(
    vm_trace: &VMTrace,
    contract_cfg: &mut Graph<String, String>,
    parent_node: Option<NodeIndex<u32>>,
    jump_taken: bool,
    seen_nodes: &mut HashMap<String, NodeIndex<u32>>,
) -> Result<()> {
    let mut cfg_node: String = String::new();
    let mut parent_node = parent_node;

    // add the current operations to the cfg
    for operation in &vm_trace.operations {
        let opcode_name = opcode_name(operation.last_instruction.opcode);

        let opcode_offset = operation.last_instruction.instruction - 1; // start from 0x00

        let assembly = format!(
            "{} {} {}",
            encode_hex_reduced(U256::from(opcode_offset)),
            opcode_name,
            if opcode_name.contains("PUSH") {
                encode_hex_reduced(
                    *operation
                        .last_instruction
                        .outputs
                        .first()
                        .ok_or_eyre("failed to get output for PUSH instruction")?,
                )
            } else {
                String::from("")
            }
        );

        cfg_node.push_str(&format!("{}\n", &assembly));
    }

    // if this node has been seen before, we still need to link the current parent to it,
    // otherwise edges into already-visited blocks (e.g. a shared fallback handler) are lost.
    // we don't recurse again, since the block's children have already been mapped.
    if let Some(&node_index) = seen_nodes.get(&cfg_node) {
        if let Some(parent_node) = parent_node {
            contract_cfg.update_edge(parent_node, node_index, jump_taken.to_string());
        }
        return Ok(());
    }

    // add the node to the graph
    let node_index = contract_cfg.add_node(cfg_node.clone());
    seen_nodes.insert(cfg_node, node_index);
    if let Some(parent_node) = parent_node {
        contract_cfg.update_edge(parent_node, node_index, jump_taken.to_string());
    }
    parent_node = Some(node_index);

    // recurse into the children of the VMTrace map
    for child in vm_trace.children.iter() {
        build_legacy_cfg(
            child,
            contract_cfg,
            parent_node,
            child
                .operations
                .first()
                .ok_or_eyre("failed to get first operation")?
                .last_instruction
                .opcode ==
                JUMPDEST,
            seen_nodes,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cfg, CfgArgsBuilder};
    use heimdall_vm::core::{context::analyze_contextual, hardfork::HardFork, opcodes};
    use tokio::test;

    #[test]
    async fn test_build_cfg() -> Result<(), Box<dyn std::error::Error>> {
        let args = CfgArgsBuilder::new()
            .target("0x6080604052348015600e575f80fd5b50600436106030575f3560e01c80632125b65b146034578063b69ef8a8146044575b5f80fd5b6044603f3660046046565b505050565b005b5f805f606084860312156057575f80fd5b833563ffffffff811681146069575f80fd5b925060208401356001600160a01b03811681146083575f80fd5b915060408401356001600160e01b0381168114609d575f80fd5b80915050925092509256".to_string())
            .build()?;

        let result = cfg(args).await?;

        assert!(result.diagnostics.canonical);
        assert_eq!(result.diagnostics.reachable_blocks, result.graph.node_count());
        assert_eq!(result.diagnostics.graph_edges, result.graph.edge_count());

        Ok(())
    }

    #[test]
    async fn cfg_surfaces_exhausted_analysis_budget() {
        let result = cfg(CfgArgsBuilder::new()
            .target("0x6000".to_owned())
            .max_iterations(0)
            .build()
            .expect("valid arguments"))
        .await
        .expect("bounded cfg");

        assert_eq!(result.diagnostics.analysis_iterations, 0);
        assert_eq!(result.diagnostics.budget_exhausted_points, 1);
    }

    #[test]
    async fn legacy_graph_remains_available_explicitly() {
        let result = cfg(CfgArgsBuilder::new()
            .target("0x60006000fd".to_owned())
            .legacy(true)
            .build()
            .expect("valid arguments"))
        .await
        .expect("legacy cfg");

        assert!(!result.diagnostics.canonical);
        assert_eq!(result.diagnostics.reachable_blocks, result.graph.node_count());
    }

    #[test]
    async fn canonical_graph_projects_contextual_edges_onto_blocks() {
        let program = Program::decode(
            &[
                opcodes::CALLVALUE,
                opcodes::PUSH1,
                6,
                opcodes::JUMPI,
                opcodes::STOP,
                opcodes::INVALID,
                opcodes::JUMPDEST,
                opcodes::STOP,
            ],
            HardFork::Latest,
        );
        let analysis = analyze_contextual(&program);
        let graph = build_canonical_cfg(&program, &analysis);
        let labels =
            graph.edge_references().map(|edge| edge.weight().as_str()).collect::<BTreeSet<_>>();

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(labels, BTreeSet::from(["false", "true"]));
        assert!(graph.node_weights().any(|label| label.starts_with("0x06 JUMPDEST")));
    }
}
