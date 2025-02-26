use std::time::Instant;

use crate::{error::Error, interfaces::DisassemblerArgs};
use eyre::eyre;
use heimdall_common::utils::strings::encode_hex;
use heimdall_vm::core::opcodes::opcode_name;
use tracing::{debug, info};

pub async fn disassemble(args: DisassemblerArgs) -> Result<String, Error> {
    // init
    let start_time = Instant::now();
    let mut program_counter = 0;
    let mut asm = String::new();

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let contract_bytecode =
        args.get_bytecode().await.map_err(|e| eyre!("fetching target bytecode failed: {}", e))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    // iterate over the bytecode, disassembling each instruction
    let start_disassemble_time = Instant::now();
    while program_counter < contract_bytecode.len() {
        let opcode = contract_bytecode[program_counter];
        let mut pushed_bytes = String::new();

        // handle PUSH0 -> PUSH32, which require us to push the next N bytes
        // onto the stack
        if (0x5f..=0x7f).contains(&opcode) {
            let byte_count_to_push: u8 = opcode - 0x5f;
            pushed_bytes = match contract_bytecode
                .get(program_counter + 1..program_counter + 1 + byte_count_to_push as usize)
            {
                Some(bytes) => encode_hex(bytes),
                None => break,
            };
            program_counter += byte_count_to_push as usize;
        }

        let offset = program_counter - 1;
        asm.push_str(
            format!(
                "{} {} {}\n",
                if args.decimal_counter {
                    offset.to_string()
                } else {
                    format!("{:06x}", offset)
                },
                opcode_name(opcode),
                pushed_bytes
            )
            .as_str(),
        );
        program_counter += 1;
    }
    debug!("disassembly took {:?}", start_disassemble_time.elapsed());

    info!("disassembled {} bytes successfully", program_counter);
    debug!("disassembly took {:?}", start_time.elapsed());
    Ok(asm)
}

#[cfg(test)]
mod tests {
    use super::super::DisassemblerArgsBuilder;
    use super::disassemble;
    use tokio::test;

    #[test]
    async fn test_disassemble() -> Result<(), Box<dyn std::error::Error>> {
        let args = DisassemblerArgsBuilder::new()
            .target("0x9f00c43700bc0000Ff91bE00841F8e04c0495000".to_string())
            .rpc_url("https://eth.llamarpc.com".to_string())
            .build()
            .unwrap();

        let result = disassemble(args).await.unwrap();

        println!("Disassembled contract: {:#?}", result);

        Ok(())
    }
}
