use std::collections::HashMap;

use ethers::types::{Transaction, TransactionTrace};
use heimdall_common::{
    resources::transpose::get_label,
    utils::{
        hex::ToLowerHex,
        io::{logging::TraceFactory, types::Parameterize},
    },
};

use crate::{decode::DecodeArgsBuilder, error::Error, inspect::InspectArgs};

/// Converts raw [`TransactionTrace`]s to human-readable [`TraceFactory`]
pub async fn build_trace_display(
    args: &InspectArgs,
    transaction: &Transaction,
    transaction_traces: Vec<TransactionTrace>,
    address_labels: &mut HashMap<String, Option<String>>,
) -> Result<TraceFactory, crate::error::Error> {
    let mut trace = TraceFactory::default();
    let decode_call = trace.add_call_with_extra(
        0,
        transaction.gas.as_u32(), // panicky
        "heimdall".to_string(),
        "inspect".to_string(),
        vec![transaction.hash.to_lower_hex()],
        "()".to_string(),
        vec![format!("{} wei", transaction.value)],
    );

    let mut trace_indices = HashMap::new();

    for transaction_trace in transaction_traces {
        let trace_address = transaction_trace
            .trace_address
            .iter()
            .map(|address| address.to_string())
            .collect::<Vec<_>>()
            .join(".");
        let parent_address = trace_address
            .split('.')
            .take(trace_address.split('.').count() - 1)
            .collect::<Vec<_>>()
            .join(".");

        // get trace index from parent_address
        let parent_index = trace_indices.get(&parent_address).unwrap_or(&decode_call);

        // get result
        let mut result_str = "()".to_string();
        if let Some(result) = transaction_trace.result {
            result_str = match result {
                ethers::types::Res::Call(res) => {
                    // we can attempt to decode this as if it is calldata, we just need to add some
                    // 4byte prefix.
                    let output =
                        format!("0x00000000{}", res.output.to_string().replacen("0x", "", 1));
                    let result = crate::decode::decode(
                        DecodeArgsBuilder::new()
                            .target(output)
                            .skip_resolving(true)
                            .build()
                            .map_err(|_e| Error::DecodeError)?,
                    )
                    .await?;

                    // get first result
                    if let Some(resolved_function) = result.first() {
                        resolved_function
                            .decoded_inputs
                            .clone()
                            .unwrap_or_default()
                            .iter()
                            .map(|token| token.parameterize())
                            .collect::<Vec<String>>()
                            .join(", ")
                    } else {
                        res.output.to_string()
                    }
                }
                ethers::types::Res::Create(res) => res.address.to_lower_hex(),
                ethers::types::Res::None => "()".to_string(),
            }
        }
        if result_str.replacen("0x", "", 1).is_empty() {
            result_str = "()".to_string();
        }

        // get action
        match transaction_trace.action {
            ethers::types::Action::Call(call) => {
                // add address label. we will use this to display the address in the trace, if
                // available. (requires `transpose_api_key`)
                if let Some(transpose_api_key) = &args.transpose_api_key {
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        address_labels.entry(call.to.to_lower_hex())
                    {
                        e.insert(get_label(&call.to.to_lower_hex(), transpose_api_key).await);
                    }
                }
                let address_label = address_labels
                    .get(&call.to.to_lower_hex())
                    .unwrap_or(&None)
                    .clone()
                    .unwrap_or(call.to.to_lower_hex())
                    .to_string();

                // build extra_data, which will be used to display the call type and value transfer
                // information
                let mut extra_data = vec![];
                let call_type = match call.call_type {
                    ethers::types::CallType::Call => "call",
                    ethers::types::CallType::DelegateCall => "delegatecall",
                    ethers::types::CallType::StaticCall => "staticcall",
                    ethers::types::CallType::CallCode => "callcode",
                    ethers::types::CallType::None => "none",
                }
                .to_string();
                extra_data.push(call_type.clone());
                if !call.value.is_zero() {
                    extra_data.push(format!("{} wei", call.value));
                }

                // attempt to decode calldata
                let calldata = call.input.to_string();
                if !calldata.replacen("0x", "", 1).is_empty() {
                    let result = crate::decode::decode(
                        DecodeArgsBuilder::new()
                            .target(calldata)
                            .build()
                            .map_err(|_e| Error::DecodeError)?,
                    )
                    .await?;

                    // get first result
                    if let Some(resolved_function) = result.first() {
                        // convert decoded inputs Option<Vec<Token>> to Vec<Token>
                        let decoded_inputs =
                            resolved_function.decoded_inputs.clone().unwrap_or_default();

                        // get index of parent
                        let parent_index = trace.add_call_with_extra(
                            *parent_index,
                            call.gas.as_u32(), // panicky
                            address_label,
                            resolved_function.name.clone(),
                            vec![decoded_inputs
                                .iter()
                                .map(|token| token.parameterize())
                                .collect::<Vec<String>>()
                                .join(", ")],
                            result_str,
                            extra_data,
                        );

                        // add trace_address to trace_indices
                        trace_indices.insert(trace_address.clone(), parent_index);
                    } else {
                        // get index of parent
                        let parent_index = trace.add_call_with_extra(
                            *parent_index,
                            call.gas.as_u32(), // panicky
                            address_label,
                            "unknown".to_string(),
                            vec![format!("bytes: {}", call.input.to_string())],
                            result_str,
                            extra_data,
                        );

                        // add trace_address to trace_indices
                        trace_indices.insert(trace_address.clone(), parent_index);
                    }
                } else {
                    // value transfer
                    trace.add_call_with_extra(
                        *parent_index,
                        call.gas.as_u32(), // panicky
                        call.to.to_lower_hex(),
                        "fallback".to_string(),
                        vec![],
                        result_str,
                        extra_data,
                    );
                }
            }
            ethers::types::Action::Create(create) => {
                // add address label. we will use this to display the address in the trace, if
                // available. (requires `transpose_api_key`)
                if let Some(transpose_api_key) = &args.transpose_api_key {
                    if !address_labels.contains_key(&result_str) {
                        address_labels.insert(
                            result_str.clone(),
                            get_label(&result_str, transpose_api_key).await,
                        );
                    }
                }
                let address_label = address_labels
                    .get(&result_str)
                    .unwrap_or(&None)
                    .clone()
                    .unwrap_or("NewContract".to_string())
                    .to_string();

                trace.add_creation(
                    *parent_index,
                    create.gas.as_u32(),
                    address_label,
                    result_str,
                    create.init.len().try_into().map_err(|_e| Error::DecodeError)?,
                );
            }
            ethers::types::Action::Suicide(_suicide) => {}
            ethers::types::Action::Reward(_) => todo!(),
        }
    }

    Ok(trace)
}
