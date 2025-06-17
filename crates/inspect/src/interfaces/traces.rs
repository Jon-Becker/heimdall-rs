use hashbrown::HashSet;
use std::{borrow::BorrowMut, collections::VecDeque};

use alloy::{
    dyn_abi::DynSolValue,
    primitives::{Address, Bytes, U256, U64},
    rpc::types::trace::parity::{
        Action, CallAction, CallOutput, CallType, CreateAction, CreateOutput, RewardAction,
        SelfdestructAction, StorageDelta, TraceOutput, TransactionTrace, VmTrace,
    },
};
use async_recursion::async_recursion;
use eyre::eyre;
use heimdall_common::{
    ether::{signatures::ResolvedFunction, types::DynSolValueExt},
    utils::{
        env::get_env,
        hex::ToLowerHex,
        io::{logging::TraceFactory, types::Parameterize},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::trace;

use async_convert::{async_trait, TryFrom};
use futures::future::try_join_all;
use heimdall_decoder::{decode, DecodeArgsBuilder};

use crate::error::Error;

use super::{contracts::Contracts, logs::DecodedLog};

/// Decoded Trace
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct DecodedTransactionTrace {
    pub trace_address: Vec<usize>,
    pub action: DecodedAction,
    pub result: Option<DecodedRes>,
    pub error: Option<String>,
    pub subtraces: Vec<DecodedTransactionTrace>,
    pub logs: Vec<DecodedLog>,
    pub diff: Vec<StorageDelta>,
}

/// Decoded Action
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged, rename_all = "lowercase")]
pub enum DecodedAction {
    /// Decoded Call
    Call(DecodedCall),
    /// Create
    Create(CreateAction),
    /// Suicide
    SelfDestruct(SelfdestructAction),
    /// Reward
    Reward(RewardAction),
}

/// Decoded Call
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct DecodedCall {
    /// Sender
    pub from: Address,
    /// Recipient
    pub to: Address,
    /// Transferred Value
    pub value: U256,
    /// Gas
    pub gas: U64,
    /// Input data
    pub input: Bytes,
    /// The type of the call.
    #[serde(rename = "callType")]
    pub call_type: CallType,
    /// Potential resolved function
    #[serde(rename = "resolvedFunction")]
    pub resolved_function: Option<ResolvedFunction>,
    /// Decoded inputs
    #[serde(skip)]
    pub decoded_inputs: Vec<DynSolValue>,
    #[serde(rename = "decodedInputs")]
    decoded_inputs_serializeable: Vec<Value>,
}

/// Decoded Response
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DecodedRes {
    /// Call
    Call(DecodedCallResult),
    /// Create
    Create(CreateOutput),
    /// None
    #[default]
    None,
}

/// Call Result
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct DecodedCallResult {
    /// Gas used
    #[serde(rename = "gasUsed")]
    pub gas_used: U64,
    /// Output bytes
    pub output: Bytes,
    /// Decoded outputs
    #[serde(skip)]
    pub decoded_outputs: Vec<DynSolValue>,
    #[serde(rename = "decodedOutputs")]
    decoded_outputs_serializeable: Vec<Value>,
}

#[async_trait]
impl TryFrom<Vec<TransactionTrace>> for DecodedTransactionTrace {
    type Error = crate::error::Error;

    async fn try_from(value: Vec<TransactionTrace>) -> Result<Self, Self::Error> {
        // convert each [`TransactionTrace`] to a [`DecodedTransactionTrace`]
        let handles = value.into_iter().map(|trace| {
            <DecodedTransactionTrace as async_convert::TryFrom<TransactionTrace>>::try_from(trace)
        });
        let mut decoded_transaction_traces = VecDeque::from(try_join_all(handles).await?);

        // get the first trace, this will be the one we are building.
        let mut decoded_transaction_trace = decoded_transaction_traces
            .pop_front()
            .ok_or(Error::Eyre(eyre!("No transaction trace found")))?;
        assert!(decoded_transaction_trace.trace_address.is_empty()); // sanity check

        for decoded_trace in decoded_transaction_traces {
            // trace_address is the index of the trace in the decoded_transaction_trace. for
            // example, if trace_address is  `[0]`, it'll be added to
            // `decoded_transaction_trace.subtraces` at index 0. if trace_address is `[0, 0]`, it'll
            // be added to `decoded_transaction_trace.subtraces[0].subtraces` at index 0.
            let mut current_trace = &mut decoded_transaction_trace;
            let trace_address = &decoded_trace.trace_address;

            // Iterate through the trace address, navigating through subtraces
            for &index in trace_address.iter().take(trace_address.len() - 1) {
                current_trace = current_trace
                    .subtraces
                    .get_mut(index)
                    .ok_or(Error::Eyre(eyre!("Invalid trace address: {:?}", trace_address)))?;
            }

            // Insert the decoded trace into the correct position
            if let Some(last_index) = trace_address.last() {
                current_trace.subtraces.insert(*last_index, decoded_trace);
            } else {
                return Err(Error::Eyre(eyre!("Invalid trace address")));
            }
        }

        Ok(decoded_transaction_trace)
    }
}

