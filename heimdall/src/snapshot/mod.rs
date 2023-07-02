use std::fs;

use clap::{AppSettings, Parser};
use heimdall_common::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::{
        compiler::detect_compiler,
        evm::disassemble::{disassemble, DisassemblerArgs},
        rpc::get_code,
    },
    io::logging::*,
};
#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Infer function information from bytecode, including access control, gas consumption, storage accesses, event emissions, and more",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall snapshot <TARGET> [OPTIONS]"
)]
pub struct SnapshotArgs {
    /// The target to analyze. This may be a file, bytecode, or contract address.
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
}

#[allow(dead_code, unused_variables)]
pub fn snapshot(args: SnapshotArgs) {
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
    let snapshot_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "snapshot".to_string(),
        vec![shortened_target],
        "()".to_string(),
    );

    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {
        // We are snapshotting a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        contract_bytecode = get_code(&args.target, &args.rpc_url, &logger);
    } else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target.clone().replacen("0x", "", 1);
    } else {
        // We are snapshotting a file, so we need to read the bytecode from the file.
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
        output: "".to_string(),
        rpc_url: args.rpc_url.clone(),
    });
    trace.add_call(
        snapshot_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(contract_bytecode.clone());
    trace.add_call(
        snapshot_call,
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

    // TODO: perform snapshot analysis

    trace.display();
    logger.debug(&format!("snapshot completed in {:?}.", now.elapsed()));
}
