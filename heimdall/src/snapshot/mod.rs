pub mod analyze;
pub mod constants;
pub mod menus;
pub mod resolve;
pub mod structures;
pub mod util;

use std::{
    collections::{HashMap, HashSet},
    env, fs,
    time::Duration,
};

use clap::{AppSettings, Parser};
use heimdall_common::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::{
        compiler::detect_compiler,
        evm::{
            core::vm::VM,
            ext::disassemble::{disassemble, DisassemblerArgs},
        },
        rpc::get_code,
        selectors::{find_function_selectors, resolve_selectors},
        signatures::{score_signature, ResolvedError, ResolvedFunction, ResolvedLog},
    },
    io::logging::*,
    utils::strings::{decode_hex, encode_hex_reduced},
};
use indicatif::ProgressBar;

use crate::snapshot::{
    analyze::snapshot_trace,
    resolve::match_parameters,
    util::{tui, GasUsed, Snapshot},
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

    /// The output directory to write the output files to
    #[clap(long = "output", short, default_value = "", hide_default_value = true)]
    pub output: String,

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
}

pub fn snapshot(args: SnapshotArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });
    let mut all_resolved_events: HashMap<String, ResolvedLog> = HashMap::new();
    let mut all_resolved_errors: HashMap<String, ResolvedError> = HashMap::new();

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

    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // We are snapshotting a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        contract_bytecode = get_code(&args.target, &args.rpc_url, &logger);
    } else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target.clone().replacen("0x", "", 1);
    } else {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str("/local");
        }

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
        rpc_url: args.rpc_url,
        decimal_counter: false,
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
    let mut shortened_target = contract_bytecode.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }
    let vm_trace = trace.add_creation(
        snapshot_call,
        line!(),
        "contract".to_string(),
        shortened_target.clone(),
        (contract_bytecode.len() / 2usize).try_into().unwrap(),
    );

    // find and resolve all selectors in the bytecode
    let selectors = find_function_selectors(&evm, &disassembled_bytecode);

    let mut resolved_selectors = HashMap::new();
    if !args.skip_resolving {
        resolved_selectors =
            resolve_selectors::<ResolvedFunction>(selectors.keys().cloned().collect(), &logger);

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
            function_entry_point.try_into().unwrap(),
            &format!("discovered entry point: {function_entry_point}"),
        );

        // get a map of possible jump destinations
        let (map, jumpdest_count) =
            &evm.clone().symbolic_exec_selector(&selector, function_entry_point);

        trace.add_debug(
            func_analysis_trace,
            function_entry_point.try_into().unwrap(),
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

        let mut snapshot = snapshot_trace(
            map,
            Snapshot {
                selector: selector.clone(),
                bytecode: decode_hex(&contract_bytecode.replacen("0x", "", 1)).unwrap(),
                entry_point: function_entry_point,
                arguments: HashMap::new(),
                storage: HashMap::new(),
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
            },
            &mut trace,
            func_analysis_trace,
        );

        println!("function {}, gas used: {:?}", selector, snapshot.gas_used);

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
                &logger,
            );
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
                &logger,
            );
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
            snapshots,
            all_resolved_errors,
            all_resolved_events,
            if args.target.len() > 64 { &shortened_target } else { args.target.as_str() },
            (compiler, &version),
        )
    }

    trace.display();
}
