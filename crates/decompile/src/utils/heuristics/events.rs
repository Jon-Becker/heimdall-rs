use alloy::primitives::U256;
use eyre::OptionExt;
use futures::future::BoxFuture;
use heimdall_common::utils::hex::ToLowerHex;
use heimdall_vm::core::vm::State;

use crate::{
    core::analyze::{AnalyzerState, AnalyzerType},
    interfaces::AnalyzedFunction,
    Error,
};

pub(crate) fn event_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        if (0xA0..=0xA4).contains(&state.last_instruction.opcode) {
            // this should be the last event in state
            let event = state.events.last().ok_or_eyre("no events in state")?;
            let selector = event.topics.first().unwrap_or(&U256::ZERO).to_owned();
            let anonymous = selector == U256::ZERO;

            // insert this selector into events
            function.events.insert(selector);

            // decode the data field
            let data_mem_ops = function.get_memory_range(
                state.last_instruction.inputs[0],
                state.last_instruction.inputs[1],
            );
            let data_mem_ops_solidified = data_mem_ops
                .iter()
                .map(|x| x.operation.solidify())
                .collect::<Vec<String>>()
                .join(", ");

            // add the event emission to the function's logic
            if analyzer_state.analyzer_type == AnalyzerType::Solidity {
                function.logic.push(format!(
                    "emit Event_{}({}{});{}",
                    &event
                        .topics
                        .first()
                        .unwrap_or(&U256::ZERO)
                        .to_lower_hex()
                        .replacen("0x", "", 1)[0..8],
                    event
                        .topics
                        .get(1..)
                        .map(|topics| {
                            let mut solidified_topics: Vec<String> = Vec::new();
                            for (i, _) in topics.iter().enumerate() {
                                solidified_topics.push(
                                    state.last_instruction.input_operations[i + 3].solidify(),
                                );
                            }

                            if !event.data.is_empty() && !topics.is_empty() {
                                format!("{}, ", solidified_topics.join(", "))
                            } else {
                                solidified_topics.join(", ")
                            }
                        })
                        .unwrap_or_else(|| "".to_string()),
                    data_mem_ops_solidified,
                    if anonymous { " // anonymous event" } else { "" }
                ));
            }
        }

        Ok(())
    })
}
