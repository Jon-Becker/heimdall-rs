use alloy::primitives::U256;
use eyre::{OptionExt, Result};
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::ext::exec::VMTrace;
use petgraph::{matrix_graph::NodeIndex, Graph};

/// convert a symbolic execution [`VMTrace`] into a [`Graph`] of blocks, illustrating the
/// control-flow graph found by the symbolic execution engine.
// TODO: should this be a trait for VMTrace to implement?
pub fn build_cfg(
    vm_trace: &VMTrace,
    contract_cfg: &mut Graph<String, String>,
    parent_node: Option<NodeIndex<u32>>,
    jump_taken: bool,
) -> Result<()> {
    let mut cfg_node: String = String::new();
    let mut parent_node = parent_node;

    // add the current operations to the cfg
    for operation in &vm_trace.operations {
        let opcode_name = operation
            .last_instruction
            .opcode_details
            .as_ref()
            .ok_or_eyre("failed to get opcode details for instruction")?
            .name;

        let assembly = format!(
            "{} {} {}",
            encode_hex_reduced(U256::from(operation.last_instruction.instruction)),
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
                .opcode_details
                .as_ref()
                .ok_or_eyre("failed to get opcode details")?
                .name ==
                "JUMPDEST",
        )?;
    }

    Ok(())
}
