pub mod constants;
pub mod lexers;
pub mod out;
pub mod precompile;
pub mod resolve;
pub mod util;

use crate::decompile::{resolve::*, util::*};

use heimdall_common::{
    ether::{
        compiler::detect_compiler,
        rpc::get_code,
        selectors::{find_function_selectors, resolve_selectors},
    },
    utils::strings::encode_hex_reduced,
};
use indicatif::ProgressBar;
use std::{collections::HashMap, env, fs, time::Duration};

use clap::{AppSettings, Parser};
use heimdall_common::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::{
        evm::{
            disassemble::{disassemble, DisassemblerArgs},
            vm::VM,
        },
        signatures::*,
    },
    io::logging::*,
};

#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Decompile EVM bytecode to Solidity",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall decompile <TARGET> [OPTIONS]"
)]
pub struct DecompilerArgs {
    /// The target to decompile, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The output directory to write the decompiled files to
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

    /// Whether to include solidity source code in the output (in beta).
    #[clap(long = "include-sol")]
    pub include_solidity: bool,

    /// Whether to include yul source code in the output (in beta).
    #[clap(long = "include-yul")]
    pub include_yul: bool,
}

pub fn decompile(args: DecompilerArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, mut trace) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });
    let mut all_resolved_events: HashMap<String, ResolvedLog> = HashMap::new();
    let mut all_resolved_errors: HashMap<String, ResolvedError> = HashMap::new();

    // ensure both --include-sol and --include-yul aren't set
    if args.include_solidity && args.include_yul {
        logger.error("arguments '--include-sol' and '--include-yul' are mutually exclusive.");
        std::process::exit(1);
    }

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }
    let decompile_call = trace.add_call(
        0,
        line!(),
        "heimdall".to_string(),
        "decompile".to_string(),
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

        // We are decompiling a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        contract_bytecode = get_code(&args.target, &args.rpc_url, &logger);
    } else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target.clone().replacen("0x", "", 1);
    } else {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str("/local");
        }

        // We are decompiling a file, so we need to read the bytecode from the file.
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
    trace.add_call(
        decompile_call,
        line!(),
        "heimdall".to_string(),
        "disassemble".to_string(),
        vec![format!("{} bytes", contract_bytecode.len() / 2usize)],
        "()".to_string(),
    );

    // perform versioning and compiler heuristics
    let (compiler, version) = detect_compiler(contract_bytecode.clone());
    trace.add_call(
        decompile_call,
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
        decompile_call,
        line!(),
        "contract".to_string(),
        shortened_target.clone(),
        (contract_bytecode.len() / 2usize).try_into().unwrap(),
    );

    // find and resolve all selectors in the bytecode
    let selectors = find_function_selectors(&evm, disassembled_bytecode);

    let mut resolved_selectors = HashMap::new();
    if !args.skip_resolving {
        resolved_selectors = resolve_selectors(selectors.keys().cloned().collect(), &logger);

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
    let mut decompilation_progress = ProgressBar::new_spinner();
    decompilation_progress.enable_steady_tick(Duration::from_millis(100));
    decompilation_progress.set_style(logger.info_spinner());

    // perform EVM analysis
    let mut analyzed_functions = Vec::new();
    for (selector, function_entry_point) in selectors {
        decompilation_progress.set_message(format!("executing '0x{selector}'"));

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
            format!("discovered entry point: {function_entry_point}").to_string(),
        );

        // get a map of possible jump destinations
        let (map, jumpdest_count) =
            map_selector(&evm.clone(), selector.clone(), function_entry_point);

        trace.add_debug(
            func_analysis_trace,
            function_entry_point.try_into().unwrap(),
            format!(
                "execution tree {}",
                match jumpdest_count {
                    0 => {
                        "appears to be linear".to_string()
                    }
                    _ => format!("has {jumpdest_count} unique branches"),
                }
            )
            .to_string(),
        );

        decompilation_progress.set_message(format!("analyzing '0x{selector}'"));

        // analyze execution tree
        let mut analyzed_function;
        if args.include_yul {
            analyzed_function = map.analyze_yul(
                Function {
                    selector: selector.clone(),
                    entry_point: function_entry_point,
                    arguments: HashMap::new(),
                    storage: HashMap::new(),
                    memory: HashMap::new(),
                    returns: None,
                    logic: Vec::new(),
                    events: HashMap::new(),
                    errors: HashMap::new(),
                    resolved_function: None,
                    indent_depth: 0,
                    notices: Vec::new(),
                    pure: true,
                    view: true,
                    payable: true,
                },
                &mut trace,
                func_analysis_trace,
                &mut Vec::new(),
            );
        } else {
            analyzed_function = map.analyze_sol(
                Function {
                    selector: selector.clone(),
                    entry_point: function_entry_point,
                    arguments: HashMap::new(),
                    storage: HashMap::new(),
                    memory: HashMap::new(),
                    returns: None,
                    logic: Vec::new(),
                    events: HashMap::new(),
                    errors: HashMap::new(),
                    resolved_function: None,
                    indent_depth: 0,
                    notices: Vec::new(),
                    pure: true,
                    view: true,
                    payable: true,
                },
                &mut trace,
                func_analysis_trace,
                &mut Vec::new(),
            );
        }

        let argument_count = analyzed_function.arguments.len();

        if argument_count != 0 {
            let parameter_trace_parent = trace.add_debug(
                func_analysis_trace,
                line!(),
                format!("discovered and analyzed {argument_count} function parameters").to_string(),
            );

            let mut parameter_vec = Vec::new();
            for (_, value) in analyzed_function.arguments.clone() {
                parameter_vec.push(value);
            }
            parameter_vec.sort_by(|a, b| a.0.slot.cmp(&b.0.slot));

            for (frame, _) in parameter_vec {
                trace.add_message(
                    parameter_trace_parent,
                    line!(),
                    vec![format!(
                        "parameter {} {} {} bytes. {}",
                        frame.slot,
                        if frame.mask_size == 32 { "has size of" } else { "is masked to" },
                        frame.mask_size,
                        if !frame.heuristics.is_empty() {
                            format!("heuristics suggest param used as '{}'", frame.heuristics[0])
                        } else {
                            "".to_string()
                        }
                    )
                    .to_string()],
                );
            }
        }

        // resolve signatures
        if !args.skip_resolving {
            let resolved_functions = match resolved_selectors.get(&selector) {
                Some(func) => func.clone(),
                None => {
                    trace.add_warn(
                        func_analysis_trace,
                        line!(),
                        "failed to resolve function signature".to_string(),
                    );
                    Vec::new()
                }
            };

            let mut matched_resolved_functions =
                match_parameters(resolved_functions, &analyzed_function);

            trace.br(func_analysis_trace);
            if matched_resolved_functions.is_empty() {
                trace.add_warn(
                    func_analysis_trace,
                    line!(),
                    "no resolved signatures matched this function's parameters".to_string(),
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
                    decompilation_progress.suspend(|| {
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
                        None => {
                            logger.error("invalid selection.");
                            std::process::exit(1)
                        }
                    };

                analyzed_function.resolved_function = Some(selected_match.clone());

                let match_trace = trace.add_info(
                    func_analysis_trace,
                    line!(),
                    format!(
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

            decompilation_progress.finish_and_clear();

            // resolve custom error signatures
            let mut resolved_counter = 0;
            let resolved_errors: HashMap<String, Vec<ResolvedError>> = resolve_selectors(
                analyzed_function
                    .errors
                    .iter()
                    .map(|(error_selector, _)| {
                        encode_hex_reduced(*error_selector).replacen("0x", "", 1)
                    })
                    .collect(),
                &logger,
            );
            for (error_selector, _) in analyzed_function.errors.clone() {
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
                    decompilation_progress.suspend(|| {
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
                        None => {
                            logger.error("invalid selection.");
                            std::process::exit(1)
                        }
                    };

                resolved_counter += 1;
                analyzed_function.errors.insert(error_selector, Some(selected_match.clone()));
                all_resolved_errors.insert(error_selector_str, selected_match.clone());
            }

            if resolved_counter > 0 {
                trace.br(func_analysis_trace);
                let error_trace = trace.add_info(
                    func_analysis_trace,
                    line!(),
                    format!(
                        "resolved {} error signatures from {} selectors.",
                        resolved_counter,
                        analyzed_function.errors.len()
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
                analyzed_function
                    .events
                    .iter()
                    .map(|(event_selector, _)| {
                        encode_hex_reduced(*event_selector).replacen("0x", "", 1)
                    })
                    .collect(),
                &logger,
            );
            for (event_selector, (_, raw_event)) in analyzed_function.events.clone() {
                let mut selected_event_index: u8 = 0;
                let event_selector_str =
                    encode_hex_reduced(event_selector.clone()).replacen("0x", "", 1);
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
                    decompilation_progress.suspend(|| {
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
                        None => {
                            logger.error("invalid selection.");
                            std::process::exit(1)
                        }
                    };

                resolved_counter += 1;
                analyzed_function
                    .events
                    .insert(event_selector, (Some(selected_match.clone()), raw_event));
                all_resolved_events.insert(event_selector_str, selected_match.clone());
            }

            if resolved_counter > 0 {
                let event_trace = trace.add_info(
                    func_analysis_trace,
                    line!(),
                    format!(
                        "resolved {} event signatures from {} selectors.",
                        resolved_counter,
                        analyzed_function.events.len()
                    )
                    .to_string(),
                );

                for resolved_event in all_resolved_events.values() {
                    trace.add_message(event_trace, line!(), vec![resolved_event.signature.clone()]);
                }
            }
        }

        // get a new progress bar
        decompilation_progress = ProgressBar::new_spinner();
        decompilation_progress.enable_steady_tick(Duration::from_millis(100));
        decompilation_progress.set_style(logger.info_spinner());

        analyzed_functions.push(analyzed_function.clone());
    }
    decompilation_progress.finish_and_clear();
    logger.info("symbolic execution completed.");
    logger.info("building decompilation output.");

    // create the decompiled source output
    if args.include_yul {
        out::yul::output(
            &args,
            output_dir,
            analyzed_functions,
            all_resolved_events,
            &logger,
            &mut trace,
            decompile_call,
        );
    } else {
        out::solidity::output(
            &args,
            output_dir,
            analyzed_functions,
            all_resolved_errors,
            all_resolved_events,
            &logger,
            &mut trace,
            decompile_call,
        );
    }

    trace.display();
    logger.debug(&format!("decompilation completed in {:?}.", now.elapsed()));
}

/// Builder pattern for using decompile method as a library.
///
/// Default values may be overriden individually.
/// ## Example
/// Use with normal settings:
/// ```no_run
/// # use crate::heimdall::decompile::DecompileBuilder;
/// const SOURCE: &'static str = "7312/* snip */04ad";
///
/// DecompileBuilder::new(SOURCE)
///     .decompile();
/// ```
/// Or change settings individually:
/// ```no_run
/// # use crate::heimdall::decompile::DecompileBuilder;
///
/// const SOURCE: &'static str = "7312/* snip */04ad";
/// DecompileBuilder::new(SOURCE)
///     .default(false)
///     .include_sol(false)
///     .output("my_contract_dir")
///     .rpc("https://127.0.0.1:8545")
///     .skip_resolving(true)
///     .verbosity(5)
///     .decompile();
/// ```
#[allow(dead_code)]
pub struct DecompileBuilder {
    args: DecompilerArgs,
}

impl DecompileBuilder {
    /// A new builder for the decompilation of the specified target.
    ///
    /// The target may be a file, bytecode, contract address, or ENS name.
    #[allow(dead_code)]
    pub fn new(target: &str) -> Self {
        DecompileBuilder {
            args: DecompilerArgs {
                target: target.to_string(),
                verbose: clap_verbosity_flag::Verbosity::new(0, 0),
                output: String::from(""),
                rpc_url: String::from(""),
                default: true,
                skip_resolving: false,
                include_solidity: true,
                include_yul: false,
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
    pub fn verbosity(mut self, level: i8) -> DecompileBuilder {
        // Calculated by the log library as: 1 + verbose - quiet.
        // Set quiet as 1, and the level corresponds to the appropriate Log level.
        self.args.verbose = clap_verbosity_flag::Verbosity::new(level, 0);
        self
    }

    /// The output directory to write the decompiled files to
    #[allow(dead_code)]
    pub fn output(mut self, directory: &str) -> DecompileBuilder {
        self.args.output = directory.to_string();
        self
    }

    /// The RPC provider to use for fetching target bytecode.
    #[allow(dead_code)]
    pub fn rpc(mut self, url: &str) -> DecompileBuilder {
        self.args.rpc_url = url.to_string();
        self
    }

    /// When prompted, always select the default value.
    #[allow(dead_code)]
    pub fn default(mut self, accept: bool) -> DecompileBuilder {
        self.args.default = accept;
        self
    }

    /// Whether to skip resolving function selectors.
    #[allow(dead_code)]
    pub fn skip_resolving(mut self, skip: bool) -> DecompileBuilder {
        self.args.skip_resolving = skip;
        self
    }

    /// Whether to include solidity source code in the output (in beta).
    #[allow(dead_code)]
    pub fn include_sol(mut self, include: bool) -> DecompileBuilder {
        self.args.include_solidity = include;
        self
    }

    /// Whether to include yul source code in the output (in beta).
    #[allow(dead_code)]
    pub fn include_yul(mut self, include: bool) -> DecompileBuilder {
        self.args.include_yul = include;
        self
    }

    /// Starts the decompilation.
    #[allow(dead_code)]
    pub fn decompile(self) {
        decompile(self.args)
    }
}
