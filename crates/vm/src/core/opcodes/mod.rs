//! EVM opcodes and related utilities.
//!
//! This module provides functionality for working with EVM opcodes, including:
//! - Opcode information (names, gas costs, stack effects)
//! - Wrapped opcode structures for tracking data flow
//! - Various utility functions for working with opcodes
//!
//! The implementation is partially adapted from https://github.com/bluealloy/revm

/// Re-export wrapped opcode module that provides structures for tracking opcode operations
/// and their relationships in data flow analysis.
pub mod wrapped;
use paste::paste;
pub use wrapped::*;

/// Information about opcode, such as name, and stack inputs and outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpCodeInfo {
    /// Name
    name: &'static str,
    /// Stack inputs.
    inputs: u8,
    /// Stack outputs.
    outputs: u8,
    /// If the opcode stops execution. aka STOP, RETURN, ..
    terminating: bool,
    /// Minimum gas required to execute the opcode.
    gas: u16,
    /// Whether the opcode is view (does not modify state).
    view: bool,
    /// Whether the opcode is pure (does not read state).
    pure: bool,
}

impl OpCodeInfo {
    /// Creates a new opcode info with the given name and default values.
    pub const fn new(name: &'static str) -> Self {
        Self { name, inputs: 0, outputs: 0, terminating: false, gas: 0, view: true, pure: true }
    }

    /// Returns the name of the opcode.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the number of stack inputs.
    #[inline]
    pub const fn inputs(&self) -> u8 {
        self.inputs
    }

    /// Returns the number of stack outputs.
    #[inline]
    pub const fn outputs(&self) -> u8 {
        self.outputs
    }

    /// Returns whether the opcode is terminating.
    #[inline]
    pub const fn terminating(&self) -> bool {
        self.terminating
    }

    /// Returns the minimum gas required to execute the opcode.
    #[inline]
    pub const fn min_gas(&self) -> u16 {
        self.gas
    }

    /// Returns whether the opcode is view.
    #[inline]
    pub const fn is_view(&self) -> bool {
        self.view
    }

    /// Returns whether the opcode is pure.
    #[inline]
    pub const fn is_pure(&self) -> bool {
        self.pure
    }
}

impl From<u8> for OpCodeInfo {
    #[inline]
    fn from(opcode: u8) -> Self {
        OPCODE_INFO_TABLE[opcode as usize].unwrap_or(OpCodeInfo {
            name: "unknown",
            inputs: 0,
            outputs: 0,
            terminating: true,
            gas: 0,
            view: false,
            pure: false,
        })
    }
}

/// Sets the number of stack inputs and outputs.
#[inline]
pub const fn stack_io(mut op: OpCodeInfo, inputs: u8, outputs: u8) -> OpCodeInfo {
    op.inputs = inputs;
    op.outputs = outputs;
    op
}

/// Sets the terminating flag to true.
#[inline]
pub const fn terminating(mut op: OpCodeInfo) -> OpCodeInfo {
    op.terminating = true;
    op
}

/// Sets the gas required to execute the opcode.
#[inline]
pub const fn min_gas(mut op: OpCodeInfo, gas: u16) -> OpCodeInfo {
    op.gas = gas;
    op
}

/// Sets the view flag to false.
#[inline]
pub const fn non_view(mut op: OpCodeInfo) -> OpCodeInfo {
    op.view = false;
    op
}

/// Sets the pure flag to false.
#[inline]
pub const fn non_pure(mut op: OpCodeInfo) -> OpCodeInfo {
    op.pure = false;
    op
}

