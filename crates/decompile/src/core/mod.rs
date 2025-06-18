pub(crate) mod analyze;
pub(crate) mod out;
pub(crate) mod postprocess;
pub(crate) mod resolve;

use alloy::primitives::Address;
use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_json_abi::JsonAbi;
use eyre::eyre;
use hashbrown::HashMap;
use heimdall_common::{
    ether::{
        compiler::detect_compiler,
        signatures::{
            cache_signatures_from_abi, score_signature, ResolvedError, ResolvedFunction,
            ResolvedLog,
        },
        types::to_type,
    },
    utils::strings::{decode_hex, encode_hex, encode_hex_reduced, StringExt},
};
use heimdall_disassembler::{disassemble, DisassemblerArgsBuilder};
use heimdall_vm::{
    core::vm::VM,
    ext::selectors::{find_function_selectors, resolve_selectors},
};
use std::time::{Duration, Instant};

use crate::{
    core::{
        analyze::{Analyzer, AnalyzerType},
        out::{build_abi, build_abi_with_details, source::build_source},
        postprocess::PostprocessOrchestrator,
        resolve::match_parameters,
    },
    error::Error,
    interfaces::{AnalyzedFunction, DecompilerArgs},
};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
/// Result of a successful decompile operation
///
/// Contains the decompiled source code (if requested) and the reconstructed ABI
/// of the contract.
pub struct DecompileResult {
    /// The decompiled source code in Solidity or Yul format (if requested)
    pub source: Option<String>,
    /// The reconstructed JSON ABI of the contract
    pub abi: JsonAbi,
    /// The extended ABI with selector and signature information
    pub abi_with_details: serde_json::Value,
}

