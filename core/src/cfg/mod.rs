pub mod graph;
pub mod output;
use derive_builder::Builder;
use heimdall_common::ether::{
    compiler::detect_compiler, rpc::get_code, selectors::find_function_selectors,
};
use indicatif::ProgressBar;
use std::{fs, time::Duration};

use clap::{AppSettings, Parser};
use heimdall_common::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::evm::core::vm::VM,
    io::logging::*,
};
use petgraph::Graph;

use crate::{
    cfg::graph::build_cfg,
    disassemble::{disassemble, DisassemblerArgs},
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
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Specify a format (other than dot) to output the CFG in.
    /// For example, `--format svg` will output a SVG image of the CFG.
    #[clap(long = "format", short, default_value = "", hide_default_value = true)]
    pub format: String,

    /// Color the edges of the graph based on the JUMPI condition.
    /// This is useful for visualizing the flow of if statements.
    #[clap(long = "color-edges", short)]
    pub color_edges: bool,
}

impl CFGArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
            format: Some(String::new()),
            color_edges: Some(false),
        }
    }
}

pub async fn cfg(args: CFGArgs) -> Result<Graph<String, String>, Box<dyn std::error::Error>> {
    use std::time::Instant;
    let now = Instant::now();

    // set logger environment variable if not already set
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            match args.verbose.log_level() {
                Some(level) => level.as_str(),
                None => "SILENT",
            },
        );
    }

    let (logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the call to the trace
    let cfg_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "cfg".to_string(),
        vec![shortened_target],
        "()".to_string(),
    );

    // fetch bytecode
    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {
        // We are working with a contract address, so we need to fetch the bytecode from the RPC
        // provider
        contract_bytecode = get_code(&args.target, &args.rpc_url).await?;
    } else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        logger.debug_max("using provided bytecode for cfg generation");
        contract_bytecode = args.target.replacen("0x", "", 1);
    } else {
        logger.debug_max("using provided file for cfg generation.");

        // We are analyzing a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {
                let _contents = contents.replace('\n', "");
                if BYTECODE_REGEX.is_match(&_contents).unwrap() && _contents.len() % 2 == 0 {
                    _contents.replacen("0x", "", 1)
                } else {
                    logger
                        .error(&format!("file '{}' doesn't contain valid bytecode.", &args.target));
                    std::process::exit(1)
                }
            }
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &args.target));
                std::process::exit(1)
            }
        };
    }

    // disassemble the bytecode
    let disassembled_bytecode = disassemble(DisassemblerArgs {
        target: contract_bytecode.clone(),
        verbose: args.verbose.clone(),
        rpc_url: args.rpc_url.clone(),
        decimal_counter: false,
    })
    .await?;

    // add the call to the trace
    trace.add_call(
        cfg_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(&contract_bytecode);
    trace.add_call(
        cfg_call,
        line!(),
        "heimdall".to_string(),
        "detect_compiler".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        format!("({compiler}, {version})"),
    );

    if compiler == "solc" {
        logger.debug(&format!("detected compiler {compiler} {version}."));
    } else {
        logger
            .warn(&format!("detected compiler {compiler} {version} is not supported by heimdall."));
    }

    // create a new EVM instance
    let evm = VM::new(
        contract_bytecode.clone(),
        String::from("0x"),
        String::from("0x6865696d64616c6c000000000061646472657373"),
        String::from("0x6865696d64616c6c0000000000006f726967696e"),
        String::from("0x6865696d64616c6c00000000000063616c6c6572"),
        0,
        u128::max_value(),
    );
    let mut shortened_target = contract_bytecode.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the creation to the trace
    let vm_trace = trace.add_creation(
        cfg_call,
        line!(),
        "contract".to_string(),
        shortened_target.clone(),
        (contract_bytecode.len() / 2usize).try_into().unwrap(),
    );

    // find all selectors in the bytecode
    let selectors = find_function_selectors(&evm, &disassembled_bytecode);
    logger.info(&format!("found {} possible function selectors.", selectors.len()));
    logger.info(&format!("performing symbolic execution on '{}' .", &shortened_target));

    // create a new progress bar
    let progress = ProgressBar::new_spinner();
    progress.enable_steady_tick(Duration::from_millis(100));
    progress.set_style(logger.info_spinner());

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
    let (map, jumpdest_count) = &evm.symbolic_exec();

    // add jumpdests to the trace
    trace.add_info(
        map_trace,
        line!(),
        &format!("traced and executed {jumpdest_count} possible paths."),
    );

    logger.debug_max("building control flow graph from symbolic execution trace");
    build_cfg(map, &mut contract_cfg, None, false);

    progress.finish_and_clear();
    logger.info("symbolic execution completed.");
    logger.debug(&format!("Control flow graph generated in {:?}.", now.elapsed()));
    trace.display();

    Ok(contract_cfg)
}
