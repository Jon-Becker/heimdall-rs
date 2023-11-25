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
        strings::{decode_hex, encode_hex_reduced},
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

    let disassembled_bytecode = disassemble(DisassemblerArgs {
        target: contract_bytecode.clone(),
        verbose: args.verbose.clone(),
        rpc_url: args.rpc_url,
        decimal_counter: false,
        output: String::new(),
    })
    .await?;

    trace.add_call(
        snapshot_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

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

    // find and resolve all selectors in the bytecode
    let selectors = find_function_selectors(&evm, &disassembled_bytecode);

    let mut resolved_selectors = HashMap::new();
    if !args.skip_resolving {
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
            &evm.clone().symbolic_exec_selector(&selector, function_entry_point);

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
            map,
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
                branch_count: *jumpdest_count,
                control_statements: HashSet::new(),
            },
            &mut trace,
            func_analysis_trace,
        );

        // resolve signatures
        if !args.skip_resolving {
            let resolved_functions = match resolved_selectors.get(&selector) {
                Some(func) => func.clone(),
                None => {
                    trace.add_warn(
                        func_analysis_trace,
                        line!(),
                        "failed to resolve function signature",
                    );
                    Vec::new()
                }
            };

            let mut matched_resolved_functions = match_parameters(resolved_functions, &snapshot);

            trace.br(func_analysis_trace);
            if matched_resolved_functions.is_empty() {
                trace.add_warn(
                    func_analysis_trace,
                    line!(),
                    "no resolved signatures matched this function's parameters",
                );
            } else {
                let mut selected_function_index: u8 = 0;

                // sort matches by signature using score heuristic from `score_signature`
                matched_resolved_functions.sort_by(|a, b| {
                    let a_score = score_signature(&a.signature);
                    let b_score = score_signature(&b.signature);
                    b_score.cmp(&a_score)
                });

                if matched_resolved_functions.len() > 1 {
                    snapshot_progress.suspend(|| {
                        selected_function_index = logger.option(
                            "warn",
                            "multiple possible matches found. select an option below",
                            matched_resolved_functions
                                .iter()
                                .map(|x| x.signature.clone())
                                .collect(),
                            Some(0u8),
                            args.default,
                        );
                    });
                }

                let selected_match =
                    match matched_resolved_functions.get(selected_function_index as usize) {
                        Some(selected_match) => selected_match,
                        None => continue,
                    };

                snapshot.resolved_function = Some(selected_match.clone());

                let match_trace = trace.add_info(
                    func_analysis_trace,
                    line!(),
                    &format!(
                        "{} resolved signature{} matched this function's parameters",
                        matched_resolved_functions.len(),
                        if matched_resolved_functions.len() > 1 { "s" } else { "" }
                    )
                    .to_string(),
                );

                for resolved_function in matched_resolved_functions {
                    trace.add_message(match_trace, line!(), vec![resolved_function.signature]);
                }
            }

            snapshot_progress.finish_and_clear();

            // resolve custom error signatures
            let mut resolved_counter = 0;
            let resolved_errors: HashMap<String, Vec<ResolvedError>> = resolve_selectors(
                snapshot
                    .errors
                    .keys()
                    .map(|error_selector| encode_hex_reduced(*error_selector).replacen("0x", "", 1))
                    .collect(),
            )
            .await;
            for (error_selector, _) in snapshot.errors.clone() {
                let error_selector_str = encode_hex_reduced(error_selector).replacen("0x", "", 1);
                let mut selected_error_index: u8 = 0;
                let mut resolved_error_selectors = match resolved_errors.get(&error_selector_str) {
                    Some(func) => func.clone(),
                    None => Vec::new(),
                };

                // sort matches by signature using score heuristic from `score_signature`
                resolved_error_selectors.sort_by(|a, b| {
                    let a_score = score_signature(&a.signature);
                    let b_score = score_signature(&b.signature);
                    b_score.cmp(&a_score)
                });

                if resolved_error_selectors.len() > 1 {
                    snapshot_progress.suspend(|| {
                        selected_error_index = logger.option(
                            "warn",
                            "multiple possible matches found. select an option below",
                            resolved_error_selectors.iter().map(|x| x.signature.clone()).collect(),
                            Some(0u8),
                            args.default,
                        );
                    });
                }

                let selected_match =
                    match resolved_error_selectors.get(selected_error_index as usize) {
                        Some(selected_match) => selected_match,
                        None => continue,
                    };

                resolved_counter += 1;
                snapshot.errors.insert(error_selector, Some(selected_match.clone()));
                all_resolved_errors.insert(error_selector_str, selected_match.clone());
            }

            if resolved_counter > 0 {
                trace.br(func_analysis_trace);
                let error_trace = trace.add_info(
                    func_analysis_trace,
                    line!(),
                    &format!(
                        "resolved {} error signatures from {} selectors.",
                        resolved_counter,
                        snapshot.errors.len()
                    )
                    .to_string(),
                );

                for resolved_error in all_resolved_errors.values() {
                    trace.add_message(error_trace, line!(), vec![resolved_error.signature.clone()]);
                }
            }

            // resolve custom event signatures
            resolved_counter = 0;
            let resolved_events: HashMap<String, Vec<ResolvedLog>> = resolve_selectors(
                snapshot
                    .events
                    .keys()
                    .map(|event_selector| encode_hex_reduced(*event_selector).replacen("0x", "", 1))
                    .collect(),
            )
            .await;
            for (event_selector, (_, raw_event)) in snapshot.events.clone() {
                let mut selected_event_index: u8 = 0;
                let event_selector_str = encode_hex_reduced(event_selector).replacen("0x", "", 1);
                let mut resolved_event_selectors = match resolved_events.get(&event_selector_str) {
                    Some(func) => func.clone(),
                    None => Vec::new(),
                };

                // sort matches by signature using score heuristic from `score_signature`
                resolved_event_selectors.sort_by(|a, b| {
                    let a_score = score_signature(&a.signature);
                    let b_score = score_signature(&b.signature);
                    b_score.cmp(&a_score)
                });

                if resolved_event_selectors.len() > 1 {
                    snapshot_progress.suspend(|| {
                        selected_event_index = logger.option(
                            "warn",
                            "multiple possible matches found. select an option below",
                            resolved_event_selectors.iter().map(|x| x.signature.clone()).collect(),
                            Some(0u8),
                            args.default,
                        );
                    });
                }

                let selected_match =
                    match resolved_event_selectors.get(selected_event_index as usize) {
                        Some(selected_match) => selected_match,
                        None => continue,
                    };

                resolved_counter += 1;
                snapshot.events.insert(event_selector, (Some(selected_match.clone()), raw_event));
                all_resolved_events.insert(event_selector_str, selected_match.clone());
            }

            if resolved_counter > 0 {
                let event_trace = trace.add_info(
                    func_analysis_trace,
                    line!(),
                    &format!(
                        "resolved {} event signatures from {} selectors.",
                        resolved_counter,
                        snapshot.events.len()
                    ),
                );

                for resolved_event in all_resolved_events.values() {
                    trace.add_message(event_trace, line!(), vec![resolved_event.signature.clone()]);
                }
            }
        }

        // push
        snapshots.push(snapshot);

        // get a new progress bar
        snapshot_progress = ProgressBar::new_spinner();
        snapshot_progress.enable_steady_tick(Duration::from_millis(100));
        snapshot_progress.set_style(logger.info_spinner());
    }
    snapshot_progress.finish_and_clear();
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