#[async_trait]
impl TryFrom<TransactionTrace> for DecodedTransactionTrace {
    type Error = crate::error::Error;

    async fn try_from(value: TransactionTrace) -> Result<Self, Self::Error> {
        let action = match value.action {
            Action::Call(call) => DecodedAction::Call(
                <DecodedCall as async_convert::TryFrom<CallAction>>::try_from(call).await?,
            ),
            Action::Create(create) => DecodedAction::Create(create),
            Action::Selfdestruct(suicide) => DecodedAction::SelfDestruct(suicide),
            Action::Reward(reward) => DecodedAction::Reward(reward),
        };

        let result = match value.result {
            Some(res) => match res {
                TraceOutput::Call(call) => Some(DecodedRes::Call(
                    <DecodedCallResult as async_convert::TryFrom<CallOutput>>::try_from(call)
                        .await?,
                )),
                TraceOutput::Create(create) => Some(DecodedRes::Create(create)),
            },
            None => None,
        };

        Ok(Self {
            trace_address: value.trace_address,
            action,
            result,
            error: value.error,
            subtraces: Vec::new(), // we will build this later
            logs: Vec::new(),      // we will build this later
            diff: Vec::new(),      // we will build this later
        })
    }
}

#[async_trait]
impl TryFrom<CallAction> for DecodedCall {
    type Error = crate::error::Error;

    async fn try_from(value: CallAction) -> Result<Self, Self::Error> {
        let calldata = value.input.to_string().replacen("0x", "", 1);
        let mut decoded_inputs = Vec::new();
        let resolved_function = if !calldata.is_empty() {
            let result = decode(
                DecodeArgsBuilder::new()
                    .target(calldata)
                    .skip_resolving(
                        get_env("SKIP_RESOLVING")
                            .unwrap_or_else(|| "false".to_string())
                            .parse::<bool>()
                            .unwrap_or(false),
                    )
                    .build()
                    .expect("failed to build DecodeArgs"),
            )
            .await?;

            decoded_inputs = result.decoded.decoded_inputs.clone().unwrap_or_default();
            Some(result.decoded)
        } else {
            None
        };

        Ok(Self {
            from: value.from,
            to: value.to,
            value: value.value,
            gas: alloy::primitives::U64::from(value.gas),
            input: value.input,
            call_type: value.call_type,
            resolved_function,
            decoded_inputs_serializeable: decoded_inputs.iter().map(|v| v.serialize()).collect(),
            decoded_inputs,
        })
    }
}

#[async_trait]
impl TryFrom<CallOutput> for DecodedCallResult {
    type Error = crate::error::Error;

    async fn try_from(value: CallOutput) -> Result<Self, Self::Error> {
        // we can attempt to decode this as if it is calldata, we just need to add some
        // 4byte prefix.
        let output = format!("0x00000000{}", value.output.to_string().replacen("0x", "", 1));
        let result = decode(
            DecodeArgsBuilder::new()
                .target(output)
                .skip_resolving(true)
                .build()
                .expect("failed to build DecodeArgs"),
        )
        .await?;

        // get first result
        let decoded_outputs = result.decoded.decoded_inputs.unwrap_or_default();

        Ok(Self {
            gas_used: alloy::primitives::U64::from(value.gas_used),
            output: value.output,
            decoded_outputs_serializeable: decoded_outputs.iter().map(|v| v.serialize()).collect(),
            decoded_outputs,
        })
    }
}

