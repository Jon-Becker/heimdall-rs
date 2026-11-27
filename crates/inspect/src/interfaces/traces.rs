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
        hex::ToLowerHex,
        io::{logging::TraceFactory, types::Parameterize},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::trace;

use async_convert::{async_trait, TryFrom};
use futures::future::try_join_all;
use heimdall_decoder::{decode, decode_parameters, DecodeArgsBuilder};

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
        Self::decode_with_options(value, false).await
    }
}

impl DecodedTransactionTrace {
    pub(crate) async fn decode_with_options(
        value: Vec<TransactionTrace>,
        skip_resolving: bool,
    ) -> Result<Self, Error> {
        let handles = value
            .into_iter()
            .map(|trace| DecodedTransactionTrace::decode_one_with_options(trace, skip_resolving));
        let mut decoded_transaction_traces = VecDeque::from(try_join_all(handles).await?);

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

            for &index in trace_address.iter().take(trace_address.len() - 1) {
                current_trace = current_trace
                    .subtraces
                    .get_mut(index)
                    .ok_or(Error::Eyre(eyre!("Invalid trace address: {:?}", trace_address)))?;
            }

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
        Self::decode_one_with_options(value, false).await
    }
}

impl DecodedTransactionTrace {
    async fn decode_one_with_options(
        value: TransactionTrace,
        skip_resolving: bool,
    ) -> Result<Self, Error> {
        let action = match value.action {
            Action::Call(call) => {
                DecodedAction::Call(DecodedCall::decode_with_options(call, skip_resolving).await?)
            }
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
        Self::decode_with_options(value, false).await
    }
}

impl DecodedCall {
    async fn decode_with_options(value: CallAction, skip_resolving: bool) -> Result<Self, Error> {
        let calldata = value.input.to_string().replacen("0x", "", 1);
        let mut decoded_inputs = Vec::new();
        let resolved_function = if value.input.len() >= 4 {
            let result = decode(
                DecodeArgsBuilder::new()
                    .target(calldata)
                    .raw(true)
                    .skip_resolving(skip_resolving)
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
        let decoded_outputs = decode_parameters(&value.output)?;

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
                    for token in &call.decoded_inputs {
                        if let DynSolValue::Address(address) = token {
                            addresses.insert(*address);
                        }
                    }
                }
                if include_outputs {
                    if let Some(DecodedRes::Call(result)) = &self.result {
                        for token in &result.decoded_outputs {
                            if let DynSolValue::Address(address) = token {
                                addresses.insert(*address);
                            }
                        }
                    }
                }
            }
            DecodedAction::Create(create) => {
                addresses.insert(create.from);
                if include_outputs {
                    if let Some(DecodedRes::Create(result)) = &self.result {
                        addresses.insert(result.address);
                    }
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
        let mut current_address = parent_address;
        let mut relative_index = 0;

        for op in vm_trace.ops.iter() {
            match op.op.as_deref().unwrap_or_default() {
                "LOG0" | "LOG1" | "LOG2" | "LOG3" | "LOG4" => {
                    let decoded_log = decoded_logs
                        .pop_front()
                        .ok_or(Error::Eyre(eyre!("no decoded log found for log operation")))?;

                    let mut current_trace = self.borrow_mut();
                    for &index in current_address.iter() {
                        current_trace = current_trace
                            .subtraces
                            .get_mut(index)
                            .ok_or(Error::Eyre(eyre!("subtrace not found")))?;
                    }

                    current_trace.logs.push(decoded_log);
                }
                _ => {}
            }

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
        let mut current_address = parent_address;
        let mut relative_index = 0;

        for op in vm_trace.ops {
            if let Some(ex) = op.ex {
                if let Some(store) = ex.store {
                    let mut current_trace = self.borrow_mut();
                    for &index in current_address.iter() {
                        current_trace = current_trace
                            .subtraces
                            .get_mut(index)
                            .ok_or(Error::Eyre(eyre!("subtrace not found")))?;
                    }

                    current_trace.diff.push(store);
                }
            }

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

        for decoded_trace in self.subtraces.iter() {
            decoded_trace.add_to_trace(contracts, trace, parent_trace_index)
        }
    }
}

fn wei_to_ether(wei: U256) -> f64 {
    let wei_u64: u64 = wei.min(U256::from(u64::MAX)).try_into().unwrap_or(0);
    let wei_f64 = wei_u64 as f64;

    if wei_u64 == u64::MAX {
        trace!("WARNING: wei value was truncated to u64::MAX. Original value: {}", wei);
    }

    wei_f64 / 10f64.powi(18)
}

#[cfg(test)]
mod option_tests {
    use super::*;

    #[tokio::test]
    async fn trace_decoding_skips_signature_resolution_when_requested() {
        let root = TransactionTrace {
            action: Action::Call(CallAction {
                input: Bytes::from(vec![0x12, 0x34, 0x56, 0x78]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let trace = DecodedTransactionTrace::decode_with_options(vec![root], true).await.unwrap();
        match trace.action {
            DecodedAction::Call(call) => {
                assert_eq!(call.resolved_function.unwrap().name, "Unresolved_12345678")
            }
            _ => panic!("expected a call"),
        }
    }

    #[tokio::test]
    async fn short_call_data_is_kept_without_decoding() {
        for bytes in [vec![], vec![1], vec![1, 2, 3]] {
            let call = CallAction { input: Bytes::from(bytes.clone()), ..Default::default() };
            let decoded = DecodedCall::decode_with_options(call, false).await.unwrap();
            assert_eq!(decoded.input.as_ref(), bytes);
            assert!(decoded.resolved_function.is_none());
            assert!(decoded.decoded_inputs.is_empty());
        }
    }

    #[tokio::test]
    async fn decodes_empty_and_single_word_call_output() {
        let empty = CallOutput { gas_used: 12, output: Bytes::new() };
        let decoded = <DecodedCallResult as async_convert::TryFrom<CallOutput>>::try_from(empty)
            .await
            .unwrap();
        assert!(decoded.decoded_outputs.is_empty());
        assert!(decoded.output.is_empty());
        assert_eq!(decoded.gas_used, U64::from(12));

        let mut word = vec![0; 32];
        word[31] = 42;
        let output = CallOutput { gas_used: 23, output: Bytes::from(word.clone()) };
        let decoded = <DecodedCallResult as async_convert::TryFrom<CallOutput>>::try_from(output)
            .await
            .unwrap();
        assert_eq!(decoded.output.as_ref(), word);
        assert_eq!(decoded.decoded_outputs.len(), 1);
        assert!(
            matches!(&decoded.decoded_outputs[0], DynSolValue::Uint(value, _) if *value == U256::from(42))
        );
    }

    #[test]
    fn collects_requested_input_output_and_created_addresses() {
        let from = Address::from([1; 20]);
        let to = Address::from([2; 20]);
        let input = Address::from([3; 20]);
        let output = Address::from([4; 20]);
        let created = Address::from([5; 20]);
        let child = DecodedTransactionTrace {
            trace_address: vec![0],
            action: DecodedAction::Create(CreateAction { from, ..Default::default() }),
            result: Some(DecodedRes::Create(CreateOutput {
                address: created,
                code: Bytes::new(),
                gas_used: 0,
            })),
            error: None,
            subtraces: vec![],
            logs: vec![],
            diff: vec![],
        };
        let trace = DecodedTransactionTrace {
            trace_address: vec![],
            action: DecodedAction::Call(DecodedCall {
                from,
                to,
                decoded_inputs: vec![DynSolValue::Address(input)],
                ..Default::default()
            }),
            result: Some(DecodedRes::Call(DecodedCallResult {
                decoded_outputs: vec![DynSolValue::Address(output)],
                ..Default::default()
            })),
            error: None,
            subtraces: vec![child],
            logs: vec![],
            diff: vec![],
        };
        assert_eq!(trace.addresses(false, false), [from, to].into_iter().collect());
        assert_eq!(trace.addresses(true, false), [from, to, input].into_iter().collect());
        assert_eq!(trace.addresses(false, true), [from, to, output, created].into_iter().collect());
        assert_eq!(
            trace.addresses(true, true),
            [from, to, input, output, created].into_iter().collect()
        );
    }
}
