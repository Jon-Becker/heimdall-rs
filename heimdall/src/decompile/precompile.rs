use ethers::types::U256;
use heimdall_common::ether::evm::opcodes::WrappedOpcode;

use super::util::StorageFrame;

// detects the usage of precompiled contracts within the EVM
pub fn decode_precompile(
    precompile_address: U256,
    extcalldata_memory: Vec<StorageFrame>,
    return_data_offset: WrappedOpcode,
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
                    .map(|x| x.operations.solidify())
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
                    .map(|x| x.operations.solidify())
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
                    .map(|x| x.operations.solidify())
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        _ => {}
    }

    (is_ext_call_precompile, ext_call_logic)
}
