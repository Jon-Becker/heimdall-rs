use std::{collections::HashMap, time::Duration};

use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
    io::{
        file::short_path,
        logging::{Logger, TraceFactory},
    },
    utils::strings::find_balanced_encapsulator,
};
use indicatif::ProgressBar;

use super::{
    super::{
        constants::{DECOMPILED_SOURCE_HEADER_SOL, STORAGE_ACCESS_REGEX},
        util::Function,
        DecompilerArgs,
    },
    abi::ABIStructure,
    postprocessers::solidity::postprocess,
};

pub fn build_solidity_output(
    args: &DecompilerArgs,
    abi: &Vec<ABIStructure>,
    functions: Vec<Function>,
    all_resolved_errors: HashMap<String, ResolvedError>,
    all_resolved_events: HashMap<String, ResolvedLog>,
    trace: &mut TraceFactory,
    trace_parent: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    // get a new logger
    let logger = Logger::default();

    // clone functions mutably
    let mut functions = functions;

    // get a new progress bar
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_style(logger.info_spinner());

    // write the decompiled source to file
    let mut decompiled_output: Vec<String> = Vec::new();

    // truncate target for prettier display
    let mut shortened_target = args.target.clone();
    if shortened_target.len() > 66 {
        shortened_target = shortened_target.chars().take(66).collect::<String>() +
            "..." +
            &shortened_target.chars().skip(shortened_target.len() - 16).collect::<String>();
    }

    trace.add_call(
        trace_parent,
        line!(),
        "heimdall".to_string(),
        "build_solidity_output".to_string(),
        vec![shortened_target.clone()],
        short_path(&shortened_target),
    );

    // write the header to the output file
    decompiled_output.push(DECOMPILED_SOURCE_HEADER_SOL.replace("{}", env!("CARGO_PKG_VERSION")));
    decompiled_output.push(String::from("contract DecompiledContract {"));

    // add blank line if there are events
    if abi.iter().any(|x| matches!(x, ABIStructure::Event(_))) {
        decompiled_output.push(String::from(""));
    }

    // write the contract's events
    for event in abi.iter().filter(|x| matches!(x, ABIStructure::Event(_))) {
        if let ABIStructure::Event(event) = event {
            decompiled_output.push(format!(
                "event {}({});",
                event.name,
                event
                    .inputs
                    .iter()
                    .map(|x| format!("{} {}", x.type_, x.name))
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }
    }

    // add blank line if there are errors
    if abi.iter().any(|x| matches!(x, ABIStructure::Error(_))) {
        decompiled_output.push(String::from(""));
    }

    // write the contract's errors
    for error in abi.iter().filter(|x| matches!(x, ABIStructure::Error(_))) {
        if let ABIStructure::Error(error) = error {
            decompiled_output.push(format!(
                "error {}({});",
                error.name,
                error
                    .inputs
                    .iter()
                    .map(|x| format!("{} {}", x.type_, x.name))
                    .collect::<Vec<String>>()
                    .join(", ")
            ));
        }
    }

    // check for any constants or storage getters
    for function in functions.iter_mut() {
        if function.payable || (!function.pure && !function.view) || !function.arguments.is_empty()
        {
            continue
        }

        // check for RLP encoding. very naive check, but it works for now
        if function.logic.iter().any(|line| line.contains("0x0100 *")) &&
            function.logic.iter().any(|line| line.contains("0x01) &"))
        {
            // find any storage accesses
            let joined = function.logic.join(" ");
            let storage_access = match STORAGE_ACCESS_REGEX.find(&joined).unwrap() {
                Some(x) => x.as_str(),
                None => continue,
            };

            let storage_access_loc = find_balanced_encapsulator(storage_access, ('[', ']'));

            function.logic = vec![format!(
                "return string(rlp.encodePacked(storage[{}]));",
                storage_access[storage_access_loc.0 + 1..storage_access_loc.1 - 1].to_string()
            )]
        }
    }

    for function in functions {
        progress_bar.set_message(format!("writing logic for '0x{}'", function.selector));

        // build the function's header and parameters
        let function_modifiers = format!(
            "public {}{}",
            if function.pure {
                "pure "
            } else if function.view {
                "view "
            } else {
                ""
            },
            if function.payable { "payable " } else { "" },
        );
        let function_returns = format!(
            "returns ({}) {{",
            if function.returns.is_some() {
                function.returns.clone().unwrap()
            } else {
                String::from("")
            }
        );

        let function_header = match function.resolved_function {
            Some(resolved_function) => {
                format!(
                    "function {}({}) {}{}",
                    resolved_function.name,
                    resolved_function
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(index, solidity_type)| {
                            format!(
                                "{} {}arg{}",
                                solidity_type,
                                if solidity_type.contains("[]") ||
                                    solidity_type.contains('(') ||
                                    ["string", "bytes"].contains(&solidity_type.as_str())
                                {
                                    "memory "
                                } else {
                                    ""
                                },
                                index
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                    function_modifiers,
                    if function.returns.is_some() { function_returns } else { String::from("{") },
                )
            }
            None => {
                // sort arguments by their calldata index
                let mut sorted_arguments: Vec<_> = function.arguments.clone().into_iter().collect();
                sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

                format!(
                    "function Unresolved_{}({}) {}{}",
                    function.selector,
                    sorted_arguments
                        .iter()
                        .map(|(index, (_, potential_types))| {
                            format!(
                                "{} {}arg{}",
                                potential_types[0],
                                if potential_types[0].contains("[]") ||
                                    potential_types[0].contains('(') ||
                                    ["string", "bytes"].contains(&potential_types[0].as_str())
                                {
                                    "memory "
                                } else {
                                    ""
                                },
                                index
                            )
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                    function_modifiers,
                    if function.returns.is_some() { function_returns } else { String::from("{") },
                )
            }
        };

        // print natspec header for the function
        decompiled_output.extend(vec![
            String::new(),
            format!("/// @custom:selector    0x{}", function.selector),
            format!(
                "/// @custom:name        {}",
                function_header.replace("function ", "").split('(').next().unwrap()
            ),
        ]);

        for notice in function.notices {
            decompiled_output.push(format!("/// @notice             {notice}"));
        }

        // sort arguments by their calldata index
        let mut sorted_arguments: Vec<_> = function.arguments.into_iter().collect();
        sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));

        for (index, (_, solidity_type)) in sorted_arguments {
            decompiled_output.push(format!("/// @param              arg{index} {solidity_type:?}"));
        }

        decompiled_output.push(function_header);

        // build the function's body
        decompiled_output.extend(function.logic);

        decompiled_output.push(String::from("}"));
    }

    decompiled_output.push(String::from("}"));

    progress_bar.finish_and_clear();
    Ok(postprocess(decompiled_output, all_resolved_errors, all_resolved_events, &progress_bar)
        .join("\n"))
}
