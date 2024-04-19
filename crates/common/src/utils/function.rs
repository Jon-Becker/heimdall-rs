use ethers::types::U256;

use crate::ether::evm::core::opcodes::WrappedOpcode;

#[derive(Clone, Debug)]
pub struct StorageFrame {
    pub value: U256,
    pub operations: WrappedOpcode,
}
