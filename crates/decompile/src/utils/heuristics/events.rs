use alloy::primitives::U256;
use eyre::OptionExt;
use futures::future::BoxFuture;
use heimdall_common::utils::hex::ToLowerHex;
use heimdall_vm::core::vm::State;

use crate::{
    core::{
        analyze::{AnalyzerState, AnalyzerType},
        ir::{Expr, Statement},
    },
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
            // add the event emission to the function's logic
            if analyzer_state.analyzer_type == AnalyzerType::Solidity {
                let mut args = event
                    .topics
                    .get(1..)
                    .map(|topics| {
                        topics
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                Expr::from_opcode(&state.last_instruction.input_operations[i + 3])
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                args.extend(data_mem_ops.iter().map(|frame| Expr::from_opcode(&frame.operation)));
                function.push_statement(Statement::Emit {
                    event: format!(
                        "Event_{}",
                        &event
                            .topics
                            .first()
                            .unwrap_or(&U256::ZERO)
                            .to_lower_hex()
                            .replacen("0x", "", 1)[0..8]
                    ),
                    args,
                    comment: anonymous.then(|| "anonymous event".to_string()),
                });
            }
        }

        Ok(())
    })
}
