use heimdall_common::utils::strings::encode_hex_reduced;

use crate::core::opcodes::{opcode_name, WrappedInput, WrappedOpcode, PUSH0};

impl WrappedOpcode {
    /// Returns a WrappedOpcode's yul representation.
    pub fn yulify(&self) -> String {
        if self.opcode == PUSH0 {
            "0".to_string()
        } else if (0x5f..=0x7f).contains(&self.opcode) {
            self.inputs[0]._yulify()
        } else {
            format!(
                "{}({})",
                opcode_name(self.opcode).to_lowercase(),
                self.inputs.iter().map(|input| input._yulify()).collect::<Vec<String>>().join(", ")
            )
        }
    }
}

impl WrappedInput {
    /// Returns a WrappedInput's solidity representation.
    fn _yulify(&self) -> String {
        let mut solidified_wrapped_input = String::new();

        match self {
            WrappedInput::Raw(u256) => {
                solidified_wrapped_input.push_str(&encode_hex_reduced(*u256));
            }
            WrappedInput::Opcode(opcode) => {
                solidified_wrapped_input.push_str(&opcode.yulify());
            }
        }

        solidified_wrapped_input
    }
}

#[cfg(test)]
mod tests {

    use alloy::primitives::U256;

    use crate::core::opcodes::{WrappedInput, WrappedOpcode};

    #[test]
    fn test_push0() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(0x5f, vec![]);
        assert_eq!(add_operation_wrapped.yulify(), "0");
    }

    #[test]
    fn test_yulify_add() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Raw(U256::from(1u8)), WrappedInput::Raw(U256::from(2u8))],
        );
        assert_eq!(add_operation_wrapped.yulify(), "add(0x01, 0x02)");
    }

    #[test]
    fn test_yulify_add_complex() {
        // wraps an ADD operation with 2 raw inputs
        let add_operation_wrapped = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Raw(U256::from(1u8)), WrappedInput::Raw(U256::from(2u8))],
        );
        let complex_add_operation = WrappedOpcode::new(
            0x01,
            vec![WrappedInput::Opcode(add_operation_wrapped), WrappedInput::Raw(U256::from(3u8))],
        );
        assert_eq!(complex_add_operation.yulify(), "add(add(0x01, 0x02), 0x03)");
    }
}
