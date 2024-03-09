use std::time::Instant;

use crate::{error::Error, interfaces::DisassemblerArgs};
use eyre::eyre;
use heimdall_common::{
    ether::{bytecode::get_bytecode_from_target, evm::core::opcodes::Opcode},
    utils::strings::encode_hex,
};
use tracing::{debug, info, trace, warn};

pub async fn disassemble(args: DisassemblerArgs) -> Result<String, Error> {
    // init
    let start_time = Instant::now();
    let mut program_counter = 0;
    let mut asm = String::new();

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let contract_bytecode = get_bytecode_from_target(&args.target, &args.rpc_url)
        .await
        .map_err(|e| eyre!("fetching target bytecode failed: {}", e))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    // iterate over the bytecode, disassembling each instruction
    let start_disassemble_time = Instant::now();
    while program_counter < contract_bytecode.len() {
        let operation = Opcode::new(contract_bytecode[program_counter]);
        let mut pushed_bytes = String::new();

        // handle PUSH0 -> PUSH32, which require us to push the next N bytes
        // onto the stack
        if operation.code >= 0x5f && operation.code <= 0x7f {
            let byte_count_to_push: u8 = operation.code - 0x5f;
            pushed_bytes = match contract_bytecode
                .get(program_counter + 1..program_counter + 1 + byte_count_to_push as usize)
            {
                Some(bytes) => encode_hex(bytes.to_vec()),
                None => break,
            };
            program_counter += byte_count_to_push as usize;
        }

        asm.push_str(
            format!(
                "{} {} {}\n",
                if args.decimal_counter {
                    program_counter.to_string()
                } else {
                    format!("{:06x}", program_counter)
                },
                operation.name,
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