macro_rules! opcodes {
    ($($val:literal => $name:ident => $($modifier:ident $(( $($modifier_arg:expr),* ))?),*);* $(;)?) => {
        // create a constant for each opcode
        $(
            #[doc = concat!("The `", stringify!($val), "` (\"", stringify!($name),"\") opcode.")]
            pub const $name: u8 = $val;
        )*

        // create a macro for each opcode which constructs a wrapped opcode
        // there can be unlimited inputs to the macro
        // each input MUST implement the `Into<WrappedOpcode>` trait
        $(
            paste!{
                /// A macro that creates a wrapped opcode with the given inputs.
                ///
                /// This macro provides a convenient way to construct a `WrappedOpcode` for a specific
                /// opcode (`$name`), supporting between 0 and 8 input arguments that implement
                /// `Into<WrappedInput>`.
                #[macro_export]
                macro_rules! [<w_$name:lower>] {
                    // zero inputs
                    () => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: Vec::new(),
                        }
                    };
                    // one input
                    ($arg:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg.into()],
                        }
                    };
                    // two inputs
                    ($arg1:expr, $arg2:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into()],
                        }
                    };
                    // three inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into()],
                        }
                    };
                    // four inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into()],
                        }
                    };
                    // five inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into()],
                        }
                    };
                    // six inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into()],
                        }
                    };
                    // seven inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into()],
                        }
                    };
                    // eight inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into()],
                        }
                    };
                    // nine inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into()],
                        }
                    };
                    // ten inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into()],
                        }
                    };
                    // eleven inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into()],
                        }
                    };
                    // twelve inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr, $arg12:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into(), $arg12.into()],
                        }
                    };
                    // thirteen inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr, $arg12:expr, $arg13:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into(), $arg12.into(), $arg13.into()],
                        }
                    };
                    // fourteen inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr, $arg12:expr, $arg13:expr, $arg14:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into(), $arg12.into(), $arg13.into(), $arg14.into()],
                        }
                    };
                    // fifteen inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr, $arg12:expr, $arg13:expr, $arg14:expr, $arg15:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into(), $arg12.into(), $arg13.into(), $arg14.into(), $arg15.into()],
                        }
                    };
                    // sixteen inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr, $arg12:expr, $arg13:expr, $arg14:expr, $arg15:expr, $arg16:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into(), $arg12.into(), $arg13.into(), $arg14.into(), $arg15.into(), $arg16.into()],
                        }
                    };
                    // seventeen inputs
                    ($arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr, $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr, $arg10:expr, $arg11:expr, $arg12:expr, $arg13:expr, $arg14:expr, $arg15:expr, $arg16:expr, $arg17:expr) => {
                        $crate::core::opcodes::WrappedOpcode {
                            opcode: $val,
                            inputs: vec![$arg1.into(), $arg2.into(), $arg3.into(), $arg4.into(), $arg5.into(), $arg6.into(), $arg7.into(), $arg8.into(), $arg9.into(), $arg10.into(), $arg11.into(), $arg12.into(), $arg13.into(), $arg14.into(), $arg15.into(), $arg16.into(), $arg17.into()],
                        }
                    };
                }
            }
        )*

        /// Maps each opcode to its info.
        pub const OPCODE_INFO_TABLE: [Option<OpCodeInfo>; 256] = {
            let mut map = [None; 256];
            let mut prev: u8 = 0;
            $(
                let val: u8 = $val;
                assert!(val == 0 || val > prev, "opcodes must be sorted in ascending order");
                prev = val;
                let info = OpCodeInfo::new(
                    stringify!($name)
                );
                $(
                let info = $modifier(info, $($($modifier_arg),*)?);
                )*
                map[$val] = Some(info);
            )*
            let _ = prev;
            map
        };

        /// Maps each opcode to its name. (So we dont need to load [`OpCodeInfo`] to get the name)
        pub const OPCODE_NAME_TABLE: [&'static str; 256] = {
            let mut map = ["unknown"; 256];
            $(
                map[$val] = stringify!($name);
            )*
            map
        };
    }
}

/// Get the name of an opcode.
#[inline]
pub fn opcode_name(opcode: u8) -> &'static str {
    OPCODE_NAME_TABLE[opcode as usize]
}