impl DecodedTransactionTrace {
    /// Returns a [`HashSet`] of all addresses involved in the traced transaction. if
    /// `include_inputs`/`include_outputs` is true, the [`HashSet`] will also include the
    /// addresses of the inputs/outputs of the transaction.
    pub fn addresses(&self, include_inputs: bool, include_outputs: bool) -> HashSet<Address> {
        let mut addresses = HashSet::new();

        match &self.action {
            DecodedAction::Call(call) => {
                addresses.insert(call.from);
                addresses.insert(call.to);

                if include_inputs {
                    let _ = call.decoded_inputs.iter().map(|token| match token {
                        DynSolValue::Address(address) => addresses.insert(address.to_owned()),
                        _ => false,
                    });
                }
                if include_outputs {
                    let _ = self.result.iter().map(|result| {
                        if let DecodedRes::Call(call_result) = result {
                            let _ = call_result.decoded_outputs.iter().map(|token| match token {
                                DynSolValue::Address(address) => {
                                    addresses.insert(address.to_owned())
                                }
                                _ => false,
                            });
                        }
                    });
                }
            }
            DecodedAction::Create(create) => {
                addresses.insert(create.from);

                if include_outputs {
                    let _ = self.result.iter().map(|result| {
                        if let DecodedRes::Create(create_result) = result {
                            addresses.insert(create_result.address);
                        }
                    });
                }
            }
            DecodedAction::SelfDestruct(suicide) => {
                addresses.insert(suicide.address);
                addresses.insert(suicide.refund_address);
            }
            DecodedAction::Reward(reward) => {
                addresses.insert(reward.author);
            }
        };

        // add all addresses found in subtraces
        for subtrace in &self.subtraces {
            addresses.extend(subtrace.addresses(include_inputs, include_outputs))
        }

        addresses
    }

    #[async_recursion]
    pub async fn join_logs(
        &mut self,
        decoded_logs: &mut VecDeque<DecodedLog>,
        vm_trace: &VmTrace,
        parent_address: Vec<usize>,
    ) -> Result<(), Error> {
        // Track the current depth using trace_address. Initialize with the trace_address of self.
        let mut current_address = parent_address;
        let mut relative_index = 0;

        // Iterate over vm_trace.ops
        for op in vm_trace.ops.iter() {
            match op.op.as_deref().unwrap_or_default() {
                // Check if the operation is one of the LOG operations
                "LOG0" | "LOG1" | "LOG2" | "LOG3" | "LOG4" => {
                    // Pop the first decoded log, this is the log that corresponds to the current
                    // operation
                    let decoded_log = decoded_logs
                        .pop_front()
                        .ok_or(Error::Eyre(eyre!("no decoded log found for log operation")))?;

                    // add the log to the correct position in the trace
                    let mut current_trace = self.borrow_mut();
                    for &index in current_address.iter() {
                        current_trace = current_trace
                            .subtraces
                            .get_mut(index)
                            .ok_or(Error::Eyre(eyre!("subtrace not found")))?;
                    }

                    // push decoded log into current_trace.logs
                    current_trace.logs.push(decoded_log);
                }
                _ => {}
            }

            // Handle subtraces if present
            if let Some(sub) = &op.sub {
                current_address.push(relative_index);
                let _ = &self.join_logs(decoded_logs, sub, current_address.clone()).await?;
                current_address.pop();
                relative_index += 1;
            }
        }

        Ok(())
    }

    #[async_recursion]
    pub async fn build_state_diffs(
        &mut self,
        vm_trace: VmTrace,
        parent_address: Vec<usize>,
    ) -> Result<(), Error> {
        // Track the current depth using trace_address. Initialize with the trace_address of self.
        let mut current_address = parent_address;
        let mut relative_index = 0;

        // Iterate over vm_trace.ops
        for op in vm_trace.ops {
            if let Some(ex) = op.ex {
                if let Some(store) = ex.store {
                    // add the diff to the correct position in the trace
                    let mut current_trace = self.borrow_mut();
                    for &index in current_address.iter() {
                        current_trace = current_trace
                            .subtraces
                            .get_mut(index)
                            .ok_or(Error::Eyre(eyre!("subtrace not found")))?;
                    }

                    // push decoded log into current_trace.diff
                    current_trace.diff.push(store);
                }
            }

            // Handle subtraces if present
            if let Some(sub) = op.sub {
                current_address.push(relative_index);
                let _ = &self.build_state_diffs(sub, current_address.clone()).await?;
                current_address.pop();
                relative_index += 1;
            }
        }

        Ok(())
    }

