pub mod util;

use std::collections::HashMap;
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
    consts::{ ADDRESS_REGEX, BYTECODE_REGEX },
    io::{ logging::* },
};
use crate::decompile::util::*;

#[derive(Debug, Clone, Parser)]
#[clap(about = "Decompile EVM bytecode to Solidity",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall decompile <TARGET> [OPTIONS]")]
pub struct DecompilerArgs {
    
    /// The target to decompile, either a file, bytecode, contract address, or ENS name.
    #[clap(required=true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
    
    /// The output directory to write the decompiled files to
    #[clap(long="output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Whether to skip resolving function selectors.
    #[clap(long="skip-resolving")]
    pub skip_resolving: bool,

}

pub fn decompile(args: DecompilerArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, mut trace)= Logger::new(args.verbose.log_level().unwrap().as_str());

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() + "..." + &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }
    let decompile_call = trace.add_call(
        0, line!(), 
        "heimdall".to_string(), 
        "decompile".to_string(), 
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

    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target) {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // We are decompiling a contract address, so we need to fetch the bytecode from the RPC provider.
        contract_bytecode = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("disassembling an on-chain contract requires an RPC provider. Use `heimdall disassemble --help` for more information.");
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
    else if BYTECODE_REGEX.is_match(&args.target) {
        contract_bytecode = args.target.clone();
    }
    else {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str("/local");
        }

        // We are decompiling a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {                
                if BYTECODE_REGEX.is_match(&contents) && contents.len() % 2 == 0 {
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
        output: args.output.clone(),
        rpc_url: args.rpc_url.clone(),
    });
    trace.add_call(
        decompile_call, 
        line!(), 
        "heimdall".to_string(), 
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len()/2usize)], 
        "()".to_string()
    );
    
    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(contract_bytecode.clone());
    trace.add_call(
        decompile_call, 
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
    let vm_trace = trace.add_creation(decompile_call, line!(), "contract".to_string(), shortened_target, (contract_bytecode.len()/2usize).try_into().unwrap());

    // find and resolve all selectors in the bytecode
    let selectors = find_function_selectors(&evm.clone(), disassembled_bytecode);

    // TODO: add to trace
    if !args.skip_resolving {
        let resolved_selectors = resolve_function_selectors(selectors.clone());
        logger.info(&format!("resolved {} possible functions from {} detected selectors.", resolved_selectors.len(), selectors.len()).to_string());
    }
    else {
        logger.info(&format!("found {} function selectors.", selectors.len()).to_string());
    }
    logger.info(&format!("performing symbolic execution on '{}' .", &args.target).to_string());

    let decompilation_progress = ProgressBar::new_spinner();
    decompilation_progress.enable_steady_tick(Duration::from_millis(100));
    decompilation_progress.set_style(logger.info_spinner());

    // perform EVM analysis    
    for selector in selectors.clone() {
        decompilation_progress.set_message(format!("executing '0x{}'", selector));
        
        let func_analysis_trace = trace.add_call(
            vm_trace, 
            line!(), 
            "heimdall".to_string(), 
            "analyze".to_string(), 
            vec![format!("0x{}", selector)], 
            "()".to_string()
        );

        // get the function's entry point
        let function_entry_point = resolve_entry_point(&evm.clone(), selector.clone());
        trace.add_info(
            func_analysis_trace, 
            function_entry_point.try_into().unwrap(), 
            format!("discovered entry point: {}", function_entry_point).to_string()
        );

        if function_entry_point == 0 {
            trace.add_error(
                func_analysis_trace,
                line!(), 
                "selector flagged as false-positive.".to_string()
            );
            continue;
        }

        // get a map of possible jump destinations
        let (map, jumpdests) = map_selector(&evm.clone(), &trace, func_analysis_trace, selector.clone(), function_entry_point);
        trace.add_debug(
            func_analysis_trace,
            function_entry_point.try_into().unwrap(),
            format!("execution tree {}",
            
            match jumpdests.len() {
                0 => "appears to be linear".to_string(),
                _ => format!("has {} branches", jumpdests.len()+1)
            }
            ).to_string()
        );
        
        decompilation_progress.set_message(format!("analyzing '0x{}'", selector));

        // solidify the execution tree
        let analyzed_function = map.analyze(
            Function {
                selector: selector.clone(),
                entry_point: function_entry_point.clone(),
                arguments: HashMap::new(),
                storage: HashMap::new(),
                memory: HashMap::new(),
                returns: None,
                logic: Vec::new(),
                events: Vec::new(),
                pure: true,
                view: true,
                payable: false,
                constant: true,
                external: false,
            },
            &mut trace,
            func_analysis_trace,
        );

        let argument_count = analyzed_function.arguments.len();

        if argument_count != 0 {
            let l = trace.add_debug(
                func_analysis_trace,
                line!(),
                format!("discovered and analyzed {} function parameters", argument_count).to_string()
            );
            for (_, (frame, _)) in analyzed_function.arguments.iter() {
                trace.add_message(
                    l,
                    line!(),
                    vec![
                        format!(
                            "parameter {} {} {} bytes.",
                            frame.slot,
                            if frame.mask_size == 32 { "has size of" } else { "is masked to" },
                            frame.mask_size
                        ).to_string()
                    ]
                );
            }
        }
    }
    decompilation_progress.finish_and_clear();
    logger.info("symbolic execution completed.");

    trace.display();
    logger.debug(&format!("decompilation completed in {:?}.", now.elapsed()).to_string());
}