opcodes! {
    0x00 => STOP => terminating;

    0x01 => ADD => stack_io(2, 1), min_gas(3);
    0x02 => MUL => stack_io(2, 1), min_gas(5);
    0x03 => SUB => stack_io(2, 1), min_gas(3);
    0x04 => DIV => stack_io(2, 1), min_gas(5);
    0x05 => SDIV => stack_io(2, 1), min_gas(5);
    0x06 => MOD => stack_io(2, 1), min_gas(5);
    0x07 => SMOD => stack_io(2, 1), min_gas(5);
    0x08 => ADDMOD => stack_io(3, 1), min_gas(8);
    0x09 => MULMOD => stack_io(3, 1), min_gas(8);
    0x0a => EXP => stack_io(2, 1), min_gas(10);
    0x0b => SIGNEXTEND => stack_io(2, 1), min_gas(5);

    0x10 => LT => stack_io(2, 1), min_gas(3);
    0x11 => GT => stack_io(2, 1), min_gas(3);
    0x12 => SLT => stack_io(2, 1), min_gas(3);
    0x13 => SGT => stack_io(2, 1), min_gas(3);
    0x14 => EQ => stack_io(2, 1), min_gas(3);
    0x15 => ISZERO => stack_io(1, 1), min_gas(3);
    0x16 => AND => stack_io(2, 1), min_gas(3);
    0x17 => OR => stack_io(2, 1), min_gas(3);
    0x18 => XOR => stack_io(2, 1), min_gas(3);
    0x19 => NOT => stack_io(1, 1), min_gas(3);
    0x1a => BYTE => stack_io(2, 1), min_gas(3);
    0x1b => SHL => stack_io(2, 1), min_gas(3);
    0x1c => SHR => stack_io(2, 1), min_gas(3);
    0x1d => SAR => stack_io(2, 1), min_gas(3);

    0x20 => SHA3 => stack_io(2, 1), min_gas(30);

    0x30 => ADDRESS => stack_io(0, 1), min_gas(2);
    0x31 => BALANCE => stack_io(1, 1), min_gas(100), non_pure;
    0x32 => ORIGIN => stack_io(0, 1), min_gas(2), non_pure;
    0x33 => CALLER => stack_io(0, 1), min_gas(2), non_pure;
    0x34 => CALLVALUE => stack_io(0, 1), min_gas(2);
    0x35 => CALLDATALOAD => stack_io(1, 1), min_gas(3);
    0x36 => CALLDATASIZE => stack_io(0, 1), min_gas(2);
    0x37 => CALLDATACOPY => stack_io(3, 0), min_gas(3);
    0x38 => CODESIZE => stack_io(0, 1), min_gas(2);
    0x39 => CODECOPY => stack_io(3, 0), min_gas(3);
    0x3a => GASPRICE => stack_io(0, 1), min_gas(2), non_pure;
    0x3b => EXTCODESIZE => stack_io(1, 1), min_gas(100), non_pure;
    0x3c => EXTCODECOPY => stack_io(4, 0), min_gas(100), non_pure;
    0x3d => RETURNDATASIZE => stack_io(0, 1), min_gas(2);
    0x3e => RETURNDATACOPY => stack_io(3, 0), min_gas(3);
    0x3f => EXTCODEHASH => stack_io(1, 1), min_gas(100), non_pure;
    0x40 => BLOCKHASH => stack_io(1, 1), min_gas(20), non_pure;
    0x41 => COINBASE => stack_io(0, 1), min_gas(2), non_pure;
    0x42 => TIMESTAMP => stack_io(0, 1), min_gas(2), non_pure;
    0x43 => NUMBER => stack_io(0, 1), min_gas(2), non_pure;
    0x44 => PREVRANDAO => stack_io(0, 1), min_gas(2), non_pure;
    0x45 => GASLIMIT => stack_io(0, 1), min_gas(2), non_pure;
    0x46 => CHAINID => stack_io(0, 1), min_gas(2), non_pure;
    0x47 => SELFBALANCE => stack_io(0, 1), min_gas(5), non_pure;
    0x48 => BASEFEE => stack_io(0, 1), min_gas(2), non_pure;
    0x49 => BLOBHASH => stack_io(0, 1), min_gas(3), non_pure;
    0x4a => BLOBBASEFEE => stack_io(0, 1), min_gas(2), non_pure;

    0x50 => POP => stack_io(1, 0), min_gas(2);
    0x51 => MLOAD => stack_io(1, 1), min_gas(3);
    0x52 => MSTORE => stack_io(2, 0), min_gas(3);
    0x53 => MSTORE8 => stack_io(2, 0), min_gas(3);
    0x54 => SLOAD => stack_io(1, 1), min_gas(0), non_pure;
    0x55 => SSTORE => stack_io(2, 0), non_pure, non_view;
    0x56 => JUMP => stack_io(1, 0), min_gas(8);
    0x57 => JUMPI => stack_io(2, 0), min_gas(10);
    0x58 => PC => stack_io(0, 1), min_gas(2);
    0x59 => MSIZE => stack_io(0, 1), min_gas(2);
    0x5a => GAS => stack_io(0, 1), min_gas(2);
    0x5b => JUMPDEST => min_gas(1);
    0x5c => TLOAD => stack_io(1, 1), min_gas(100);
    0x5d => TSTORE => stack_io(2, 0), min_gas(100);
    0x5e => MCOPY => stack_io(3, 0), min_gas(3);

    0x5f => PUSH0 => stack_io(0, 1), min_gas(3);
    0x60 => PUSH1 => stack_io(0, 1), min_gas(3);
    0x61 => PUSH2 => stack_io(0, 1), min_gas(3);
    0x62 => PUSH3 => stack_io(0, 1), min_gas(3);
    0x63 => PUSH4 => stack_io(0, 1), min_gas(3);
    0x64 => PUSH5 => stack_io(0, 1), min_gas(3);
    0x65 => PUSH6 => stack_io(0, 1), min_gas(3);
    0x66 => PUSH7 => stack_io(0, 1), min_gas(3);
    0x67 => PUSH8 => stack_io(0, 1), min_gas(3);
    0x68 => PUSH9 => stack_io(0, 1), min_gas(3);
    0x69 => PUSH10 => stack_io(0, 1), min_gas(3);
    0x6a => PUSH11 => stack_io(0, 1), min_gas(3);
    0x6b => PUSH12 => stack_io(0, 1), min_gas(3);
    0x6c => PUSH13 => stack_io(0, 1), min_gas(3);
    0x6d => PUSH14 => stack_io(0, 1), min_gas(3);
    0x6e => PUSH15 => stack_io(0, 1), min_gas(3);
    0x6f => PUSH16 => stack_io(0, 1), min_gas(3);
    0x70 => PUSH17 => stack_io(0, 1), min_gas(3);
    0x71 => PUSH18 => stack_io(0, 1), min_gas(3);
    0x72 => PUSH19 => stack_io(0, 1), min_gas(3);
    0x73 => PUSH20 => stack_io(0, 1), min_gas(3);
    0x74 => PUSH21 => stack_io(0, 1), min_gas(3);
    0x75 => PUSH22 => stack_io(0, 1), min_gas(3);
    0x76 => PUSH23 => stack_io(0, 1), min_gas(3);
    0x77 => PUSH24 => stack_io(0, 1), min_gas(3);
    0x78 => PUSH25 => stack_io(0, 1), min_gas(3);
    0x79 => PUSH26 => stack_io(0, 1), min_gas(3);
    0x7a => PUSH27 => stack_io(0, 1), min_gas(3);
    0x7b => PUSH28 => stack_io(0, 1), min_gas(3);
    0x7c => PUSH29 => stack_io(0, 1), min_gas(3);
    0x7d => PUSH30 => stack_io(0, 1), min_gas(3);
    0x7e => PUSH31 => stack_io(0, 1), min_gas(3);
    0x7f => PUSH32 => stack_io(0, 1), min_gas(3);

    0x80 => DUP1 => stack_io(1, 2), min_gas(3);
    0x81 => DUP2 => stack_io(2, 3), min_gas(3);
    0x82 => DUP3 => stack_io(3, 4), min_gas(3);
    0x83 => DUP4 => stack_io(4, 5), min_gas(3);
    0x84 => DUP5 => stack_io(5, 6), min_gas(3);
    0x85 => DUP6 => stack_io(6, 7), min_gas(3);
    0x86 => DUP7 => stack_io(7, 8), min_gas(3);
    0x87 => DUP8 => stack_io(8, 9), min_gas(3);
    0x88 => DUP9 => stack_io(9, 10), min_gas(3);
    0x89 => DUP10 => stack_io(10, 11), min_gas(3);
    0x8a => DUP11 => stack_io(11, 12), min_gas(3);
    0x8b => DUP12 => stack_io(12, 13), min_gas(3);
    0x8c => DUP13 => stack_io(13, 14), min_gas(3);
    0x8d => DUP14 => stack_io(14, 15), min_gas(3);
    0x8e => DUP15 => stack_io(15, 16), min_gas(3);
    0x8f => DUP16 => stack_io(16, 17), min_gas(3);

    0x90 => SWAP1 => stack_io(2, 2), min_gas(3);
    0x91 => SWAP2 => stack_io(3, 3), min_gas(3);
    0x92 => SWAP3 => stack_io(4, 4), min_gas(3);
    0x93 => SWAP4 => stack_io(5, 5), min_gas(3);
    0x94 => SWAP5 => stack_io(6, 6), min_gas(3);
    0x95 => SWAP6 => stack_io(7, 7), min_gas(3);
    0x96 => SWAP7 => stack_io(8, 8), min_gas(3);
    0x97 => SWAP8 => stack_io(9, 9), min_gas(3);
    0x98 => SWAP9 => stack_io(10, 10), min_gas(3);
    0x99 => SWAP10 => stack_io(11, 11), min_gas(3);
    0x9a => SWAP11 => stack_io(12, 12), min_gas(3);
    0x9b => SWAP12 => stack_io(13, 13), min_gas(3);
    0x9c => SWAP13 => stack_io(14, 14), min_gas(3);
    0x9d => SWAP14 => stack_io(15, 15), min_gas(3);
    0x9e => SWAP15 => stack_io(16, 16), min_gas(3);
    0x9f => SWAP16 => stack_io(17, 17), min_gas(3);

    0xa0 => LOG0 => stack_io(2, 0), min_gas(375);
    0xa1 => LOG1 => stack_io(3, 0), min_gas(750);
    0xa2 => LOG2 => stack_io(4, 0), min_gas(1125);
    0xa3 => LOG3 => stack_io(5, 0), min_gas(1500);
    0xa4 => LOG4 => stack_io(6, 0), min_gas(1875);

    0xf0 => CREATE => stack_io(3, 1), min_gas(32000), non_pure, non_view;
    0xf1 => CALL => stack_io(7, 1), min_gas(100), non_pure, non_view;
    0xf2 => CALLCODE => stack_io(7, 1), min_gas(100), non_pure, non_view;
    0xf3 => RETURN => stack_io(2, 0), terminating;
    0xf4 => DELEGATECALL => stack_io(6, 1), min_gas(100), non_pure, non_view;
    0xf5 => CREATE2 => stack_io(4, 1), min_gas(32000), non_pure, non_view;
    0xfa => STATICCALL => stack_io(6, 1), min_gas(100), non_pure, non_view;
    0xfd => REVERT => stack_io(2, 0), terminating;
    0xfe => INVALID => terminating;
    0xff => SELFDESTRUCT => stack_io(1, 0), min_gas(5000), terminating, non_pure, non_view;
}
