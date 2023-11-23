use std::collections::HashMap;

use heimdall_common::{
    ether::signatures::{ResolvedError, ResolvedLog},
    utils::strings::encode_hex_reduced,
};

use crate::snapshot::structures::snapshot::Snapshot;

/// Write the snapshot data to a CSV file
pub fn generate_csv(
    snapshots: &Vec<Snapshot>,
    resolved_errors: &HashMap<String, ResolvedError>,
    resolved_events: &HashMap<String, ResolvedLog>,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    // add header
    lines.push(
        vec![
            "Function Selector",
            "Resolved Function Signature",
            "Payable",
            "View",
            "Pure",
            "Returns",
            "Entry Point",
            "Branch Count",
            "Emitted Events",
            "Custom Errors",
            "Storage Slots",
            "Strings",
            "Hardcoded Addresses",
            "Minimum Gas Used",
            "Maximum Gas Used",
            "Average Gas Used",
            "External Calls Made",
            "Control Statements",
        ]
        .join(","),
    );

    for snapshot in snapshots {
        let mut line = Vec::new();

        // add resolved function signature
        let mut arg_strings: Vec<String> = Vec::new();
        match &snapshot.resolved_function {
            Some(function) => {
                for (index, input) in function.inputs.iter().enumerate() {
                    arg_strings.push(format!("arg{} {}", index, input));
                }
            }
            None => {
                let mut sorted_arguments: Vec<_> = snapshot.arguments.clone().into_iter().collect();
                sorted_arguments.sort_by(|x, y| x.0.cmp(&y.0));
                for (index, (_, solidity_type)) in sorted_arguments {
                    arg_strings.push(format!("arg{} {}", index, solidity_type.first().unwrap()));
                }
            }
        };

        // build events column
        let event_column = snapshot
            .events
            .iter()
            .map(|x| {
                let key = encode_hex_reduced(*x.0).replacen("0x", "", 1);
                match resolved_events.get(&key) {
                    Some(event) => format!(" {}({})", event.name, event.inputs.join(",")),
                    None => format!(" Event_{}()", key[0..8].to_owned()),
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // build errors column
        let error_column = snapshot
            .errors
            .iter()
            .map(|x| {
                let key = encode_hex_reduced(*x.0).replacen("0x", "", 1);
                match resolved_errors.get(&key) {
                    Some(errors) => format!(" {}({})", errors.name, errors.inputs.join(",")),
                    None => format!(" Error_{}()", key[0..8].to_owned()),
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // build storage column
        let storage_column = snapshot.storage.clone().into_iter().collect::<Vec<_>>().join("\n");

        // build string column
        let strings_column = snapshot.strings.clone().into_iter().collect::<Vec<_>>().join("\n");

        // build address column
        let address_column = snapshot.addresses.clone().into_iter().collect::<Vec<_>>().join("\n");

        // build external calls column
        let external_calls_column =
            snapshot.external_calls.clone().into_iter().collect::<Vec<_>>().join("\n");

        // build control statements column
        let control_statements_column =
            snapshot.control_statements.clone().into_iter().collect::<Vec<_>>().join("\n");

        // push column values
        line.push(snapshot.selector.clone());
        line.push(match &snapshot.resolved_function {
            Some(function) => format!("\"{}({})\"", function.name, arg_strings.join(", ")),
            None => format!("\"Unresolved_{}({})\"", snapshot.selector, arg_strings.join(", ")),
        });
        line.push(snapshot.payable.to_string());
        line.push((snapshot.view && !snapshot.pure).to_string());
        line.push(snapshot.pure.to_string());
        line.push(snapshot.returns.clone().unwrap_or(String::new()));
        line.push(snapshot.entry_point.to_string());
        line.push(snapshot.branch_count.to_string());
        line.push(format!("\"{event_column}\""));
        line.push(format!("\"{error_column}\""));
        line.push(format!("\"{storage_column}\""));
        line.push(format!("\"{strings_column}\""));
        line.push(format!("\"{address_column}\""));
        line.push(snapshot.gas_used.min.to_string());
        line.push(snapshot.gas_used.max.to_string());
        line.push(snapshot.gas_used.avg.to_string());
        line.push(format!("\"{external_calls_column}\""));
        line.push(format!("\"{control_statements_column}\""));

        lines.push(line.join(","));
    }

    lines
}
