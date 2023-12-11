use std::{
    borrow::BorrowMut,
    collections::{HashSet, VecDeque},
};

use async_recursion::async_recursion;
use ethers::{
    abi::Token,
    types::{
        ActionType, Address, Bytes, Call, CallResult, CallType, Create, CreateResult,
        ExecutedInstruction, Reward, Suicide, TransactionTrace, VMTrace, U256,
    },
};
use heimdall_common::ether::signatures::ResolvedFunction;
use serde::{Deserialize, Serialize};

use crate::{decode::DecodeArgsBuilder, error::Error};
use async_convert::{async_trait, TryFrom};
use futures::future::try_join_all;

use super::logs::DecodedLog;

/// Decoded Trace
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct DecodedTransactionTrace {
    pub trace_address: Vec<usize>,
    pub action: DecodedAction,
    pub action_type: ActionType,
    pub result: Option<DecodedRes>,
    pub error: Option<String>,
    pub subtraces: Vec<DecodedTransactionTrace>,
    pub logs: Vec<DecodedLog>,
}

/// Decoded Action
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged, rename_all = "lowercase")]
pub enum DecodedAction {
    /// Decoded Call
    Call(DecodedCall),
    /// Create
    Create(Create),
    /// Suicide
    Suicide(Suicide),
    /// Reward
    Reward(Reward),
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
    pub gas: U256,
    /// Input data
    pub input: Bytes,
    /// The type of the call.
    #[serde(rename = "callType")]
    pub call_type: CallType,
    /// Potential resolved function
    #[serde(rename = "resolvedFunction")]
    pub resolved_function: Option<ResolvedFunction>,
    /// Decoded inputs
    #[serde(rename = "decodedInputs")]
    pub decoded_inputs: Vec<Token>,
}

/// Decoded Response
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DecodedRes {
    /// Call
    Call(DecodedCallResult),
    /// Create
    Create(CreateResult),
    /// None
    #[default]
    None,
}

/// Call Result
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct DecodedCallResult {
    /// Gas used
    #[serde(rename = "gasUsed")]
    pub gas_used: U256,
    /// Output bytes
    pub output: Bytes,
    /// Decoded outputs
    #[serde(rename = "decodedOutputs")]
    pub decoded_outputs: Vec<Token>,
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
        let mut decoded_transaction_trace =
            decoded_transaction_traces.pop_front().ok_or(Error::DecodeError)?;
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
                current_trace = current_trace.subtraces.get_mut(index).ok_or(Error::DecodeError)?;
            }

            // Insert the decoded trace into the correct position
            if let Some(last_index) = trace_address.last() {
                current_trace.subtraces.insert(*last_index, decoded_trace);
            } else {
                return Err(Error::DecodeError); // Trace address cannot be empty here
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
            ethers::types::Action::Call(call) => DecodedAction::Call(
                <DecodedCall as async_convert::TryFrom<Call>>::try_from(call).await?,
            ),
            ethers::types::Action::Create(create) => DecodedAction::Create(create),
            ethers::types::Action::Suicide(suicide) => DecodedAction::Suicide(suicide),
            ethers::types::Action::Reward(reward) => DecodedAction::Reward(reward),
        };

        let result = match value.result {
            Some(res) => match res {
                ethers::types::Res::Call(call) => Some(DecodedRes::Call(
                    <DecodedCallResult as async_convert::TryFrom<CallResult>>::try_from(call)
                        .await?,
                )),
                ethers::types::Res::Create(create) => Some(DecodedRes::Create(create)),
                ethers::types::Res::None => Some(DecodedRes::None),
            },
            None => None,
        };

        Ok(Self {
            trace_address: value.trace_address,
            action,
            action_type: value.action_type,
            result,
            error: value.error,
            subtraces: Vec::new(), // we will build this later
            logs: Vec::new(),      // we will build this later
        })
    }
}

