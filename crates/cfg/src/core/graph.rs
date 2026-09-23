use alloy::primitives::U256;
use eyre::{OptionExt, Result};
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::{
    core::opcodes::{opcode_name, JUMPDEST},
    ext::exec::VMTrace,
};
use petgraph::{matrix_graph::NodeIndex, Graph};
use std::collections::HashMap;

/// convert a symbolic execution [`VMTrace`] into a [`Graph`] of blocks, illustrating the
/// control-flow graph found by the symbolic execution engine.
pub(crate) fn build_cfg(
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
        build_cfg(
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
    use tokio::test;

    #[test]
    async fn test_build_cfg() -> Result<(), Box<dyn std::error::Error>> {
        let args = CfgArgsBuilder::new()
            .target(include_str!("../../../core/tests/testdata/cfg/build_cfg.hex").to_string())
            .build()?;

        let result = cfg(args).await?;

        println!("Contract Cfg: {:#?}", result);

        Ok(())
    }
}
