//! EVM opcode handlers organized by category.
//!
//! Each submodule contains handler functions for related opcodes.

/// Arithmetic operations: ADD, MUL, SUB, DIV, SDIV, MOD, SMOD, ADDMOD, MULMOD, EXP, SIGNEXTEND
pub mod arithmetic;

/// Bitwise operations: AND, OR, XOR, NOT, BYTE, SHL, SHR, SAR
pub mod bitwise;

/// Block information: COINBASE, TIMESTAMP, NUMBER, etc.
pub mod block;

/// Comparison operations: LT, GT, SLT, SGT, EQ, ISZERO
pub mod comparison;

/// Control flow: STOP, JUMP, JUMPI, JUMPDEST, PC, GAS
pub mod control;

/// Cryptographic operations: SHA3
pub mod crypto;

/// Environment information: ADDRESS, BALANCE, CALLER, CALLVALUE, CALLDATALOAD, etc.
pub mod environment;

/// Logging operations: LOG0-LOG4
pub mod logging;

/// Memory operations: MLOAD, MSTORE, MSTORE8, MSIZE, MCOPY
pub mod memory;

/// Stack operations: POP, PUSH0-PUSH32, DUP1-DUP16, SWAP1-SWAP16
pub mod stack;

/// Storage operations: SLOAD, SSTORE, TLOAD, TSTORE
pub mod storage;

/// System operations: CREATE, CALL, CALLCODE, RETURN, DELEGATECALL, STATICCALL, CREATE2, REVERT
pub mod system;
