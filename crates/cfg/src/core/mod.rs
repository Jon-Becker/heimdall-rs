pub(crate) mod graph;

use alloy::primitives::Address;
use eyre::eyre;
use heimdall_common::utils::strings::StringExt;
use heimdall_vm::core::vm::VM;
use std::collections::HashMap;

use petgraph::{
    dot::{Config, Dot},
    Graph,
};
use std::time::{Duration, Instant};

use super::CfgArgs;

use crate::{core::graph::build_cfg, error::Error};
use tracing::{debug, info};

/// The result of the cfg command. Contains the generated control flow graph.
#[derive(Debug, Clone)]
pub struct CfgResult {
    /// The generated control flow graph of the contract.
    pub graph: Graph<String, String>,
}

impl CfgResult {
    /// Returns the control flow graph as a graphviz formatted string.
    pub fn as_dot(&self, color_edges: bool) -> String {
        let edge_attributes =
            |_: &Graph<String, String>, edge: petgraph::graph::EdgeReference<'_, String>| match edge
                .weight()
                .as_str()
            {
                "true" if color_edges => "color = \"green\"".to_string(),
                "false" if color_edges => "color = \"red\"".to_string(),
                "true" | "false" | "" => String::new(),
                label => format!("label = {label:?}"),
            };
        let node_attributes = |_: &Graph<String, String>, _| String::new();
        let content = format!(
            "{}",
            Dot::with_attr_getters(
                &self.graph,
                &[Config::GraphContentOnly, Config::EdgeNoLabel],
                &edge_attributes,
                &node_attributes,
            )
        );
        format!(
            "digraph G {{\n    node [shape=box, style=\"rounded\", fontname=\"Helvetica\"];\n    edge [fontname=\"Helvetica\"];\n{content}}}\n"
        )
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
        return Err(Error::Eyre(eyre!("contract bytecode is empty")));
    }

    let mut evm = VM::new(
        &contract_bytecode,
        &[],
        Address::default(),
        Address::default(),
        Address::default(),
        0,
        u128::MAX,
    )
    .with_hardfork(hardfork);

    info!("performing symbolic execution on '{}'", args.target.truncate(64));
    let start_sym_exec_time = Instant::now();
    let (map, details, jumpdest_count) = evm
        .symbolic_exec_detailed(
            Instant::now()
                .checked_add(Duration::from_millis(args.timeout))
                .expect("invalid timeout"),
        )
        .map_err(|e| Error::Eyre(eyre!("symbolic execution failed: {}", e)))?;

    debug!("'{}' has {} unique branches", args.target.truncate(64), jumpdest_count);
    debug!("symbolic execution took {:?}", start_sym_exec_time.elapsed());
    info!("symbolically executed '{}'", args.target.truncate(64));

    let start_cfg_time = Instant::now();
    info!("building cfg for '{}' from symbolic execution trace", args.target.truncate(64));
    let mut contract_cfg = Graph::new();
    let mut seen_nodes = HashMap::new();
    build_cfg(&map, &details, &mut contract_cfg, None, false, &mut seen_nodes)?;
    debug!("building cfg took {:?}", start_cfg_time.elapsed());

    debug!("cfg generated in {:?}", start_time.elapsed());
    info!("generated cfg successfully");

    Ok(CfgResult { graph: contract_cfg })
}

#[cfg(test)]
mod rendering_tests {
    use super::*;

    #[test]
    fn dot_renders_edge_attributes_without_changing_node_labels() {
        let mut graph = Graph::new();
        let from = graph.add_node("block [ label = \"true\" ]".to_string());
        let to = graph.add_node("target".to_string());
        graph.add_edge(from, to, "true".to_string());
        graph.add_edge(to, from, "false".to_string());
        graph.add_edge(from, from, "custom".to_string());
        let result = CfgResult { graph };
        let plain = result.as_dot(false);
        let colored = result.as_dot(true);
        assert!(plain.contains("block [ label = \\\"true\\\" ]"));
        assert!(colored.contains("block [ label = \\\"true\\\" ]"));
        assert!(colored.contains("color = \"green\""));
        assert!(colored.contains("color = \"red\""));
        assert!(!plain.contains("color ="));
        assert!(plain.contains("label = \"custom\""));
        assert!(colored.contains("label = \"custom\""));
    }
}
