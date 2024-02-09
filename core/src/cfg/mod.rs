pub mod graph;
pub mod output;
use derive_builder::Builder;
use ethers::types::H160;
use heimdall_common::{
    debug, debug_max,
    ether::{
        bytecode::get_bytecode_from_target,
        compiler::{detect_compiler, Compiler},
        selectors::find_function_selectors,
    },
    info, info_spinner,
    utils::{
        strings::{encode_hex, StringExt},
        threading::run_with_timeout,
    },
    warn,
};
use heimdall_config::parse_url_arg;
use indicatif::ProgressBar;
use std::time::Duration;

use clap::{AppSettings, Parser};
use heimdall_common::{ether::evm::core::vm::VM, utils::io::logging::*};
use petgraph::Graph;

use crate::{
    cfg::graph::build_cfg,
    disassemble::{disassemble, DisassemblerArgs},
    error::Error,
};

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Generate a visual control flow graph for EVM bytecode",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall cfg <TARGET> [OPTIONS]"
)]
pub struct CFGArgs {
    /// The target to generate a CFG for, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Color the edges of the graph based on the JUMPI condition.
    /// This is useful for visualizing the flow of if statements.
    #[clap(long = "color-edges", short)]
    pub color_edges: bool,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The name for the output file
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// Timeout for symbolic execution
    #[clap(long, short, default_value = "10000", hide_default_value = true)]
    pub timeout: u64,
}

impl CFGArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
            color_edges: Some(false),
            output: Some(String::new()),
            name: Some(String::new()),
            timeout: Some(10000),
        }
    }
}

/// The main entry point for the CFG module. Will generate a control flow graph of the target
/// bytecode, after performing symbolic execution and discovering all possible execution paths.
pub async fn cfg(args: CFGArgs) -> Result<Graph<String, String>, Error> {
    use std::time::Instant;
    let now = Instant::now();

    set_logger_env(&args.verbose);
    let mut trace = TraceFactory::default();

    // add the call to the trace
    let cfg_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "cfg".to_string(),
        vec![args.target.truncate(64)],
        "()".to_string(),
    );

    let contract_bytecode = get_bytecode_from_target(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::Generic(format!("failed to get bytecode from target: {}", e)))?;

    // disassemble the bytecode
    let disassembled_bytecode = disassemble(DisassemblerArgs {
        target: encode_hex(contract_bytecode.clone()),
        verbose: args.verbose.clone(),
        rpc_url: args.rpc_url.clone(),
        decimal_counter: false,
        name: String::from(""),
        output: String::from(""),
    })
    .await?;

    // add the call to the trace
    trace.add_call(
        cfg_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len())],
        "()".to_string(),
    );

    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(&contract_bytecode);
    trace.add_call(
        cfg_call,
        line!(),
        "heimdall".to_string(),
        "detect_compiler".to_string(),
        vec![format!("{} bytes", contract_bytecode.len())],
        format!("({compiler}, {version})"),
    );

    if compiler == Compiler::Solc {
        debug!("detected compiler {} {}", compiler, version);
    } else {
        warn!("detected compiler {} {} is not supported by heimdall", compiler, version);
    }

    // create a new EVM instance
    let evm = VM::new(
        &contract_bytecode,
        &[],
        H160::default(),
        H160::default(),
        H160::default(),
        0,
        u128::max_value(),
    );

    // add the creation to the trace
    let vm_trace = trace.add_creation(
        cfg_call,
        line!(),
        "contract".to_string(),
        encode_hex(contract_bytecode.clone()).truncate(64),
        contract_bytecode
            .len()
            .try_into()
            .map_err(|_| Error::ParseError("failed to parse bytecode length".to_string()))?,
    );

    // find all selectors in the bytecode
    let selectors = find_function_selectors(&evm, &disassembled_bytecode);
    info!("found {} possible function selectors.", selectors.len());
    info!("performing symbolic execution on '{}' .", args.target.truncate(64));

    // create a new progress bar
    let progress = ProgressBar::new_spinner();
    progress.enable_steady_tick(Duration::from_millis(100));
    progress.set_style(info_spinner!());

    // create a new petgraph StableGraph
    let mut contract_cfg = Graph::<String, String>::new();

    // add the call to the trace
    let map_trace = trace.add_call(
        vm_trace,
        line!(),
        "heimdall".to_string(),
        "cfg".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

    // get a map of possible jump destinations
    let (map, jumpdest_count) =
        match run_with_timeout(move || evm.symbolic_exec(), Duration::from_millis(args.timeout)) {
            Some(map) => map.map_err(|e| {
                Error::Generic(format!("failed to perform symbolic execution: {}", e))
            })?,
            None => {
                return Err(Error::Generic("symbolic execution timed out".to_string()));
            }
        };

    // add jumpdests to the trace
    trace.add_info(
        map_trace,
        line!(),
        &format!("traced and executed {jumpdest_count} possible paths."),
    );

    debug_max!("building control flow graph from symbolic execution trace");
    build_cfg(&map, &mut contract_cfg, None, false)?;

    progress.finish_and_clear();
    info!("symbolic execution completed.");
    debug!(&format!("control flow graph generated in {:?}.", now.elapsed()));
    trace.display();

    Ok(contract_cfg)
}