#[async_trait]
impl TryFrom<Call> for DecodedCall {
    type Error = crate::error::Error;

    async fn try_from(value: Call) -> Result<Self, Self::Error> {
        let calldata = value.input.to_string().replacen("0x", "", 1);
        let mut decoded_inputs = Vec::new();
        let mut resolved_function = None;

        if !calldata.is_empty() {
            let result = crate::decode::decode(
                DecodeArgsBuilder::new()
                    .target(calldata)
                    .build()
                    .map_err(|_e| Error::DecodeError)?,
            )
            .await?;

            if let Some(first_result) = result.first() {
                decoded_inputs = first_result.decoded_inputs.clone().unwrap_or_default();
                resolved_function = Some(first_result.clone());
            }
        }

        Ok(Self {
            from: value.from,
            to: value.to,
            value: value.value,
            gas: value.gas,
            input: value.input,
            call_type: value.call_type,
            resolved_function,
            decoded_inputs,
        })
    }
}

#[async_trait]
impl TryFrom<CallResult> for DecodedCallResult {
    type Error = crate::error::Error;

    async fn try_from(value: CallResult) -> Result<Self, Self::Error> {
        // we can attempt to decode this as if it is calldata, we just need to add some
        // 4byte prefix.
        let output = format!("0x00000000{}", value.output.to_string().replacen("0x", "", 1));
        let result = crate::decode::decode(
            DecodeArgsBuilder::new()
                .target(output)
                .skip_resolving(true)
                .build()
                .map_err(|_e| Error::DecodeError)?,
        )
        .await?;

        // get first result
        let decoded_outputs = if let Some(resolved_function) = result.first() {
            resolved_function.decoded_inputs.clone().unwrap_or_default()
        } else {
            vec![]
        };

        Ok(Self { gas_used: value.gas_used, output: value.output, decoded_outputs })
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
                        Token::Address(address) => addresses.insert(*address),
                        _ => false,
                    });
                }
                if include_outputs {
                    let _ = self.result.iter().map(|result| {
                        if let DecodedRes::Call(call_result) = result {
                            let _ = call_result.decoded_outputs.iter().map(|token| match token {
                                Token::Address(address) => addresses.insert(*address),
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
            DecodedAction::Suicide(suicide) => {
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
        vm_trace: VMTrace,
        parent_address: Vec<usize>,
    ) -> Result<(), Error> {
        // Track the current depth using trace_address. Initialize with the trace_address of self.
        let mut current_address = parent_address.clone();
        let mut relative_index = 0;

        // Iterate over vm_trace.ops
        for op in vm_trace.ops {
            match op.op {
                // Check if the operation is one of the LOG operations
                ExecutedInstruction::Known(ethers::types::Opcode::LOG0) |
                ExecutedInstruction::Known(ethers::types::Opcode::LOG1) |
                ExecutedInstruction::Known(ethers::types::Opcode::LOG2) |
                ExecutedInstruction::Known(ethers::types::Opcode::LOG3) |
                ExecutedInstruction::Known(ethers::types::Opcode::LOG4) => {
                    // Pop the first decoded log, this is the log that corresponds to the current
                    // operation
                    let decoded_log = decoded_logs.pop_front().ok_or(Error::DecodeError)?;

                    // add the log to the correct position in the trace
                    let mut current_trace = self.borrow_mut();
                    for &index in current_address.iter() {
                        current_trace =
                            current_trace.subtraces.get_mut(index).ok_or(Error::DecodeError)?;
                    }

                    // push decoded log into current_trace.logs
                    current_trace.logs.push(decoded_log);
                }
                _ => {}
            }

            // Handle subtraces if present
            if let Some(sub) = op.sub {
                current_address.push(relative_index);
                let _ = &self.join_logs(decoded_logs, sub, current_address.clone()).await?;
                current_address.pop();
                relative_index += 1;
            }
        }

        Ok(())
    }
}