/// Decompiles EVM bytecode into higher-level Solidity-like code
///
/// This function analyzes the bytecode of a contract through symbolic execution
/// and attempts to reconstruct the original source code or a functionally equivalent
/// representation. It also generates an ABI for the contract.
///
/// # Arguments
///
/// * `args` - Configuration parameters for the decompile operation
///
/// # Returns
///
/// A DecompileResult containing the decompiled source (if requested) and the ABI
pub async fn decompile(args: DecompilerArgs) -> Result<DecompileResult, Error> {
    // init
    let start_time = Instant::now();
    let mut all_resolved_events: HashMap<String, ResolvedLog> = HashMap::new();
    let mut all_resolved_errors: HashMap<String, ResolvedError> = HashMap::new();

    // validate arguments
    if args.include_solidity && args.include_yul {
        return Err(Error::Eyre(eyre!(
            "arguments '--include-sol' and '--include-yul' are mutually exclusive.".to_string(),
        )));
    }
    if args.llm_postprocess && args.openai_api_key.is_empty() {
        return Err(Error::Eyre(eyre!(
                "llm postprocessing requires an openai API key. please provide one using the '--openai-api-key' flag."
            )));
    }
    if !args.include_solidity && args.llm_postprocess {
        return Err(Error::Eyre(eyre!(
            "llm postprocessing requires including solidity source code. please enable the '--include-sol' flag."
        )));
    }

    let analyzer_type = AnalyzerType::from_args(args.include_solidity, args.include_yul);

    // parse and cache signatures from the ABI, if provided
    if let Some(abi_path) = args.abi.as_ref() {
        cache_signatures_from_abi(abi_path.into())
            .map_err(|e| Error::Eyre(eyre!("caching signatures from ABI failed: {}", e)))?;
    }

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let contract_bytecode = args
        .get_bytecode()
        .await
        .map_err(|e| Error::FetchError(format!("fetching target bytecode failed: {e}")))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    if contract_bytecode.is_empty() {
        return Err(Error::Eyre(eyre!("contract bytecode is empty")));
    }

    // perform versioning and compiler heuristics
    let (_compiler, _version) = detect_compiler(&contract_bytecode);

    // create a new EVM instance. we will use this for finding function selectors,
    // performing symbolic execution, and more.
    let mut evm = VM::new(
        &contract_bytecode,
        &[],
        Address::default(),
        Address::default(),
        Address::default(),
        0,
        u128::MAX,
    );

    // disassemble the contract's bytecode
    let assembly = disassemble(
        DisassemblerArgsBuilder::new()
            .target(encode_hex(&contract_bytecode))
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
        let (map, jumpdest_count) = evm
            .symbolic_exec(
                Instant::now()
                    .checked_add(Duration::from_millis(args.timeout))
                    .expect("invalid timeout"),
            )
            .map_err(|e| Error::Eyre(eyre!("symbolic execution failed: {}", e)))?;

        symbolic_execution_maps.insert("fallback".to_string(), map);
        debug!("symbolic execution (fallback) took {:?}", start_sym_exec_time.elapsed());
        debug!("'fallback' has {} unique branches", jumpdest_count);
    }

    let overall_sym_exec_time = Instant::now();
    for (selector, entry_point) in selectors {
        let start_sym_exec_time = Instant::now();
        evm.reset();
        let (map, jumpdest_count) = match evm.symbolic_exec_selector(
            &selector,
            entry_point,
            Instant::now()
                .checked_add(Duration::from_millis(args.timeout))
                .expect("invalid timeout"),
        ) {
            Ok(map) => map,
            Err(e) => {
                warn!("failed to symbolically execute '{}': {}", selector, e);
                continue;
            }
        };
        symbolic_execution_maps.insert(selector.clone(), map);
        debug!("symbolically executed '{}' in {:?}", selector, start_sym_exec_time.elapsed());
        debug!("'{}' has {} unique branches", selector, jumpdest_count);
    }
    debug!("symbolic execution took {:?}", overall_sym_exec_time.elapsed());
    info!("symbolically executed {} selectors", symbolic_execution_maps.len());

    let start_analysis_time = Instant::now();
    let handles = symbolic_execution_maps.into_iter().map(|(selector, trace_root)| {
        let mut evm_clone = evm.clone();
        async move {
            let mut analyzer = Analyzer::new(
                analyzer_type,
                args.skip_resolving,
                AnalyzedFunction::new(&selector, selector == "fallback"),
            );

            // analyze the symbolic execution trace
            let mut analyzed_function = analyzer.analyze(trace_root).await?;

            // if the function is constant, we can get the exact val
            if analyzed_function.is_constant() && !analyzed_function.fallback {
                evm_clone.reset();
                let x = evm_clone.call(&decode_hex(&selector).expect("invalid selector"), 0)?;

                let returns_param_type = analyzed_function
                    .returns
                    .as_ref()
                    .map(|ret_type| to_type(ret_type.replace("memory", "").trim()))
                    .unwrap_or(DynSolType::Bytes);

                let decoded = returns_param_type
                    .abi_decode(&x.returndata)
                    .map(|decoded| match decoded {
                        DynSolValue::String(s) => format!("\"{s}\""),
                        DynSolValue::Uint(x, _) => x.to_string(),
                        DynSolValue::Int(x, _) => x.to_string(),
                        token => format!("0x{token:?}"),
                    })
                    .unwrap_or_else(|_| encode_hex(&x.returndata));

                analyzed_function.constant_value = Some(decoded);
            }

            Ok::<_, Error>(analyzed_function)
        }
    });
    let mut analyzed_functions = futures::future::try_join_all(handles).await?;

    debug!("analyzing symbolic execution results took {:?}", start_analysis_time.elapsed());
    info!("analyzed {} symbolic execution traces", analyzed_functions.len());

    // resolve event and error selectors
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
                .into_iter()
                .map(|(k, mut potential_values)| {
                    // sort by score, take the highest
                    potential_values.sort_by(|a: &ResolvedError, b: &ResolvedError| {
                        let a_score = score_signature(&a.signature, None);
                        let b_score = score_signature(&b.signature, None);
                        b_score.cmp(&a_score)
                    });

                    (k, potential_values.remove(0))
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
                .into_iter()
                .map(|(k, mut potential_values)| {
                    // sort by score, take the highest
                    potential_values.sort_by(|a: &ResolvedLog, b: &ResolvedLog| {
                        let a_score = score_signature(&a.signature, None);
                        let b_score = score_signature(&b.signature, None);
                        b_score.cmp(&a_score)
                    });

                    (k, potential_values.remove(0))
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

    // match analyzed parameters with resolved signatures for each function
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
            let a_score = score_signature(&a.signature, None);
            let b_score = score_signature(&b.signature, None);
            b_score.cmp(&a_score)
        });

        f.resolved_function = matched_resolved_functions.first().cloned();
        debug!(
            "using signature '{}' for '{}'",
            f.resolved_function.as_ref().map(|r| &r.signature).unwrap_or(&String::new()),
            f.selector
        );
    });

    // get a new PostprocessorOrchestrator
    // note: this will do nothing if the include_solidity and include_yul flags are false
    let mut postprocessor = PostprocessOrchestrator::new(analyzer_type)?;
    let states = analyzed_functions
        .iter_mut()
        .filter_map(|f| {
            postprocessor.postprocess(f).map_err(|e| f.notices.push(e.to_string())).ok()
        })
        .collect::<Vec<_>>();

    let storage_variables = states
        .iter()
        .flat_map(|s| s.storage_type_map.iter())
        .chain(states.iter().flat_map(|s| s.transient_type_map.iter()))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<String, String>>();

    // construct the abi for the given analyzed functions
    let abi = build_abi(&analyzed_functions, &all_resolved_errors, &all_resolved_events)?;
    let abi_with_details = build_abi_with_details(&abi, &analyzed_functions)?;
    let source = build_source(
        &analyzed_functions,
        &all_resolved_errors,
        &all_resolved_events,
        &storage_variables,
        args.llm_postprocess,
        args.openai_api_key,
    )
    .await?;

    debug!("decompilation took {:?}", start_time.elapsed());

    Ok(DecompileResult { source, abi, abi_with_details })
}
