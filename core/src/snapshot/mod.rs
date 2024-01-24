pub mod analyze;
pub mod constants;
pub mod menus;
pub mod resolve;
pub mod structures;
pub mod util;
use heimdall_common::{debug_max, utils::threading::run_with_timeout, debug, warn, info};

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use clap::{AppSettings, Parser};
use derive_builder::Builder;
use heimdall_common::{
    ether::{
        bytecode::get_bytecode_from_target,
        compiler::detect_compiler,
        evm::core::vm::VM,
        selectors::get_resolved_selectors,
        signatures::{ResolvedError, ResolvedFunction, ResolvedLog},
    },
    utils::{
        io::logging::*,
        strings::{decode_hex, get_shortned_target},
    },
};
use indicatif::ProgressBar;

use crate::{
    disassemble::{disassemble, DisassemblerArgs},
    snapshot::{
        analyze::snapshot_trace,
        resolve::resolve_signatures,
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

    /// Name for the output snapshot file.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// The output directory to write the output to, or 'print' to print to the console.
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The timeout for each function's symbolic execution in milliseconds.
    #[clap(long, short, default_value = "10000", hide_default_value = true)]
    pub timeout: u64,
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
            name: Some(String::new()),
            output: Some(String::new()),
            timeout: Some(10000),
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
    let (logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });
    let shortened_target = get_shortned_target(&args.target);
    let snapshot_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "snapshot".to_string(),
        vec![shortened_target],
        "()".to_string(),
    );

    let contract_bytecode = get_bytecode_from_target(&args.target, &args.rpc_url).await?;

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
        debug!("detected compiler {} {}.", compiler, version);
    } else {
        warn!("detected compiler {} {} is not supported by heimdall.", compiler, version);
    }

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

    trace.add_call(
        snapshot_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

    let disassembled_bytecode = disassemble(DisassemblerArgs {
        rpc_url: args.rpc_url.clone(),
        verbose: args.verbose.clone(),
        target: args.target.clone(),
        name: args.name.clone(),
        decimal_counter: false,
        output: String::new(),
    })
    .await?;

    let (selectors, resolved_selectors) =
        get_resolved_selectors(&disassembled_bytecode, &args.skip_resolving, &evm).await?;

    let (snapshots, all_resolved_errors, all_resolved_events) = get_snapshots(
        selectors,
        resolved_selectors,
        &contract_bytecode,
        &logger,
        &mut trace,
        vm_trace,
        &evm,
        &args,
    )
    .await?;

    info!("symbolic execution completed.");
    debug!("snapshot completed in {:?}.", now.elapsed());

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

async fn get_snapshots(
    selectors: HashMap<String, u128>,
    resolved_selectors: HashMap<String, Vec<ResolvedFunction>>,
    contract_bytecode: &str,
    logger: &Logger,
    trace: &mut TraceFactory,
    vm_trace: u32,
    evm: &VM,
    args: &SnapshotArgs,
) -> Result<
    (Vec<Snapshot>, HashMap<String, ResolvedError>, HashMap<String, ResolvedLog>),
    Box<dyn std::error::Error>,
> {
    let mut all_resolved_errors: HashMap<String, ResolvedError> = HashMap::new();
    let mut all_resolved_events: HashMap<String, ResolvedLog> = HashMap::new();
    let mut snapshots: Vec<Snapshot> = Vec::new();
    let mut snapshot_progress = ProgressBar::new_spinner();

    snapshot_progress.enable_steady_tick(Duration::from_millis(100));
    snapshot_progress.set_style(logger.info_spinner());

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
        let mut evm_clone = evm.clone();
        let selector_clone = selector.clone();
        let (map, jumpdest_count) = match run_with_timeout(
            move || evm_clone.symbolic_exec_selector(&selector_clone, function_entry_point),
            Duration::from_millis(args.timeout),
        ) {
            Some(map) => map,
            None => {
                trace.add_error(
                    func_analysis_trace,
                    line!(),
                    "symbolic execution timed out, skipping snapshotting.",
                );
                continue
            }
        };

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

        debug_max!("building snapshot for selector {} from symbolic execution trace", selector);
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

        if !args.skip_resolving {
            resolve_signatures(
                &mut snapshot,
                &mut all_resolved_errors,
                &mut all_resolved_events,
                &mut snapshot_progress,
                trace,
                &selector,
                &resolved_selectors,
                func_analysis_trace,
                args.default,
            )
            .await?;
        }

        snapshots.push(snapshot);

        snapshot_progress = ProgressBar::new_spinner();
        snapshot_progress.enable_steady_tick(Duration::from_millis(100));
        snapshot_progress.set_style(logger.info_spinner());
    }

    snapshot_progress.finish_and_clear();

    Ok((snapshots, all_resolved_errors, all_resolved_events))
}
