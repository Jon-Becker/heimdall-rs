use ethers::types::U256;

use super::util::StorageFrame;

// detects the usage of precompiled contracts within the EVM
pub fn decode_precompile(
    precompile_address: U256,
    extcalldata_memory: Vec<StorageFrame>,
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
                "ecrecover({})",
                extcalldata_memory.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(", ")
            );
        }
        2 => {
            is_ext_call_precompile = true;
            ext_call_logic = format!(
                "sha256({})",
                extcalldata_memory.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(", ")
            );
        }
        3 => {
            is_ext_call_precompile = true;
            ext_call_logic = format!(
                "ripemd160({})",
                extcalldata_memory.iter().map(|x| x.operations.solidify()).collect::<Vec<String>>().join(", ")
            );
        }
        _ => {}
    }

    return (is_ext_call_precompile, ext_call_logic);
}