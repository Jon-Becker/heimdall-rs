use std::collections::HashMap;

use super::structures::snapshot::Snapshot;
use crate::error::Error;
use heimdall_common::{
    ether::{
        selectors::resolve_selectors,
        signatures::{score_signature, ResolvedError, ResolvedFunction, ResolvedLog},
    },
    utils::{io::logging::TraceFactory, strings::encode_hex_reduced},
};
use indicatif::ProgressBar;
use tracing::{trace, warn};

/// Given a list of potential [`ResolvedFunction`]s and a [`Snapshot`], return a list of
/// [`ResolvedFunction`]s (that is, resolved signatures that were found on a 4byte directory) that
/// match the parameters found during symbolic execution for said [`Snapshot`].
pub fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &Snapshot,
) -> Vec<ResolvedFunction> {
    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();
    for mut resolved_function in resolved_functions {
        trace!(
            "checking function {}({}) against Unresolved_0x{}({})",
            &resolved_function.name,
            &resolved_function.inputs.join(","),
            &function.selector,
            &function
                .arguments
                .values()
                .map(|(_, potential_types)| potential_types
                    .first()
                    .expect("impossible case: argument has no potential types")
                    .clone())
                .collect::<Vec<String>>()
                .join(",")
        );
        // skip checking if length of parameters list is less than the resolved functions inputs
        resolved_function.inputs.retain(|x| !x.is_empty());
        let mut matched = true;

        // check each parameter type against a list of potential types
        for (index, input) in resolved_function.inputs.iter().enumerate() {
            trace!("    checking for parameter {} with type {}", &index.to_string(), &input);
            match function.arguments.get(&index) {
                Some((_, potential_types)) => {
                    // arrays are typically recorded as bytes by the decompiler's potential
                    // types
                    if input.contains("[]") {
                        if !potential_types.contains(&"bytes".to_string()) {
                            trace!(
                                "        parameter {} does not match type {} for function {}({})",
                                &index.to_string(),
                                &input,
                                &resolved_function.name,
                                &resolved_function.inputs.join(",")
                            );
                            continue;
                        }
                    } else if !potential_types.contains(input) {
                        matched = false;
                        trace!(
                            "        parameter {} does not match type {} for function {}({})",
                            &index.to_string(),
                            &input,
                            &resolved_function.name,
                            &resolved_function.inputs.join(",")
                        );
                        break;
                    }
                }
                None => {
                    // parameter not found
                    matched = false;
                    trace!(
                        "        parameter {} not found for function {}({})",
                        &index.to_string(),
                        &resolved_function.name,
                        &resolved_function.inputs.join(",")
                    );
                    break;
                }
            }
        }

        trace!("    matched: {}", &matched.to_string());
        if matched {
            matched_functions.push(resolved_function);
        }
    }

    matched_functions
}

// Given a [`Snapshot`], resolve all the errors, functions and events signatures
pub async fn resolve_signatures(
    snapshot: &mut Snapshot,
    all_resolved_errors: &mut HashMap<String, ResolvedError>,
    all_resolved_events: &mut HashMap<String, ResolvedLog>,
    snapshot_progress: &mut ProgressBar,
    trace: &mut TraceFactory,
    selector: &str,
    resolved_selectors: &HashMap<String, Vec<ResolvedFunction>>,
    func_analysis_trace: u32,
) -> Result<(), Error> {
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
            &func_analysis_trace,
            trace,
        )
        .await?;
    }

    snapshot_progress.finish_and_clear();

    let mut resolved_counter = 0;
    resolve_error_signatures(snapshot, all_resolved_errors, &mut resolved_counter).await?;

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
    resolve_event_signatures(
        snapshot,
        all_resolved_events,
        snapshot_progress,
        &mut resolved_counter,
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

    func_analysis_trace: &u32,
    trace: &mut TraceFactory,
) -> Result<(), Error> {
    // sort matches by signature using score heuristic from `score_signature`
    matched_resolved_functions.sort_by(|a, b| {
        let a_score = score_signature(&a.signature);
        let b_score = score_signature(&b.signature);
        b_score.cmp(&a_score)
    });

    if matched_resolved_functions.len() > 1 {
        warn!("multiple possible matches found. as of 0.8.0, heimdall uses a heuristic to select the best match.");
    }

    let selected_match = matched_resolved_functions.get(0).expect("no resolved functions matched");

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
    all_resolved_errors: &mut HashMap<String, ResolvedError>,
    resolved_counter: &mut i32,
) -> Result<(), Error> {
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
            warn!("multiple possible matches found. as of 0.8.0, heimdall uses a heuristic to select the best match.");
        }

        let selected_match = match resolved_error_selectors.get(0) {
            Some(selected_match) => selected_match,
            None => continue,
        };

        *resolved_counter += 1;

        snapshot.errors.insert(error_selector, Some(selected_match.clone()));
        all_resolved_errors.insert(error_selector_str, selected_match.clone());
    }

    Ok(())
}

async fn resolve_event_signatures(
    snapshot: &mut Snapshot,
    all_resolved_events: &mut HashMap<String, ResolvedLog>,
    snapshot_progress: &ProgressBar,
    resolved_counter: &mut i32,
) -> Result<(), Error> {
    let resolved_events: HashMap<String, Vec<ResolvedLog>> = resolve_selectors(
        snapshot
            .events
            .keys()
            .map(|event_selector| encode_hex_reduced(*event_selector).replacen("0x", "", 1))
            .collect(),
    )
    .await;

    for (event_selector, (_, raw_event)) in snapshot.events.clone() {
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
                warn!("multiple possible matches found. as of 0.8.0, heimdall uses a heuristic to select the best match.");
            });
        }

        let selected_match = match resolved_event_selectors.get(0) {
            Some(selected_match) => selected_match,
            None => continue,
        };

        *resolved_counter += 1;
        snapshot.events.insert(event_selector, (Some(selected_match.clone()), raw_event));
        all_resolved_events.insert(event_selector_str, selected_match.clone());
    }

    Ok(())
}
