use std::time::{SystemTime, UNIX_EPOCH};

use alloy::primitives::U256;
use eyre::Result;

use crate::core::{constants::COINBASE_ADDRESS, opcodes::WrappedOpcode};

use super::super::core::VM;

/// COINBASE - Get the block's beneficiary address
pub fn coinbase(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(*COINBASE_ADDRESS, operation);
    Ok(())
}

/// TIMESTAMP - Get the block's timestamp
pub fn timestamp(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    vm.stack.push(U256::from(timestamp), operation);
    Ok(())
}

/// Generic handler for block info opcodes that return 1
/// (NUMBER, PREVRANDAO, GASLIMIT, CHAINID, SELFBALANCE, BASEFEE, BLOBHASH, BLOBBASEFEE)
pub fn block_info_stub(vm: &mut VM, operation: WrappedOpcode) -> Result<()> {
    vm.stack.push(U256::from(1u8), operation);
    Ok(())
}
