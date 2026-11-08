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
        let inputs_to_abi_format = |signature: &str| -> Vec<serde_json::Value> {
            match parse_function_parameters(signature) {
                Ok(types) => types
                    .iter()
                    .enumerate()
                    .map(|(i, sol_type)| {
                        let mut param = json!({
                            "name": format!("arg{}", i),
                            "type": to_abi_string(sol_type)
                        });

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
                    .collect(),
                Err(_) => {
                    vec![]
                }
            }
        };

        let mut result = json!({
            "name": self.decoded.name,
            "signature": self.decoded.signature,
            "inputs": inputs_to_abi_format(&self.decoded.signature),
            "decoded_inputs": if let Some(decoded_inputs) = &self.decoded.decoded_inputs {
                decoded_inputs
                    .iter()
                    .map(|input| input.serialize())
                    .collect::<Vec<_>>()
            } else {
                vec![]
            }
        });

        if let Some(multicall_results) = &self.multicall_results {
            let mut multicalls = vec![];

            for mc_result in multicall_results {
                let mut mc_json = json!({
                    "index": mc_result.index,
                    "target": mc_result.target,
                    "value": mc_result.value,
                    "calldata": format!("0x{}", encode_hex(&mc_result.calldata)),
                });

                if let Some(decoded) = &mc_result.decoded {
                    mc_json["decoded"] = json!({
                        "name": decoded.decoded.name,
                        "signature": decoded.decoded.signature,
                        "inputs": inputs_to_abi_format(&decoded.decoded.signature),
                        "decoded_inputs": if let Some(decoded_inputs) = &decoded.decoded.decoded_inputs {
                            decoded_inputs
                                .iter()
                                .map(|input| input.serialize())
                                .collect::<Vec<_>>()
                        } else {
                            vec![]
                        }
                    });
                }

                multicalls.push(mc_json);
            }

            result["multicall_results"] = json!(multicalls);
        }

        serde_json::to_string_pretty(&result)
            .map_err(|e| Error::Eyre(eyre::eyre!("Failed to serialize to JSON: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::MulticallDecoded;

    fn result(signature: &str) -> DecodeResult {
        DecodeResult {
            decoded: ResolvedFunction {
                name: "example".into(),
                signature: signature.into(),
                inputs: vec![],
                decoded_inputs: None,
            },
            multicall_results: None,
            _trace: TraceFactory::default(),
        }
    }

    #[test]
    fn renders_nested_tuple_components_for_calls_and_multicalls() {
        let nested = result("example((uint256,(address,uint256))[])");
        let mut outer = result("example(address)");
        outer.multicall_results = Some(vec![MulticallDecoded {
            index: 0,
            target: "0x0000000000000000000000000000000000000001".into(),
            value: None,
            calldata: vec![1, 2, 3, 4],
            decoded: Some(nested.clone()),
        }]);
        let nested_json: serde_json::Value =
            serde_json::from_str(&nested.to_json().unwrap()).unwrap();
        let outer_json: serde_json::Value =
            serde_json::from_str(&outer.to_json().unwrap()).unwrap();
        let expected = json!([{
            "name": "arg0",
            "type": "tuple[]",
            "components": [
                { "name": "arg0", "type": "uint256" },
                { "name": "arg1", "type": "tuple", "components": [
                    { "name": "arg0", "type": "address" },
                    { "name": "arg1", "type": "uint256" }
                ] }
            ]
        }]);
        assert_eq!(nested_json["inputs"], expected);
        assert_eq!(outer_json["multicall_results"][0]["decoded"]["inputs"], expected);
    }
}
