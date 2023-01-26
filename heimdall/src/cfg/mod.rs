mod tests;

pub mod output;
pub mod graph;
pub mod util;

use std::env;
use std::fs;
use std::time::Duration;
use indicatif::ProgressBar;

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{Address},
    providers::{Middleware, Provider, Http},
};
use heimdall_common::{
    ether::evm::{
        disassemble::{
            DisassemblerArgs,
            disassemble
        },
        vm::VM
    },
    constants::{ ADDRESS_REGEX, BYTECODE_REGEX },
    io::{ logging::* },
};
use petgraph::Graph;

use crate::cfg::output::build_output;
use crate::cfg::util::detect_compiler;
use crate::cfg::util::find_function_selectors;
use crate::cfg::util::map_selector;
use crate::cfg::util::resolve_entry_point;

#[derive(Debug, Clone, Parser)]
#[clap(about = "Generate a visual control flow graph for EVM bytecode",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall cfg <TARGET> [OPTIONS]")]
pub struct CFGArgs {

    /// The target to generate a CFG for, either a file, bytecode, contract address, or ENS name.
    #[clap(required=true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The output directory to write the output to
    #[clap(long="output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,
}

pub fn cfg(args: CFGArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, mut trace)= Logger::new(args.verbose.log_level().unwrap().as_str());

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() + "..." + &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the call to the trace
    let cfg_call = trace.add_call(
        0, line!(),
        "heimdall".to_string(),
        "cfg".to_string(),
        vec![shortened_target],
        "()".to_string()
    );

    // parse the output directory
    let mut output_dir: String;
    if &args.output.len() <= &0 {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    }
    else {
        output_dir = args.output.clone();
    }

    // fetch bytecode
    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // We are working with a contract address, so we need to fetch the bytecode from the RPC provider.
        contract_bytecode = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("fetching an on-chain contract requires an RPC provider. Use `heimdall cfg --help` for more information.");
                std::process::exit(1);
            }

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // safely unwrap the address
            let address = match args.target.parse::<Address>() {
                Ok(address) => address,
                Err(_) => {
                    logger.error(&format!("failed to parse address '{}' .", &args.target).to_string());
                    std::process::exit(1)
                }
            };

            // fetch the bytecode at the address
            let bytecode_as_bytes = match provider.get_code(address, None).await {
                Ok(bytecode) => bytecode,
                Err(_) => {
                    logger.error(&format!("failed to fetch bytecode from '{}' .", &args.target).to_string());
                    std::process::exit(1)
                }
            };
            return bytecode_as_bytes.to_string().replacen("0x", "", 1);
        });

    }
    else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target.clone();
    }
    else {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str("/local");
        }

        // We are analyzing a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {
                if BYTECODE_REGEX.is_match(&contents).unwrap() && contents.len() % 2 == 0 {
                    contents.replacen("0x", "", 1)
                }
                else {
                    logger.error(&format!("file '{}' doesn't contain valid bytecode.", &args.target).to_string());
                    std::process::exit(1)
                }
            },
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &args.target).to_string());
                std::process::exit(1)
            }
        };
    }

    // disassemble the bytecode
    let disassembled_bytecode = disassemble(DisassemblerArgs {
        target: contract_bytecode.clone(),
        default: args.default.clone(),
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
        vec![format!("{} bytes", contract_bytecode.len()/2usize)],
        "()".to_string()
    );

    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(contract_bytecode.clone());
    trace.add_call(
        cfg_call,
        line!(),
        "heimdall".to_string(),
        "detect_compiler".to_string(),
        vec![format!("{} bytes", contract_bytecode.len()/2usize)],
        format!("({}, {})", compiler, version)
    );

    if compiler == "solc" {
        logger.debug(&format!("detected compiler {} {}.", compiler, version));
    }
    else {
        logger.warn(&format!("detected compiler {} {} is not supported by heimdall.", compiler, version));
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
        shortened_target = shortened_target.chars().take(66).collect::<String>() + "..." + &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    // add the creation to the trace
    let vm_trace = trace.add_creation(cfg_call, line!(), "contract".to_string(), shortened_target, (contract_bytecode.len()/2usize).try_into().unwrap());

    // find all selectors in the bytecode
    let selectors = find_function_selectors(disassembled_bytecode);
    logger.info(&format!("found {} possible function selectors.", selectors.len()).to_string());
    logger.info(&format!("performing symbolic execution on '{}' .", &args.target).to_string());

    // create a new progress bar
    let progress = ProgressBar::new_spinner();
    progress.enable_steady_tick(Duration::from_millis(100));
    progress.set_style(logger.info_spinner());

    // create a new petgraph StableGraph
    let mut contract_cfg = Graph::<String, String>::new();

    // perform EVM symbolic execution
    for selector in selectors.clone() {
        progress.set_message(format!("executing '0x{}'", selector));

        // get the function's entry point
        let function_entry_point = resolve_entry_point(&evm.clone(), selector.clone());

        // if the entry point is 0, then the function is not reachable
        if function_entry_point == 0 {
            continue;
        }

        // add the call to the trace
        let func_analysis_trace = trace.add_call(
            vm_trace,
            line!(),
            "heimdall".to_string(),
            "analyze".to_string(),
            vec![format!("0x{}", selector)],
            "()".to_string()
        );

        // add the entry point to the trace
        trace.add_info(
            func_analysis_trace,
            function_entry_point.try_into().unwrap(),
            format!("discovered entry point: {}", function_entry_point).to_string()
        );

        // get a map of possible jump destinations
        let (map, jumpdest_count) = map_selector(&evm.clone(), selector.clone(), function_entry_point);

        map.build_cfg(&mut contract_cfg, None);

        // add the jumpdest count* to the trace
        trace.add_debug(
            func_analysis_trace,
            function_entry_point.try_into().unwrap(),
            format!("execution tree {}",

            match jumpdest_count {
                0 => "appears to be linear".to_string(),
                _ => format!("has {} branches", jumpdest_count)
            }
            ).to_string()
        );

        if jumpdest_count >= 1000 {
            trace.add_error(
                func_analysis_trace,
                function_entry_point.try_into().unwrap(),
                format!("Execution tree truncated to {} branches", jumpdest_count).to_string()
            );
        }
    }

    progress.finish_and_clear();
    logger.info("symbolic execution completed.");

    // build the dot file
    build_output(
        &contract_cfg,
        &args,
        output_dir.clone(),
        &logger,
        &mut trace,
        cfg_call
    );
    
    logger.debug(&format!("Control flow graph generated in {:?}.", now.elapsed()).to_string());
    trace.display();
}

