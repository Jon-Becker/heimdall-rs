use crate::utils::strings::encode_hex_reduced;

use super::evm::opcodes::*;

impl WrappedOpcode {
    // Returns a WrappedOpcode's yul representation.
    pub fn yulify(&self) -> String {
        if self.opcode.name == "PUSH0" {
            "0".to_string()
        } else if self.opcode.name.starts_with("PUSH") {
            self.inputs[0]._yulify()
        } else {
            format!(
                "{}({})",
                self.opcode.name.to_lowercase(),
                self.inputs.iter().map(|input| input._yulify()).collect::<Vec<String>>().join(", ")
            )
        }
    }
}

impl WrappedInput {
    // Returns a WrappedInput's solidity representation.
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
