use alloy::primitives::U256;
use eyre::eyre;
use heimdall_common::utils::{hex::ToLowerHex, sync::blocking_await};
use heimdall_vm::{
    core::{opcodes::opcode_name, vm::State},
    w_gas,
};

use crate::{
    core::analyze::AnalyzerState, interfaces::AnalyzedFunction,
    utils::precompile::decode_precompile, Error,
};
use heimdall_decoder::{decode, DecodeArgsBuilder};

pub fn extcall_heuristic(
    function: &mut AnalyzedFunction,
    state: &State,
    _: &mut AnalyzerState,
) -> Result<(), Error> {
    let instruction = &state.last_instruction;

    match instruction.opcode {
        // CALL / CALLCODE
        0xf1 | 0xf2 => {
            let address = instruction.input_operations[1].solidify();
            let memory = function.get_memory_range(instruction.inputs[3], instruction.inputs[4]);
            let extcalldata = memory
                .iter()
                .map(|x| x.value.to_lower_hex().trim_start_matches("0x").to_owned())
                .collect::<Vec<String>>()
                .join("");

            let decoded = blocking_await(move || {
                let rt = tokio::runtime::Runtime::new().expect("failed to get runtime");

                rt.block_on(async {
                    decode(
                        DecodeArgsBuilder::new()
                            .target(extcalldata)
                            .raw(true)
                            .build()
                            .expect("Failed to build DecodeArgs"),
                    )
                    .await
                })
            })
            .map_err(|e| eyre!("Failed to decode extcalldata: {}", e))?;

            // build modifiers
            // - if gas is just the default (GAS()), we don't need to include it
            // - if value is just the default (0), we don't need to include it
            let mut modifiers = vec![];
            if instruction.input_operations[0] != w_gas!() {
                modifiers.push(format!("gas: {}", instruction.input_operations[0].solidify()));
            }
            if instruction.inputs[2] != U256::ZERO {
                modifiers.push(format!("value: {}", instruction.input_operations[2].solidify()));
            }
            let modifier = if modifiers.is_empty() {
                "".to_string()
            } else {
                format!("{{ {} }}", modifiers.join(", "))
            };

            // check if the external call is a precompiled contract
            if let Some(precompile_logic) =
                decode_precompile(instruction.inputs[1], &memory, &instruction.input_operations[5])
            {
                function.logic.push(precompile_logic);
            } else {
                function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).{}{}(...); // {}",
                    address,
                    modifier,
                    decoded.decoded.name,
                    opcode_name(instruction.opcode).to_lowercase(),
                ));
            }
        }

        // STATICCALL / DELEGATECALL
        0xfa | 0xf4 => {
            let gas = format!("gas: {}", instruction.input_operations[0].solidify());
            let address = instruction.input_operations[1].solidify();
            let memory = function.get_memory_range(instruction.inputs[2], instruction.inputs[3]);
            let extcalldata = memory
                .iter()
                .map(|x| x.value.to_lower_hex().trim_start_matches("0x").to_owned())
                .collect::<Vec<String>>()
                .join("");

            let decoded = blocking_await(move || {
                let rt = tokio::runtime::Runtime::new().expect("failed to get runtime");

                rt.block_on(async {
                    decode(
                        DecodeArgsBuilder::new()
                            .target(extcalldata)
                            .raw(true)
                            .build()
                            .expect("Failed to build DecodeArgs"),
                    )
                    .await
                })
            })
            .map_err(|e| eyre!("Failed to decode extcalldata: {}", e))?;

            // build the modifier w/ gas
            // if the modifier is just the default (GAS()), we don't need to include it
            let modifier = if instruction.input_operations[0] != w_gas!() {
                format!("{{ {} }}", gas)
            } else {
                "".to_string()
            };

            // check if the external call is a precompiled contract
            if let Some(precompile_logic) =
                decode_precompile(instruction.inputs[1], &memory, &instruction.input_operations[4])
            {
                function.logic.push(precompile_logic);
            } else {
                function.logic.push(format!(
                    "(bool success, bytes memory ret0) = address({}).{}{}(...); // {}",
                    address,
                    modifier,
                    decoded.decoded.name,
                    opcode_name(instruction.opcode).to_lowercase(),
                ));
            }
        }

        _ => {}
    };

    Ok(())
}

// TODO: handle skip_resolving (need to fix in inspect mod too)
// TODO: handle case where decoding fails
