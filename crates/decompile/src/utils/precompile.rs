use alloy::primitives::U256;
use heimdall_vm::core::opcodes::WrappedOpcode;

use crate::interfaces::StorageFrame;

/// Detects the usage of precompiled contracts within the EVM. Whenever an internal call is found
/// within symbolic execution traces, this function will attempt to detect if the call is to a
/// precompiled contract. It is relatively trivial to do this, as calls to specific addresses (i.e,
/// `0x..01`), are precompiled contracts.
/// Once a precompile has been detected, this function attempts to format it in a solidity-like
/// format.
/// TODO: move to common
pub fn decode_precompile(
    precompile_address: U256,
    extcalldata_memory: &[StorageFrame],
    return_data_offset: &WrappedOpcode,
) -> (bool, String) {
    // safely convert the precompile address to a usize.
    let address: usize = match precompile_address.try_into() {
        Ok(x) => x,
        Err(_) => usize::MAX,
    };
    let mut is_ext_call_precompile = false;
    let mut ext_call_logic = String::new();

    match address {
        1 => {
            is_ext_call_precompile = true;
            ext_call_logic = format!(
                "address memory[{}] = ecrecover({});",
                return_data_offset.solidify(),
                extcalldata_memory
                    .iter()
                    .map(|x| x.operation.solidify())
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        2 => {
            is_ext_call_precompile = true;
            ext_call_logic = format!(
                "bytes memory[{}] = sha256({});",
                return_data_offset.solidify(),
                extcalldata_memory
                    .iter()
                    .map(|x| x.operation.solidify())
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        3 => {
            is_ext_call_precompile = true;
            ext_call_logic = format!(
                "bytes memory[{}] = ripemd160({});",
                return_data_offset.solidify(),
                extcalldata_memory
                    .iter()
                    .map(|x| x.operation.solidify())
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        _ => {}
    }

    (is_ext_call_precompile, ext_call_logic)
}
