use alloy::primitives::U256;
use heimdall_vm::core::opcodes::WrappedOpcode;

use crate::{
    core::ir::{Expr, Statement},
    interfaces::StorageFrame,
};

/// Detects the usage of precompiled contracts within the EVM. Whenever an internal call is found
/// within symbolic execution traces, this function will attempt to detect if the call is to a
/// precompiled contract. It is relatively trivial to do this, as calls to specific addresses (i.e,
/// `0x..01`), are precompiled contracts.
/// Once a precompile has been detected, this function returns a structured source-level call.
/// TODO: move to common
pub(crate) fn decode_precompile(
    precompile_address: U256,
    extcalldata_memory: &[StorageFrame],
    return_data_offset: &WrappedOpcode,
) -> Option<Statement> {
    let address: usize = precompile_address.try_into().unwrap_or(usize::MAX);
    let (ty, callee) = match address {
        1 => ("address", "ecrecover"),
        2 => ("bytes", "sha256"),
        3 => ("bytes", "ripemd160"),
        _ => return None,
    };

    Some(Statement::DeclareAssign {
        ty: ty.to_string(),
        target: Expr::index("memory", Expr::from_opcode(return_data_offset)),
        value: Expr::Call {
            callee: callee.to_string(),
            args: extcalldata_memory
                .iter()
                .map(|frame| Expr::from_opcode(&frame.operation))
                .collect(),
        },
    })
}
