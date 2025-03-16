use alloy::primitives::U256;
use eyre::{OptionExt, Result};
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::{
    core::opcodes::{opcode_name, JUMPDEST},
    ext::exec::VMTrace,
};
use petgraph::{matrix_graph::NodeIndex, Graph};
use std::collections::HashSet;

/// convert a symbolic execution [`VMTrace`] into a [`Graph`] of blocks, illustrating the
/// control-flow graph found by the symbolic execution engine.
pub(crate) fn build_cfg(
    vm_trace: &VMTrace,
    contract_cfg: &mut Graph<String, String>,
    parent_node: Option<NodeIndex<u32>>,
    jump_taken: bool,
    seen_nodes: &mut HashSet<String>,
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

    // check if this node has been seen before
    if seen_nodes.contains(&cfg_node) {
        return Ok(());
    }
    seen_nodes.insert(cfg_node.clone());

    // add the node to the graph
    let node_index = contract_cfg.add_node(cfg_node);
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
            .target("0x6080604052348015600e575f80fd5b50600436106030575f3560e01c80632125b65b146034578063b69ef8a8146044575b5f80fd5b6044603f3660046046565b505050565b005b5f805f606084860312156057575f80fd5b833563ffffffff811681146069575f80fd5b925060208401356001600160a01b03811681146083575f80fd5b915060408401356001600160e01b0381168114609d575f80fd5b80915050925092509256".to_string())
            .build()?;

        let result = cfg(args).await?;

        println!("Contract Cfg: {:#?}", result);

        Ok(())
    }
}
