pub(crate) mod analyze;
pub(crate) mod out;
pub(crate) mod resolve;

use alloy_json_abi::JsonAbi;
use ethers::types::H160;
use eyre::eyre;
use heimdall_common::{
    ether::{
        bytecode::get_bytecode_from_target,
        compiler::{detect_compiler},
        evm::core::vm::VM,
        selectors::{find_function_selectors, resolve_selectors},
        signatures::{score_signature, ResolvedError, ResolvedFunction, ResolvedLog},
    },
    utils::{
        io::logging::TraceFactory,
        strings::{encode_hex, encode_hex_reduced, StringExt},
        threading::run_with_timeout,
    },
};
use heimdall_disassembler::{disassemble, DisassemblerArgsBuilder};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    core::{
        analyze::{Analyzer, AnalyzerType},
        out::abi::build_abi,
        resolve::match_parameters,
    },
    error::Error,
    interfaces::{AnalyzedFunction, DecompilerArgs},
};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct DecompileResult {
    pub source: Option<String>,
    pub abi: JsonAbi,
    _trace: TraceFactory,
}

impl DecompileResult {
    pub fn display(&self) {
        self._trace.display();
    }
}

pub async fn decompile(args: DecompilerArgs) -> Result<DecompileResult, Error> {
    // init
    let _start_time = Instant::now();
    let mut all_resolved_events: HashMap<String, ResolvedLog> = HashMap::new();
    let mut all_resolved_errors: HashMap<String, ResolvedError> = HashMap::new();

    // for now, we only support one of these at a time
    if args.include_solidity && args.include_yul {
        return Err(Error::Eyre(eyre!(
            "arguments '--include-sol' and '--include-yul' are mutually exclusive.".to_string(),
        )));
    }

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let contract_bytecode = get_bytecode_from_target(&args.target, &args.rpc_url)
        .await
        .map_err(|e| Error::FetchError(format!("fetching target bytecode failed: {}", e)))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    if contract_bytecode.is_empty() {
        return Err(Error::Eyre(eyre!("contract bytecode is empty")));
    }

    // perform versioning and compiler heuristics
    let (_compiler, _version) = detect_compiler(&contract_bytecode);

    // create a new EVM instance. we will use this for finding function selectors,
    // performing symbolic execution, and more.
    let evm = VM::new(
        &contract_bytecode,
        &[],
        H160::default(),
        H160::default(),
        H160::default(),
        0,
        u128::max_value(),
    );

    // disassemble the contract's bytecode
    let assembly = disassemble(
        DisassemblerArgsBuilder::new()
            .target(encode_hex(contract_bytecode))
            .build()
            .expect("impossible case: failed to build disassembly arguments"),
    )
    .await
    .map_err(|e| Error::Eyre(eyre!("disassembling contract bytecode failed: {}", e)))?;

    // find all the function selectors in the bytecode
    let start_selectors_time = Instant::now();
    let selectors = find_function_selectors(&evm, &assembly);
    debug!("finding function selectors took {:?}", start_selectors_time.elapsed());

    // resolve selectors (if enabled)
    let resolved_selectors = match args.skip_resolving {
        true => HashMap::new(),
        false => resolve_selectors::<ResolvedFunction>(selectors.keys().cloned().collect()).await,
    };

    info!("performing symbolic execution on '{}'", args.target.truncate(64));

    let mut symbolic_execution_maps = HashMap::new();
    if selectors.is_empty() {
        warn!("discovered no function selectors in the bytecode.");
        let start_sym_exec_time = Instant::now();
        let evm_clone = evm.clone();
        let (map, jumpdest_count) = match run_with_timeout(
            move || evm_clone.symbolic_exec(),
            Duration::from_millis(args.timeout),
        ) {
            Some(map) => {
                map.map_err(|e| Error::Eyre(eyre!("symbolic execution (fallback) failed: {}", e)))?
            }
            None => {
                return Err(Error::Eyre(eyre!(
                    "symbolic execution (fallback) timed out after {}ms",
                    args.timeout
                )))
            }
        };
        symbolic_execution_maps.insert("fallback".to_string(), map);
        debug!("symbolic execution (fallback) took {:?}", start_sym_exec_time.elapsed());
        debug!("'fallback' has {} unique branches", jumpdest_count);
    }

    let overall_sym_exec_time = Instant::now();
    for (selector, entry_point) in selectors.clone() {
        let start_sym_exec_time = Instant::now();
        let mut evm_clone = evm.clone();
        let selector_clone = selector.clone();
        let (map, jumpdest_count) = match run_with_timeout(
            move || evm_clone.symbolic_exec_selector(&selector_clone, entry_point),
            Duration::from_millis(args.timeout),
        ) {
            Some(map) => map.map_err(|e| Error::Eyre(eyre!("symbolic execution failed: {}", e)))?,
            None => {
                return Err(Error::Eyre(eyre!(
                    "symbolic execution timed out after {}ms",
                    args.timeout
                )))
            }
        };
        symbolic_execution_maps.insert(selector.clone(), map);
        debug!("symbolically executed '{}' in {:?}", selector, start_sym_exec_time.elapsed());
        debug!("'{}' has {} unique branches", selector, jumpdest_count);
    }
    debug!("symbolic execution took {:?}", overall_sym_exec_time.elapsed());
    info!("symbolically executed {} selectors", symbolic_execution_maps.len());

    let start_analysis_time = Instant::now();
    let mut analyzed_functions = symbolic_execution_maps
        .into_iter()
        .map(|(selector, trace_root)| {
            let mut analyzer = Analyzer::new(
                AnalyzerType::from_args(args.include_solidity, args.include_yul),
                AnalyzedFunction::new(
                    &selector,
                    selectors
                        .get(&selector)
                        .expect("impossible case: analyzing nonexistent selector"),
                    selector == "fallback",
                ),
                trace_root,
            );

            // analyze the symbolic execution trace
            let analyzed_function = analyzer.analyze()?;
            println!("{:#?}", analyzed_function.arguments);

            Ok::<_, Error>(analyzed_function)
        })
        .collect::<Result<Vec<AnalyzedFunction>, Error>>()?;

    debug!("analyzing symbolic execution results took {:?}", start_analysis_time.elapsed());
    info!("analyzed {} symbolic execution traces", analyzed_functions.len());

    if !args.skip_resolving {
        // resolve error selectors
        let start_error_resolving_time = Instant::now();
        let mut error_selectors: Vec<String> = analyzed_functions
            .iter()
            .flat_map(|f| f.errors.iter().map(|e| encode_hex_reduced(*e).replacen("0x", "", 1)))
            .collect();
        error_selectors.dedup();
        debug!("resolving {} error signatures", error_selectors.len());
        let resolved_errors: HashMap<String, ResolvedError> =
            resolve_selectors(error_selectors.clone())
                .await
                .iter()
                .map(|(k, v)| {
                    // sort by score, take the highest
                    let mut potential_values = v.clone();
                    potential_values.sort_by(|a: &ResolvedError, b: &ResolvedError| {
                        let a_score = score_signature(&a.signature);
                        let b_score = score_signature(&b.signature);
                        b_score.cmp(&a_score)
                    });

                    (k.clone(), potential_values.remove(0))
                })
                .collect();
        debug!("resolving error signatures took {:?}", start_error_resolving_time.elapsed());
        info!(
            "resolved {} error signatures from {} selectors",
            resolved_errors.len(),
            error_selectors.len()
        );
        all_resolved_errors.extend(resolved_errors);

        // resolve event selectors
        let start_event_resolving_time = Instant::now();
        let mut event_selectors: Vec<String> = analyzed_functions
            .iter()
            .flat_map(|f| f.events.iter().map(|e| encode_hex_reduced(*e).replacen("0x", "", 1)))
            .collect();
        event_selectors.dedup();
        debug!("resolving {} event signatures", event_selectors.len());
        let resolved_events: HashMap<String, ResolvedLog> =
            resolve_selectors(event_selectors.clone())
                .await
                .iter()
                .map(|(k, v)| {
                    // sort by score, take the highest
                    let mut potential_values = v.clone();
                    potential_values.sort_by(|a: &ResolvedLog, b: &ResolvedLog| {
                        let a_score = score_signature(&a.signature);
                        let b_score = score_signature(&b.signature);
                        b_score.cmp(&a_score)
                    });

                    (k.clone(), potential_values.remove(0))
                })
                .collect();
        debug!("resolving event signaturess took {:?}", start_event_resolving_time.elapsed());
        info!(
            "resolved {} event signatures from {} selectors",
            resolved_events.len(),
            event_selectors.len()
        );
        all_resolved_events.extend(resolved_events);
    }

    analyzed_functions.iter_mut().for_each(|f| {
        let resolve_function_signatures =
            resolved_selectors.get(&f.selector).unwrap_or(&Vec::new()).to_owned();
        let mut matched_resolved_functions = match_parameters(resolve_function_signatures, f);
        debug!(
            "matched {} resolved functions for '{}'",
            matched_resolved_functions.len(),
            f.selector
        );

        matched_resolved_functions.sort_by(|a, b| {
            let a_score = score_signature(&a.signature);
            let b_score = score_signature(&b.signature);
            b_score.cmp(&a_score)
        });

        f.resolved_function = matched_resolved_functions.first().cloned();
        debug!(
            "using signature '{}' for '{}'",
            f.resolved_function.as_ref().map(|r| r.signature.clone()).unwrap_or_default(),
            f.selector
        );
    });

    // construct the abi for the given analyzed functions
    let _abi = build_abi(analyzed_functions, all_resolved_errors, all_resolved_events)?;

    todo!()
}
