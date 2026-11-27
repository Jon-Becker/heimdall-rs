use alloy::primitives::U256;
use eyre::{OptionExt, Result};
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::{
    core::opcodes::opcode_name,
    ext::exec::{TraceDetails, TraceEnd, VMTrace},
};
use petgraph::{matrix_graph::NodeIndex, Graph};
use std::collections::HashMap;

/// convert a symbolic execution [`VMTrace`] into a [`Graph`] of blocks, illustrating the
/// control-flow graph found by the symbolic execution engine.
pub(crate) fn build_cfg(
    vm_trace: &VMTrace,
    details: &TraceDetails,
    contract_cfg: &mut Graph<String, String>,
    parent_node: Option<NodeIndex<u32>>,
    jump_taken: bool,
    seen_nodes: &mut HashMap<String, NodeIndex<u32>>,
) -> Result<()> {
    let mut cfg_node: String = String::new();
    let mut parent_node = parent_node;

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
            add_branch_edge(contract_cfg, parent_node, node_index, jump_taken);
        }
        return Ok(());
    }

    let node_index = contract_cfg.add_node(cfg_node.clone());
    seen_nodes.insert(cfg_node, node_index);
    if let Some(parent_node) = parent_node {
        add_branch_edge(contract_cfg, parent_node, node_index, jump_taken);
    }
    parent_node = Some(node_index);

    for branch in &details.branches {
        if let Some(index) = branch.child_index {
            let child = &vm_trace.children[index];
            build_cfg(child, &branch.details, contract_cfg, parent_node, branch.taken, seen_nodes)?;
        } else if branch.details.end == TraceEnd::InvalidDestination {
            let label = "OBSERVED INVALID JUMP DESTINATION".to_string();
            let target =
                *seen_nodes.entry(label.clone()).or_insert_with(|| contract_cfg.add_node(label));
            add_branch_edge(contract_cfg, node_index, target, branch.taken);
        }
    }

    Ok(())
}

fn add_branch_edge(
    graph: &mut Graph<String, String>,
    from: NodeIndex<u32>,
    to: NodeIndex<u32>,
    taken: bool,
) {
    let label = taken.to_string();
    if !graph.edges_connecting(from, to).any(|edge| edge.weight() == &label) {
        graph.add_edge(from, to, label);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cfg, CfgArgsBuilder};
    use petgraph::visit::EdgeRef;
    use tokio::test;

    #[test]
    async fn test_build_cfg() -> Result<(), Box<dyn std::error::Error>> {
        let args = CfgArgsBuilder::new()
            .target(include_str!("../../../core/tests/testdata/cfg/build_cfg.hex").to_string())
            .build()?;

        let result = cfg(args).await?;

        assert!(result.graph.node_weights().any(|block| block.contains("CALLDATASIZE")));
        assert!(result.graph.node_weights().any(|block| block.contains("JUMPDEST")));
        assert!(result.graph.edge_count() > 0);

        Ok(())
    }

    #[test]
    async fn labels_fallthrough_jumpdest_as_false() {
        let args = CfgArgsBuilder::new().target("60016007575b005b00".to_string()).build().unwrap();
        let graph = cfg(args).await.unwrap().graph;
        let mut labels: Vec<_> = graph.edge_weights().cloned().collect();
        labels.sort();
        assert_eq!(labels, vec!["false", "true"]);
    }

    #[test]
    async fn keeps_both_outcomes_when_they_reach_the_same_block() {
        let args = CfgArgsBuilder::new().target("60016005575b00".to_string()).build().unwrap();
        let graph = cfg(args).await.unwrap().graph;
        let mut labels: Vec<_> = graph.edge_weights().cloned().collect();
        labels.sort();
        assert_eq!(labels, vec!["false", "true"]);
    }

    #[test]
    async fn invalid_taken_destination_is_an_exceptional_edge() {
        let args = CfgArgsBuilder::new().target("3460045700".to_string()).build().unwrap();
        let graph = cfg(args).await.unwrap().graph;
        let root = petgraph::graph::NodeIndex::new(0);
        let target = graph.edges(root).find(|edge| edge.weight() == "true").unwrap().target();
        assert_eq!(graph[target], "OBSERVED INVALID JUMP DESTINATION");
        assert!(graph.edges(root).any(|edge| edge.weight() == "false"));
    }
}
