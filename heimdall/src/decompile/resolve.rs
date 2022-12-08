use std::{collections::HashMap, time::Duration};
use std::sync::{Arc, Mutex};
use std::thread;
use heimdall_common::{
    ether::signatures::{resolve_function_signature, ResolvedFunction},
    io::logging::Logger,
};
use indicatif::ProgressBar;

use super::util::Function;

// resolve a list of function selectors to their possible signatures
pub fn resolve_function_selectors(
    selectors: Vec<String>,
    logger: &Logger,
) -> HashMap<String, Vec<ResolvedFunction>> {
    let resolved_functions: Arc<Mutex<HashMap<String, Vec<ResolvedFunction>>>> = Arc::new(Mutex::new(HashMap::new()));
    let resolve_progress: Arc<Mutex<ProgressBar>> = Arc::new(Mutex::new(ProgressBar::new_spinner()));

    let mut threads = Vec::new();

    resolve_progress.lock().unwrap().enable_steady_tick(Duration::from_millis(100));
    resolve_progress.lock().unwrap().set_style(logger.info_spinner());

    for selector in selectors {
        let function_clone = resolved_functions.clone();
        let resolve_progress = resolve_progress.clone();

        // create a new thread for each selector
        threads.push(thread::spawn(move || {
            match resolve_function_signature(&selector) {
                Some(function) => {
                    let mut _resolved_functions = function_clone.lock().unwrap();
                    let mut _resolve_progress = resolve_progress.lock().unwrap();
                    _resolve_progress.set_message(format!("resolved {} selectors.", _resolved_functions.len()));
                    _resolved_functions.insert(selector, function);
                }
                None => {},
            }
        }));
        
    }

    // wait for all threads to finish
    for thread in threads {
        thread.join().unwrap();
    }

    resolve_progress.lock().unwrap().finish_and_clear();

    let x = resolved_functions.lock().unwrap().clone();
    x
}

// match the ResolvedFunction to a list of Function parameters
pub fn match_parameters(
    resolved_functions: Vec<ResolvedFunction>,
    function: &Function,
) -> Vec<ResolvedFunction> {

    let mut matched_functions: Vec<ResolvedFunction> = Vec::new();

    for mut resolved_function in resolved_functions {

        // skip checking if length of parameters is different
        resolved_function.inputs.retain(|x| x != "");
        if resolved_function.inputs.len() == function.arguments.len() {
            let mut matched = true;

            // check each parameter type against a list of potential types
            for (index, input) in resolved_function.inputs.iter().enumerate() {
                match function.arguments.get(&index) {
                    Some((_, potential_types)) => {

                        // arrays are typically recorded as bytes by the decompiler's potential types
                        if input.contains("[]") {
                            if !potential_types.contains(&"bytes".to_string()) {
                                continue;
                            }
                        } 
                        else if !potential_types.contains(&input) {
                            matched = false;
                            break;
                        }
                    }
                    None => {

                        // parameter not found
                        matched = false;
                        break;
                    }
                }
            }
            if matched {
                matched_functions.push(resolved_function);
            }
        }
    }

    matched_functions
}