    pub fn add_to_trace(
        &self,
        contracts: &Contracts,
        trace: &mut TraceFactory,
        parent_trace_index: u32,
    ) {
        let parent_trace_index = match &self.action {
            DecodedAction::Call(call) => trace.add_call_with_extra(
                parent_trace_index,
                call.gas.try_into().unwrap_or(0),
                contracts.get(call.to).cloned().unwrap_or_else(|| call.to.to_lower_hex()),
                match call.resolved_function.as_ref() {
                    Some(f) => f.name.clone(),
                    None => "fallback".to_string(),
                },
                match call.resolved_function.as_ref() {
                    Some(f) => f
                        .decoded_inputs
                        .as_ref()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|token| token.parameterize())
                        .collect(),
                    None => vec![],
                },
                match &self.result.as_ref() {
                    Some(DecodedRes::Call(call_result)) => {
                        let outputs = call_result
                            .decoded_outputs
                            .iter()
                            .map(|token| token.parameterize())
                            .collect::<Vec<String>>();

                        if outputs.is_empty() {
                            [call_result.output.to_lower_hex()].join(", ")
                        } else {
                            outputs.join(", ")
                        }
                    }
                    _ => "".to_string(),
                },
                vec![
                    format!("{:?}", call.call_type).to_lowercase(),
                    format!("value: {} ether", wei_to_ether(call.value)),
                ],
            ),
            DecodedAction::Create(create) => trace.add_creation(
                parent_trace_index,
                create.gas.try_into().unwrap_or(0),
                "NewContract".to_string(),
                match &self.result.as_ref() {
                    Some(DecodedRes::Create(create_result)) => contracts
                        .get(create_result.address)
                        .cloned()
                        .unwrap_or_else(|| create_result.address.to_lower_hex()),
                    _ => "".to_string(),
                },
                create.init.len().try_into().unwrap_or(0),
            ),
            DecodedAction::SelfDestruct(suicide) => trace.add_suicide(
                parent_trace_index,
                0,
                suicide.address.to_lower_hex(),
                suicide.refund_address.to_lower_hex(),
                wei_to_ether(suicide.balance),
            ),
            DecodedAction::Reward(reward) => trace.add_call_with_extra(
                parent_trace_index,
                0,
                Address::ZERO.to_lower_hex(),
                "reward".to_string(),
                vec![
                    reward.author.to_lower_hex(),
                    format!("{:?}", reward.reward_type).to_lowercase(),
                ],
                "()".to_string(),
                vec![format!("value: {} ether", wei_to_ether(reward.value))],
            ),
        };

        // for each log, add to trace
        for log in &self.logs {
            if let Some(event) = &log.resolved_event {
                // TODO: ResolveLog should decode raw data
                trace.add_emission(
                    parent_trace_index,
                    log.log_index.unwrap_or(0).try_into().unwrap_or_default(),
                    &event.name,
                    &event.inputs,
                );
                trace.add_raw_emission(
                    parent_trace_index,
                    log.log_index.unwrap_or(0).try_into().unwrap_or_default(),
                    log.topics.iter().map(|topic| topic.to_lower_hex()).collect(),
                    log.data.to_lower_hex(),
                );
            } else {
                trace.add_raw_emission(
                    parent_trace_index,
                    log.log_index.unwrap_or(0).try_into().unwrap_or_default(),
                    log.topics.iter().map(|topic| topic.to_lower_hex()).collect(),
                    log.data.to_lower_hex(),
                );
            }
        }

        // for each diff, add to trace
        for diff in &self.diff {
            trace.add_message(
                parent_trace_index,
                line!(),
                vec![format!(
                    "store '{}' in slot '{}'",
                    diff.val.to_lower_hex(),
                    diff.key.to_lower_hex()
                )],
            );
        }

        // iterate over traces
        for decoded_trace in self.subtraces.iter() {
            decoded_trace.add_to_trace(contracts, trace, parent_trace_index)
        }
    }
}

fn wei_to_ether(wei: U256) -> f64 {
    // convert U256 to u64 safely
    let wei_u64: u64 = wei.min(U256::from(u64::MAX)).try_into().unwrap_or(0);
    let wei_f64 = wei_u64 as f64;

    // if wei = u64::MAX, log that it was truncated
    if wei_u64 == u64::MAX {
        trace!("WARNING: wei value was truncated to u64::MAX. Original value: {}", wei);
    }

    wei_f64 / 10f64.powi(18)
}
