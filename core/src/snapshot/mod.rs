pub mod analyze;
pub mod constants;
pub mod menus;
pub mod resolve;
pub mod structures;
pub mod util;

use std::{
    collections::{HashMap, HashSet},
    fs,
    time::Duration,
};

use clap::{AppSettings, Parser};
use clap_verbosity_flag::Verbosity;
use derive_builder::Builder;
use heimdall_common::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::{
        compiler::detect_compiler,
        evm::core::vm::VM,
        rpc::get_code,
        selectors::{find_function_selectors, resolve_selectors},
        signatures::{score_signature, ResolvedError, ResolvedFunction, ResolvedLog},
    },
    utils::{
        io::logging::*,
        strings::{decode_hex, encode_hex_reduced, get_shortned_target},
    },
};
use indicatif::ProgressBar;

use crate::{
    disassemble::{disassemble, DisassemblerArgs},
    snapshot::{
        analyze::snapshot_trace,
        resolve::match_parameters,
        structures::snapshot::{GasUsed, Snapshot},
        util::tui,
    },
};
#[derive(Debug, Clone, Parser, Builder)]
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

    /// Whether to skip resolving function selectors.
    #[clap(long = "skip-resolving")]
    pub skip_resolving: bool,

    /// Whether to skip opening the TUI.
    #[clap(long)]
    pub no_tui: bool,

    /// The output directory to write the output to, or 'print' to print to the console.
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,
}

impl SnapshotArgsBuilder {
    pub fn new() -> Self {
        SnapshotArgsBuilder {
            target: Some(String::new()),
            verbose: Some(clap_verbosity_flag::Verbosity::new(0, 1)),
            rpc_url: Some(String::new()),
            default: Some(true),
            skip_resolving: Some(false),
            no_tui: Some(true),
            output: Some(String::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotResult {
    pub snapshots: Vec<Snapshot>,
    pub resolved_errors: HashMap<String, ResolvedError>,
    pub resolved_events: HashMap<String, ResolvedLog>,
}

/// The main snapshot function, which will be called from the main thread. This module is
/// responsible for generating a high-level overview of the target contract, including function
/// signatures, access control, gas consumption, storage accesses, event emissions, and more.
pub async fn snapshot(args: SnapshotArgs) -> Result<SnapshotResult, Box<dyn std::error::Error>> {
    use std::time::Instant;

    set_logger_env(&args.verbose);

    let now = Instant::now();
    let mut all_resolved_events: HashMap<String, ResolvedLog> = HashMap::new();
    let mut all_resolved_errors: HashMap<String, ResolvedError> = HashMap::new();
    let (logger, mut trace) = get_logger_and_trace(&args.verbose);
    let shortened_target = get_shortned_target(&args.target);

    let snapshot_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "snapshot".to_string(),
        vec![shortened_target],
        "()".to_string(),
    );

    let contract_bytecode = get_contract_bytecode(&args.target, &args.rpc_url, &logger).await?;

    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(&contract_bytecode);
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
    let shortened_target = get_shortned_target(&contract_bytecode);
    let vm_trace = trace.add_creation(
        snapshot_call,
        line!(),
        "contract".to_string(),
        shortened_target.clone(),
        (contract_bytecode.len() / 2usize).try_into()?,
    );

    let (selectors, resolved_selectors) = get_selectors(
        &contract_bytecode,
        args.skip_resolving,
        &logger,
        &evm,
        &shortened_target,
        args.rpc_url.clone(),
        args.verbose.clone(),
        &mut trace,
        snapshot_call,
    )
    .await?;

    let snapshots = get_snapshots(
        selectors,
        resolved_selectors,
        &contract_bytecode,
        &logger,
        &mut trace,
        vm_trace,
        &evm,
        &args,
        &mut all_resolved_events,
        &mut all_resolved_errors,
    )
    .await?;

    logger.info("symbolic execution completed.");
    logger.debug(&format!("snapshot completed in {:?}.", now.elapsed()));

    // open the tui
    if !args.no_tui {
        tui::handle(
            snapshots.clone(),
            &all_resolved_errors,
            &all_resolved_events,
            if args.target.len() > 64 { &shortened_target } else { args.target.as_str() },
            (compiler, &version),
        )
    }

    trace.display();
    Ok(SnapshotResult {
        snapshots,
        resolved_errors: all_resolved_errors,
        resolved_events: all_resolved_events,
    })
}

async fn get_contract_bytecode(
    target: &str,
    rpc_url: &str,
    logger: &Logger,
) -> Result<String, Box<dyn std::error::Error>> {
    if ADDRESS_REGEX.is_match(target)? {
        // We are snapshotting a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        get_code(target, rpc_url).await
    } else if BYTECODE_REGEX.is_match(target)? {
        logger.debug_max("using provided bytecode for snapshotting.");
        Ok(target.replacen("0x", "", 1))
    } else {
        logger.debug_max("using provided file for snapshotting.");

        // We are snapshotting a file, so we need to read the bytecode from the file.
        match fs::read_to_string(target) {
            Ok(contents) => {
                let _contents = contents.replace('\n', "");
                if BYTECODE_REGEX.is_match(&_contents)? && _contents.len() % 2 == 0 {
                    Ok(_contents.replacen("0x", "", 1))
                } else {
                    logger.error(&format!("file '{}' doesn't contain valid bytecode.", &target));
                    std::process::exit(1)
                }
            }
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &target));
                std::process::exit(1)
            }
        }
    }
}