fn set_logger_env(verbosity: &clap_verbosity_flag::Verbosity) {
    let env_not_set = std::env::var("RUST_LOG").is_err();

    if env_not_set {
        let log_level = match verbosity.log_level() {
            Some(level) => level.as_str(),
            None => "SILENT",
        };

        std::env::set_var("RUST_LOG", log_level);
    }
}

fn get_logger_and_trace(verbosity: &clap_verbosity_flag::Verbosity) -> (Logger, TraceFactory) {
    Logger::new(match verbosity.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    })
}

fn get_shortned_target(target: &String) -> String {
    let mut shortened_target = target.clone();

    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    shortened_target
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

#[cfg(test)]
mod tests {
    use super::*;
    use fancy_regex::Regex;
    use std::env;

    #[test]
    fn test_set_logger_env_default() {
        env::remove_var("RUST_LOG");

        let verbosity = clap_verbosity_flag::Verbosity::new(-1, 0);

        set_logger_env(&verbosity);

        assert_eq!(env::var("RUST_LOG").unwrap(), "SILENT");
    }

    #[test]
    fn test_shorten_long_target() {
        let long_target = "0".repeat(80);
        let shortened_target = get_shortned_target(&long_target);

        assert_eq!(shortened_target.len(), 85);
    }

    #[test]
    fn test_shorten_short_target() {
        let short_target = "0".repeat(66);
        let shortened_target = get_shortned_target(&short_target);

        assert_eq!(shortened_target.len(), 66);
    }

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
}
