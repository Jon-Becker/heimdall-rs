use std::collections::HashMap;

use heimdall_common::{ether::{signatures::{ResolvedFunction, ResolvedLog, ResolvedError, score_signature}, selectors::resolve_selectors}, utils::{io::logging::{Logger, TraceFactory}, strings::encode_hex_reduced}};
use indicatif::ProgressBar;

use super::structures::snapshot::Snapshot;

/// Given a list of potential [`ResolvedFunction`]s and a [`Snapshot`], return a list of
/// [`ResolvedFunction`]s (that is, resolved signatures that were found on a 4byte directory) that
/// match the parameters found during symbolic execution for said [`Snapshot`].
pub fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &Snapshot,
) -> Vec<ResolvedFunction> {
    // get a new logger
    let logger = Logger::default();

    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();
    for mut resolved_function in resolved_functions {
        logger.debug_max(&format!(
            "checking function {}({}) against Unresolved_0x{}({})",
            &resolved_function.name,
            &resolved_function.inputs.join(","),
            &function.selector,
            &function
                .arguments
                .values()
                .map(|(_, types)| types.first().unwrap().clone())
                .collect::<Vec<String>>()
                .join(",")
        ));
        // skip checking if length of parameters list is less than the resolved functions inputs
        resolved_function.inputs.retain(|x| !x.is_empty());
        let mut matched = true;

        // check each parameter type against a list of potential types
        for (index, input) in resolved_function.inputs.iter().enumerate() {
            logger.debug_max(&format!(
                "    checking for parameter {} with type {}",
                &index.to_string(),
                &input
            ));
            match function.arguments.get(&index) {
                Some((_, potential_types)) => {
                    // arrays are typically recorded as bytes by the decompiler's potential
                    // types
                    if input.contains("[]") {
                        if !potential_types.contains(&"bytes".to_string()) {
                            logger.debug_max(&format!(
                                "        parameter {} does not match type {} for function {}({})",
                                &index.to_string(),
                                &input,
                                &resolved_function.name,
                                &resolved_function.inputs.join(",")
                            ));
                            continue
                        }
                    } else if !potential_types.contains(input) {
                        matched = false;
                        logger.debug_max(&format!(
                            "        parameter {} does not match type {} for function {}({})",
                            &index.to_string(),
                            &input,
                            &resolved_function.name,
                            &resolved_function.inputs.join(",")
                        ));
                        break
                    }
                }
                None => {
                    // parameter not found
                    matched = false;
                    logger.debug_max(&format!(
                        "        parameter {} not found for function {}({})",
                        &index.to_string(),
                        &resolved_function.name,
                        &resolved_function.inputs.join(",")
                    ));
                    break
                }
            }
        }

        logger.debug_max(&format!("    matched: {}", &matched.to_string()));
        if matched {
            matched_functions.push(resolved_function);
        }
    }

    matched_functions
}

pub async fn resolve_signatures(
    snapshot: &mut Snapshot,
    selector: &str,
    resolved_selectors: &HashMap<String, Vec<ResolvedFunction>>,
    trace: &mut TraceFactory,
    func_analysis_trace: u32,
    snapshot_progress: &mut ProgressBar,
    logger: &Logger,
    default: bool,
    all_resolved_events: &mut HashMap<String, ResolvedLog>,
    all_resolved_errors: &mut HashMap<String, ResolvedError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved_functions = match resolved_selectors.get(selector) {
        Some(func) => func.clone(),
        None => {
            trace.add_warn(func_analysis_trace, line!(), "failed to resolve function signature");
            Vec::new()
        }
    };

    let mut matched_resolved_functions = match_parameters(resolved_functions, snapshot);

    trace.br(func_analysis_trace);
    if matched_resolved_functions.is_empty() {
        trace.add_warn(
            func_analysis_trace,
            line!(),
            "no resolved signatures matched this function's parameters",
        );
    } else {
        resolve_function_signatures(
            &mut matched_resolved_functions,
            snapshot,
            snapshot_progress,
            logger,
            default,
            &func_analysis_trace,
            trace,
        )
        .await?;
    }

    snapshot_progress.finish_and_clear();

    // resolve custom error signatures
    let mut resolved_counter = 0;
    resolve_error_signatures(
        snapshot,
        snapshot_progress,
        logger,
        &mut resolved_counter,
        all_resolved_errors,
        default,
    )
    .await?;

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

    resolved_counter = 0;
    resolve_custom_events_signatures(
        snapshot,
        snapshot_progress,
        logger,
        &mut resolved_counter,
        all_resolved_events,
        default,
    )
    .await?;

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

    Ok(())
}

async fn resolve_function_signatures(
    matched_resolved_functions: &mut Vec<ResolvedFunction>,
    snapshot: &mut Snapshot,
    snapshot_progress: &mut ProgressBar,
    logger: &Logger,
    default: bool,
    func_analysis_trace: &u32,
    trace: &mut TraceFactory,
) -> Result<(), Box<dyn std::error::Error>> {
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
                matched_resolved_functions.iter().map(|x| x.signature.clone()).collect(),
                Some(0u8),
                default,
            );
        });
    }

    let selected_match = match matched_resolved_functions.get(selected_function_index as usize) {
        Some(selected_match) => selected_match,
        None => panic!(), // TODO: can this function panic? It used to be a continue
    };

    snapshot.resolved_function = Some(selected_match.clone());

    let match_trace = trace.add_info(
        *func_analysis_trace,
        line!(),
        &format!(
            "{} resolved signature{} matched this function's parameters",
            matched_resolved_functions.len(),
            if matched_resolved_functions.len() > 1 { "s" } else { "" }
        )
        .to_string(),
    );

    for resolved_function in matched_resolved_functions {
        trace.add_message(match_trace, line!(), vec![resolved_function.signature.clone()]);
    }

    Ok(())
}

async fn resolve_error_signatures(
    snapshot: &mut Snapshot,
    snapshot_progress: &mut ProgressBar,
    logger: &Logger,
    resolved_counter: &mut i32,
    all_resolved_errors: &mut HashMap<String, ResolvedError>,
    default: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
                    default,
                );
            });
        }

        let selected_match = match resolved_error_selectors.get(selected_error_index as usize) {
            Some(selected_match) => selected_match,
            None => continue,
        };

        *resolved_counter += 1;
        snapshot.errors.insert(error_selector, Some(selected_match.clone()));
        all_resolved_errors.insert(error_selector_str, selected_match.clone());
    }

    Ok(())
}

async fn resolve_custom_events_signatures(
    snapshot: &mut Snapshot,
    snapshot_progress: &mut ProgressBar,
    logger: &Logger,
    resolved_counter: &mut i32,
    all_resolved_events: &mut HashMap<String, ResolvedLog>,
    default: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
                    default,
                );
            });
        }

        let selected_match = match resolved_event_selectors.get(selected_event_index as usize) {
            Some(selected_match) => selected_match,
            None => continue,
        };

        *resolved_counter += 1;
        snapshot.events.insert(event_selector, (Some(selected_match.clone()), raw_event));
        all_resolved_events.insert(event_selector_str, selected_match.clone());
    }

    Ok(())
}
