pub(crate) mod graph;

use eyre::eyre;
use heimdall_common::{ether::compiler::detect_compiler, utils::strings::StringExt};
use heimdall_vm::core::vm::Vm;

use petgraph::{dot::Dot, Graph};
use std::time::{Duration, Instant};

use super::CfgArgs;

use crate::{core::graph::build_cfg, error::Error};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct CfgResult {
    pub graph: Graph<String, String>,
}

impl CfgResult {
    pub fn as_dot(&self, color_edges: bool) -> String {
        let output = format!("{}", Dot::with_config(&self.graph, &[]));

        // find regex matches and replace
        let mut output = output.replace(
            "digraph {",
            "digraph G {\n    node [shape=box, style=\"rounded\", fontname=\"Helvetica\"];\n    edge [fontname=\"Helvetica\"];"
        );

        if color_edges {
            // replace edge labels with colors
            output = output.replace("[ label = \"true\" ]", "[ color = \"green\" ]");
            output = output.replace("[ label = \"false\" ]", "[ color = \"red\" ]");
        } else {
            // remove edge labels
            output = output.replace("[ label = \"true\" ]", "[]");
            output = output.replace("[ label = \"false\" ]", "[]");
        }

        output = output.replace("[ label = \"\" ]", "[]");

        output
    }
}

pub async fn cfg(args: CfgArgs) -> Result<CfgResult, Error> {
    // init
    let start_time = Instant::now();

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let contract_bytecode = args
        .get_bytecode()
        .await
        .map_err(|e| Error::FetchError(format!("fetching target bytecode failed: {}", e)))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    if contract_bytecode.is_empty() {
        return Err(Error::Eyre(eyre!("contract bytecode is empty")));
    }

    // perform versioning and compiler heuristics
    let (_compiler, _version) = detect_compiler(&contract_bytecode);

    // create a new EVM instance. we will use this for finding function selectors,
    // performing symbolic execution, and more.
    let mut evm = Vm::new_with_bytecode(&contract_bytecode);

    info!("performing symbolic execution on '{}'", args.target.truncate(64));
    let start_sym_exec_time = Instant::now();
    let (map, jumpdest_count) = evm
        .symbolic_exec(
            Instant::now()
                .checked_add(Duration::from_millis(args.timeout))
                .expect("invalid timeout"),
        )
        .map_err(|e| Error::Eyre(eyre!("symbolic execution failed: {}", e)))?;

    debug!("'{}' has {} unique branches", args.target.truncate(64), jumpdest_count);
    debug!("symbolic execution took {:?}", start_sym_exec_time.elapsed());
    info!("symbolically executed '{}'", args.target.truncate(64));

    // run cfg generation
    let start_cfg_time = Instant::now();
    info!("building cfg for '{}' from symbolic execution trace", args.target.truncate(64));
    let mut contract_cfg = Graph::new();
    build_cfg(&map, &mut contract_cfg, None, false)?;
    debug!("building cfg took {:?}", start_cfg_time.elapsed());

    debug!("cfg generated in {:?}", start_time.elapsed());
    info!("generated cfg successfully");

    Ok(CfgResult { graph: contract_cfg })
}
