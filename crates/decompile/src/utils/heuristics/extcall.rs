use alloy::primitives::U256;
use futures::future::BoxFuture;
use heimdall_common::utils::{hex::ToLowerHex, strings::encode_hex_reduced};
use heimdall_vm::{
    core::{opcodes::opcode_name, vm::State},
    w_gas, w_push0,
};
use tracing::trace;

use crate::{
    core::analyze::AnalyzerState, interfaces::AnalyzedFunction,
    utils::precompile::decode_precompile, Error,
};
use heimdall_decoder::{decode, DecodeArgsBuilder};

pub(crate) fn extcall_heuristic<'a>(
    function: &'a mut AnalyzedFunction,
    state: &'a State,
    analyzer_state: &'a mut AnalyzerState,
) -> BoxFuture<'a, Result<(), Error>> {
    Box::pin(async move {
        let instruction = &state.last_instruction;

        match instruction.opcode {
            // CALL / CALLCODE
            0xf1 | 0xf2 => {
                let address = instruction.input_operations[1].solidify();
                let memory =
                    function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);
                let extcalldata =
                    memory.iter().map(|x| x.value.to_lower_hex()).collect::<Vec<String>>().join("");
                let gas_solidified = instruction.input_operations[0].solidify();
                let value_solidified = instruction.input_operations[2].solidify();

                // if gas is 2,300, this is a value transfer
                if gas_solidified.contains("0x08fc") {
                    trace!(
                        "instruction {} ({}) with 2300 gas indicates a value transfer",
                        instruction.instruction,
                        opcode_name(instruction.opcode)
                    );
                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({address}).transfer({value_solidified});"
                    ));
                    return Ok(());
                }
                if extcalldata.is_empty() {
                    trace!(
                        "instruction {} ({}) with no calldata indicates a value transfer",
                        instruction.instruction,
                        opcode_name(instruction.opcode)
                    );
                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({address}).transfer({value_solidified});"
                    ));
                    return Ok(());
                }

                let extcalldata_clone = extcalldata.clone();
                let decoded = decode(
                    DecodeArgsBuilder::new()
                        .target(extcalldata_clone)
                        .raw(true)
                        .skip_resolving(analyzer_state.skip_resolving)
                        .build()
                        .expect("Failed to build DecodeArgs"),
                )
                .await
                .ok();

                // build modifiers
                // - if gas is just the default (GAS()), we don't need to include it
                // - if value is just the default (0), we don't need to include it
                let mut modifiers = vec![];
                if instruction.input_operations[0] != w_gas!() {
                    modifiers.push(format!("gas: {gas_solidified}"));
                }
                if instruction.input_operations[2] != w_push0!() {
                    // if the value is just a hex string, we can parse it as ether for readability
                    if let Ok(value) =
                        u128::from_str_radix(value_solidified.trim_start_matches("0x"), 16)
                    {
                        let ether_value = value as f64 / 10_f64.powi(18);
                        modifiers.push(format!("value: {ether_value} ether"));
                    } else {
                        modifiers.push(format!("value: {value_solidified}"));
                    }
                }
                let modifier = if modifiers.is_empty() {
                    "".to_string()
                } else {
                    format!("{{ {} }}", modifiers.join(", "))
                };

                // check if the external call is a precompiled contract
                if let Some(precompile_logic) = decode_precompile(
                    instruction.inputs[1],
                    &memory,
                    &instruction.input_operations[5],
                ) {
                    function.logic.push(precompile_logic);
                } else if let Some(decoded) = decoded {
                    let start_slot = instruction.inputs[3] + U256::from(4);

                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({}).{}{}({}); // {}",
                        address,
                        modifier,
                        decoded.decoded.name,
                        decoded
                            .decoded
                            .inputs
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                format!(
                                    "memory[{}]",
                                    encode_hex_reduced(start_slot + U256::from(i * 32))
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(", "),
                        opcode_name(instruction.opcode).to_lowercase(),
                    ));
                } else {
                    function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).Unresolved_{}{}(msg.data[{}:{}]); // {}",
                    address,
                    extcalldata.get(2..10).unwrap_or(""),
                    modifier,
                    instruction.input_operations[3].solidify(),
                    instruction.input_operations[4].solidify(),
                    opcode_name(instruction.opcode).to_lowercase(),
                ));
                }
            }

            // STATICCALL / DELEGATECALL
            0xfa | 0xf4 => {
                let gas = format!("gas: {}", instruction.input_operations[0].solidify());
                let address = instruction.input_operations[1].solidify();
                let memory =
                    function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);
                let extcalldata = memory
                    .iter()
                    .map(|x| x.value.to_lower_hex().trim_start_matches("0x").to_owned())
                    .collect::<Vec<String>>()
                    .join("");

                let extcalldata_clone = extcalldata.clone();
                let decoded = decode(
                    DecodeArgsBuilder::new()
                        .target(extcalldata_clone)
                        .raw(true)
                        .skip_resolving(analyzer_state.skip_resolving)
                        .build()
                        .expect("Failed to build DecodeArgs"),
                )
                .await
                .ok();

                // build the modifier w/ gas
                // if the modifier is just the default (GAS()), we don't need to include it
                let modifier = if instruction.input_operations[0] != w_gas!() {
                    format!("{{ {gas} }}")
                } else {
                    "".to_string()
                };

                // check if the external call is a precompiled contract
                if let Some(precompile_logic) = decode_precompile(
                    instruction.inputs[1],
                    &memory,
                    &instruction.input_operations[4],
                ) {
                    function.logic.push(precompile_logic);
                } else if let Some(decoded) = decoded {
                    let start_slot = instruction.inputs[2] + U256::from(4);

                    function.logic.push(format!(
                        "(bool success, bytes memory ret0) = address({}).{}{}({}); // {}",
                        address,
                        modifier,
                        decoded.decoded.name,
                        decoded
                            .decoded
                            .inputs
                            .iter()
                            .enumerate()
                            .map(|(i, _)| {
                                format!(
                                    "memory[{}]",
                                    encode_hex_reduced(start_slot + U256::from(i * 32))
                                )
                            })
                            .collect::<Vec<String>>()
                            .join(", "),
                        opcode_name(instruction.opcode).to_lowercase(),
                    ));
                } else {
                    function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).Unresolved_{}{}(memory[{}:{}]); // {}",
                    address,
                    extcalldata.get(2..10).unwrap_or(""),
                    modifier,
                    instruction.input_operations[2].solidify(),
                    instruction.input_operations[3].solidify(),
                    opcode_name(instruction.opcode).to_lowercase(),
                ));
                }
            }

            _ => {}
        };

        Ok(())
    })
}
