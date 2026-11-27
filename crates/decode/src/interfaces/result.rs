use heimdall_common::{
    ether::{
        signatures::ResolvedFunction,
        types::{parse_function_parameters, to_abi_string, to_components, DynSolValueExt},
    },
    utils::{io::logging::TraceFactory, strings::encode_hex},
};
use serde_json::json;

use crate::error::Error;

#[derive(Debug, Clone)]
/// Result of a successful decode operation
///
/// Contains the decoded function signature and parameters, as well as
/// a trace factory for displaying the result in a formatted way.
pub struct DecodeResult {
    /// The resolved function with its decoded inputs
    pub decoded: ResolvedFunction,
    /// Other supported raw interpretations when the calldata fits multiple tuple layouts.
    pub alternatives: Vec<ResolvedFunction>,
    /// Multicall results if detected
    pub multicall_results: Option<Vec<crate::utils::MulticallDecoded>>,
    pub(crate) _trace: TraceFactory,
}

impl DecodeResult {
    /// Displays the decoded function signature and parameters in a formatted way
    pub fn display(&self) {
        self._trace.display();
    }

    /// Converts the decode result to JSON, including multicall results if present
    pub fn to_json(&self) -> Result<String, Error> {
        // Helper to convert inputs to ABI format with components
        let inputs_to_abi_format = |signature: &str| -> Vec<serde_json::Value> {
            match parse_function_parameters(signature) {
                Ok(types) => {
                    types
                        .iter()
                        .enumerate()
                        .map(|(i, sol_type)| {
                            let mut param = json!({
                                "name": format!("arg{}", i),
                                "type": to_abi_string(sol_type)
                            });

                            // Add components if it's a tuple type
                            let components = to_components(sol_type);
                            if !components.is_empty() {
                                param["components"] = json!(components
                                    .iter()
                                    .enumerate()
                                    .map(|(j, comp)| {
                                        let mut comp_json = json!({
                                            "name": format!("arg{}", j),
                                            "type": comp.ty.clone()
                                        });

                                        // Recursively add nested components
                                        if !comp.components.is_empty() {
                                            comp_json["components"] = json!(comp
                                                .components
                                                .iter()
                                                .enumerate()
                                                .map(|(k, nested)| {
                                                    json!({
                                                        "name": format!("arg{}", k),
                                                        "type": nested.ty.clone()
                                                    })
                                                })
                                                .collect::<Vec<_>>());
                                        }

                                        comp_json
                                    })
                                    .collect::<Vec<_>>());
                            }

                            param
                        })
                        .collect()
                }
                Err(_) => {
                    // Fallback to simple format if parsing fails
                    vec![]
                }
            }
        };

        let function_json = |function: &ResolvedFunction| {
            json!({
                "name": function.name,
                "signature": function.signature,
                "inputs": inputs_to_abi_format(&function.signature),
                "decoded_inputs": function.decoded_inputs.as_deref().unwrap_or_default().iter()
                    .map(|input| input.serialize()).collect::<Vec<_>>()
            })
        };
        let decoded_json = |decoded: &DecodeResult| {
            let mut value = function_json(&decoded.decoded);
            if !decoded.alternatives.is_empty() {
                value["alternatives"] = decoded.alternatives.iter().map(&function_json).collect();
            }
            value
        };
        let mut result = decoded_json(self);

        // Add multicall results if present
        if let Some(multicall_results) = &self.multicall_results {
            let mut multicalls = vec![];

            for mc_result in multicall_results {
                let mut mc_json = json!({
                    "index": mc_result.index,
                    "target": mc_result.target,
                    "value": mc_result.value,
                    "calldata": format!("0x{}", encode_hex(&mc_result.calldata)),
                });

                // Add decoded result if available
                if let Some(decoded) = &mc_result.decoded {
                    mc_json["decoded"] = decoded_json(decoded);
                }

                multicalls.push(mc_json);
            }

            result["multicall_results"] = json!(multicalls);
        }

        serde_json::to_string_pretty(&result)
            .map_err(|e| Error::Eyre(eyre::eyre!("Failed to serialize to JSON: {}", e)))
    }
}