async fn get_selectors(
    contract_bytecode: &str,
    skip_resolving: bool,
    logger: &Logger,
    evm: &VM,
    shortened_target: &str,
    rpc_url: String,
    verbose: Verbosity,
    trace: &mut TraceFactory,
    snapshot_call: u32,
) -> Result<
    (HashMap<String, u128>, HashMap<String, Vec<ResolvedFunction>>),
    Box<dyn std::error::Error>,
> {
    trace.add_call(
        snapshot_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

    // find and resolve all selectors in the bytecode
    let disassembled_bytecode = disassemble(DisassemblerArgs {
        rpc_url,
        verbose,
        target: contract_bytecode.to_string(),
        decimal_counter: false,
        output: String::new(),
    })
    .await?;
    let selectors = find_function_selectors(evm, &disassembled_bytecode);

    let mut resolved_selectors = HashMap::new();
    if !skip_resolving {
        resolved_selectors =
            resolve_selectors::<ResolvedFunction>(selectors.keys().cloned().collect()).await;

        // if resolved selectors are empty, we can't perform symbolic execution
        if resolved_selectors.is_empty() {
            logger.error(&format!(
                "failed to resolve any function selectors from '{shortened_target}' .",
                shortened_target = shortened_target
            ));
        }

        logger.info(&format!(
            "resolved {} possible functions from {} detected selectors.",
            resolved_selectors.len(),
            selectors.len()
        ));
    } else {
        logger.info(&format!("found {} possible function selectors.", selectors.len()));
    }

    logger.info(&format!("performing symbolic execution on '{shortened_target}' ."));

    Ok((selectors, resolved_selectors))
}

async fn get_snapshots(
    selectors: HashMap<String, u128>,
    resolved_selectors: HashMap<String, Vec<ResolvedFunction>>,
    contract_bytecode: &str,
    logger: &Logger,
    trace: &mut TraceFactory,
    vm_trace: u32,
    evm: &VM,
    args: &SnapshotArgs,
    all_resolved_events: &mut HashMap<String, ResolvedLog>,
    all_resolved_errors: &mut HashMap<String, ResolvedError>,
) -> Result<Vec<Snapshot>, Box<dyn std::error::Error>> {
    // get a new progress bar
    let mut snapshot_progress = ProgressBar::new_spinner();
    snapshot_progress.enable_steady_tick(Duration::from_millis(100));
    snapshot_progress.set_style(logger.info_spinner());

    // perform EVM analysis
    let mut snapshots: Vec<Snapshot> = Vec::new();
    for (selector, function_entry_point) in selectors {
        snapshot_progress.set_message(format!("executing '0x{selector}'"));

        let func_analysis_trace = trace.add_call(
            vm_trace,
            line!(),
            "heimdall".to_string(),
            "analyze".to_string(),
            vec![format!("0x{selector}")],
            "()".to_string(),
        );

        trace.add_info(
            func_analysis_trace,
            function_entry_point.try_into()?,
            &format!("discovered entry point: {function_entry_point}"),
        );

        // get a map of possible jump destinations
        let (map, jumpdest_count) =
            evm.clone().symbolic_exec_selector(&selector, function_entry_point);

        trace.add_debug(
            func_analysis_trace,
            function_entry_point.try_into()?,
            &format!(
                "execution tree {}",
                match jumpdest_count {
                    0 => {
                        "appears to be linear".to_string()
                    }
                    _ => format!("has {jumpdest_count} unique branches"),
                }
            ),
        );

        logger.debug_max(&format!(
            "building snapshot for selector {} from symbolic execution trace",
            selector
        ));
        let mut snapshot = snapshot_trace(
            &map,
            Snapshot {
                selector: selector.clone(),
                bytecode: decode_hex(&contract_bytecode.replacen("0x", "", 1))?,
                entry_point: function_entry_point,
                arguments: HashMap::new(),
                storage: HashSet::new(),
                memory: HashMap::new(),
                returns: None,
                events: HashMap::new(),
                errors: HashMap::new(),
                resolved_function: None,
                pure: true,
                view: true,
                payable: true,
                strings: HashSet::new(),
                external_calls: Vec::new(),
                gas_used: GasUsed { min: u128::MAX, max: 0, avg: 0 },
                addresses: HashSet::new(),
                branch_count: jumpdest_count,
                control_statements: HashSet::new(),
            },
            trace,
            func_analysis_trace,
        );

        // resolve signatures
        if !args.skip_resolving {
            resolve_signatures(
                &mut snapshot,
                &selector,
                &resolved_selectors,
                trace,
                func_analysis_trace, // TODO: not clone
                &mut snapshot_progress,
                logger,
                args.default,
                all_resolved_events,
                all_resolved_errors,
            )
            .await?;
        }

        // push
        snapshots.push(snapshot);

        // get a new progress bar
        snapshot_progress = ProgressBar::new_spinner();
        snapshot_progress.enable_steady_tick(Duration::from_millis(100));
        snapshot_progress.set_style(logger.info_spinner());
    }

    snapshot_progress.finish_and_clear();

    Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fancy_regex::Regex;

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_address() {
        let bytecode_regex = Regex::new(r"^[0-9a-fA-F]{0,50000}$").unwrap();
        let (logger, _) = get_logger_and_trace(&clap_verbosity_flag::Verbosity::new(-1, 0));
        let bytecode = get_contract_bytecode(
            "0x9f00c43700bc0000Ff91bE00841F8e04c0495000",
            "https://eth.llamarpc.com",
            &logger,
        )
        .await
        .unwrap();

        assert!(bytecode_regex.is_match(&bytecode).unwrap());
        // Not possible to express with regex since fancy_regex
        // doesn't support look-arounds
        assert!(!bytecode.starts_with("0x"));
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_bytecode() {
        let bytecode_regex = Regex::new(r"^[0-9a-fA-F]{0,50000}$").unwrap();
        let (logger, _) = get_logger_and_trace(&clap_verbosity_flag::Verbosity::new(-1, 0));
        let bytecode = get_contract_bytecode(
            "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001",
            "https://eth.llamarpc.com",
            &logger,
        ).await.unwrap();

        assert!(bytecode_regex.is_match(&bytecode).unwrap());
        assert!(!bytecode.starts_with("0x"));
    }

    #[tokio::test]
    async fn test_get_bytecode_when_target_is_file_path() {
        let (logger, _) = get_logger_and_trace(&clap_verbosity_flag::Verbosity::new(-1, 0));
        let bytecode_regex = Regex::new(r"^[0-9a-fA-F]{0,50000}$").unwrap();
        let file_path = "./mock-file.txt";
        let mock_bytecode = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";

        fs::write(file_path, mock_bytecode).unwrap();

        let bytecode =
            get_contract_bytecode(file_path, "https://eth.llamarpc.com", &logger).await.unwrap();

        assert!(bytecode_regex.is_match(&bytecode).unwrap());
        assert!(!bytecode.starts_with("0x"));

        fs::remove_file(file_path).unwrap();
    }

    #[tokio::test]
    async fn test_get_selectors() {}
}
