use std::{collections::HashMap, sync::Mutex};

use ethers::prelude::U256;
use heimdall_common::utils::strings::encode_hex_reduced;
use petgraph::{matrix_graph::NodeIndex, Graph};

use super::util::*;

use lazy_static::lazy_static;
lazy_static! {
    static ref INSTRUCTION_NODE_MAP: Mutex<HashMap<u128, NodeIndex<u32>>> =
        Mutex::new(HashMap::new());
    static ref CONNECTING_EDGES: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

impl VMTrace {
    // converts a VMTrace to a CFG graph
    pub fn build_cfg(
        &self,
        contract_cfg: &mut Graph<String, String>,
        parent_node: Option<NodeIndex<u32>>,
        jump_taken: bool,
    ) {
        let mut cfg_node: String = String::new();
        let mut parent_node = parent_node;

        // add the current operations to the cfg
        for operation in &self.operations {
            let instruction = operation.last_instruction.clone();

            let opcode_name = instruction.opcode_details.clone().unwrap().name;

            let assembly = format!(
                "{} {} {}",
                encode_hex_reduced(U256::from(instruction.instruction)),
                opcode_name,
                if opcode_name.contains("PUSH") {
                    encode_hex_reduced(*instruction.outputs.clone().first().unwrap())
                } else {
                    String::from("")
                }
            );

            cfg_node.push_str(&format!("{}\n", &assembly));
        }

        // check if the map already contains the current node
        let mut instruction_node_map = INSTRUCTION_NODE_MAP.lock().unwrap();
        let chunk_index = match self.operations.first() {
            Some(operation) => operation.last_instruction.instruction,
            None => 0,
        };

        match instruction_node_map.get(&chunk_index) {
            Some(node_index) => {
                // this node already exists, so we need to add an edge to it.
                if let Some(parent_node) = parent_node {
                    // check if the edge already exists
                    let mut connecting_edges = CONNECTING_EDGES.lock().unwrap();
                    let edge = format!("{} -> {}", parent_node.index(), node_index.index());
                    if !connecting_edges.contains(&edge) {
                        contract_cfg.add_edge(parent_node, *node_index, jump_taken.to_string());
                        connecting_edges.push(edge);
                    }
                    drop(connecting_edges)
                }
            }
            None => {
                // this node does not exist, so we need to add it to the map and the graph
                let node_index = contract_cfg.add_node(cfg_node);

                if let Some(parent_node) = parent_node {
                    // check if the edge already exists
                    let mut connecting_edges = CONNECTING_EDGES.lock().unwrap();
                    let edge = format!("{} -> {}", parent_node.index(), node_index.index());
                    if !connecting_edges.contains(&edge) {
                        contract_cfg.add_edge(parent_node, node_index, jump_taken.to_string());
                        connecting_edges.push(edge);
                    }
                    drop(connecting_edges)
                }

                instruction_node_map.insert(chunk_index, node_index);
                parent_node = Some(node_index);
            }
        };

        drop(instruction_node_map);

        // recurse into the children of the VMTrace map
        for child in self.children.iter() {
            child.build_cfg(
                contract_cfg,
                parent_node,
                child
                    .operations
                    .first()
                    .unwrap()
                    .last_instruction
                    .opcode_details
                    .clone()
                    .unwrap()
                    .name ==
                    "JUMPDEST",
            );
        }
    }
}
