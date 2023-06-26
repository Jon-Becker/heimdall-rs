pub mod graph;
pub mod output;
pub mod util;

use heimdall_common::ether::{
    compiler::detect_compiler, rpc::get_code, selectors::find_function_selectors,
};
use indicatif::ProgressBar;
use std::{env, fs, time::Duration};

use clap::{AppSettings, Parser};
use heimdall_common::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::evm::{
        disassemble::{disassemble, DisassemblerArgs},
        vm::VM,
    },
    io::logging::*,
};
use petgraph::Graph;

use crate::cfg::{output::build_output, util::map_contract};

#[derive(Debug, Clone, Parser)]
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

    /// The output directory to write the output to
    #[clap(long = "output", short, default_value = "", hide_default_value = true)]
    pub output: String,

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

pub fn cfg(args: CFGArgs) {
    use std::time::Instant;
    let now = Instant::now();

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

    // parse the output directory
    let mut output_dir: String;
    if args.output.is_empty() {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    } else {
        output_dir = args.output.clone();
    }

    // fetch bytecode
    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // We are working with a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        contract_bytecode = get_code(&args.target, &args.rpc_url, &logger);
    } else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target.replacen("0x", "", 1);
    } else {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str("/local");
        }

        // We are analyzing a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {
                if BYTECODE_REGEX.is_match(&contents).unwrap() && contents.len() % 2 == 0 {
                    contents.replacen("0x", "", 1)
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
        output: output_dir.clone(),
        rpc_url: args.rpc_url.clone(),
    });

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
    let (compiler, version) = detect_compiler(contract_bytecode.clone());
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
    let selectors = find_function_selectors(&evm, disassembled_bytecode);
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
    let (map, jumpdest_count) = map_contract(&evm);

    // add jumpdests to the trace
    trace.add_info(
        map_trace,
        line!(),
        format!("traced and executed {jumpdest_count} possible paths."),
    );

    map.build_cfg(&mut contract_cfg, None, false);

    progress.finish_and_clear();
    logger.info("symbolic execution completed.");

    // build the dot file
    build_output(&contract_cfg, &args, output_dir.clone(), &logger);

    logger.debug(&format!("Control flow graph generated in {:?}.", now.elapsed()));
    trace.display();
}

/// Builder pattern for using cfg genertion as a library.
///
/// Default values may be overriden individually.
/// ## Example
/// Use with normal settings:
/// ```no_run
/// # use crate::heimdall::cfg::CFGBuilder;
/// const SOURCE: &'static str = "7312/* snip */04ad";
///
/// CFGBuilder::new(SOURCE)
///     .generate();
/// ```
/// Or change settings individually:
/// ```no_run
/// # use crate::heimdall::cfg::CFGBuilder;
///
/// const SOURCE: &'static str = "7312/* snip */04ad";
/// CFGBuilder::new(SOURCE)
///     .default(false)
///     .output("my_contract_dir")
///     .rpc("https://127.0.0.1:8545")
///     .format("svg")
///     .verbosity(4)
///     .color_edges(true)
///     .generate();
/// ```
#[allow(dead_code)]
pub struct CFGBuilder {
    args: CFGArgs,
}

impl CFGBuilder {
    /// A new builder for the control flow graph generation of the specified target.
    ///
    /// The target may be a file, bytecode, contract address, or ENS name.
    #[allow(dead_code)]
    pub fn new(target: &str) -> Self {
        CFGBuilder {
            args: CFGArgs {
                target: target.to_string(),
                verbose: clap_verbosity_flag::Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from(""),
                format: String::from(""),
                color_edges: false,
                default: true,
            },
        }
    }

    /// Set the output verbosity level.
    ///
    /// - 0 Error
    /// - 1 Warn
    /// - 2 Info
    /// - 3 Debug
    /// - 4 Trace
    #[allow(dead_code)]
    pub fn verbosity(mut self, level: i8) -> CFGBuilder {
        // Calculated by the log library as: 1 + verbose - quiet.
        // Set quiet as 1, and the level corresponds to the appropriate Log level.
        self.args.verbose = clap_verbosity_flag::Verbosity::new(level, 0);
        self
    }

    /// The output directory to write the decompiled files to
    #[allow(dead_code)]
    pub fn output(mut self, directory: &str) -> CFGBuilder {
        self.args.output = directory.to_string();
        self
    }

    /// The RPC provider to use for fetching target bytecode.
    #[allow(dead_code)]
    pub fn rpc(mut self, url: &str) -> CFGBuilder {
        self.args.rpc_url = url.to_string();
        self
    }

    /// When prompted, always select the default value.
    #[allow(dead_code)]
    pub fn default(mut self, accept: bool) -> CFGBuilder {
        self.args.default = accept;
        self
    }

    /// The format to additionally generate to. (e.g. svg, png, pdf)
    #[allow(dead_code)]
    pub fn format(mut self, format: String) -> CFGBuilder {
        self.args.format = format;
        self
    }

    /// Whether to color the edges of the graph based on the JUMPI condition.
    #[allow(dead_code)]
    pub fn color_edges(mut self, color_edges: bool) -> CFGBuilder {
        self.args.color_edges = color_edges;
        self
    }

    /// Starts the decompilation.
    #[allow(dead_code)]
    pub fn generate(self) {
        cfg(self.args)
    }
}
