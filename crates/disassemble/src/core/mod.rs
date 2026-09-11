use std::time::Instant;

use crate::{error::Error, interfaces::DisassemblerArgs};
use eyre::eyre;
use heimdall_common::utils::strings::encode_hex;
use heimdall_vm::core::{opcodes::OpCodeInfo, program::Program};
use tracing::{debug, info};

/// Disassembles EVM bytecode into readable assembly instructions
///
/// This function takes the bytecode of a contract and converts it into a string
/// representation of the equivalent EVM assembly code. It handles special cases
/// like PUSH operations which consume additional bytes as data.
///
/// # Arguments
///
/// * `args` - Arguments specifying the target and disassembly options
///
/// # Returns
///
/// A string containing the disassembled bytecode in assembly format
pub async fn disassemble(args: DisassemblerArgs) -> Result<String, Error> {
    // init
    let start_time = Instant::now();
    let mut asm = String::new();

    // Resolve hardfork (handles Auto detection if needed)
    let start_hardfork_resolve = Instant::now();
    let hardfork = args.get_hardfork().await;
    debug!("resolved hardfork: {} (took {:?})", hardfork, start_hardfork_resolve.elapsed());

    // get the bytecode from the target
    let start_fetch_time = Instant::now();
    let contract_bytecode =
        args.get_bytecode().await.map_err(|e| eyre!("fetching target bytecode failed: {}", e))?;
    debug!("fetching target bytecode took {:?}", start_fetch_time.elapsed());

    // Decode through the shared structural front end so disassembly, CFG recovery, and future
    // analysis passes agree on instruction boundaries and truncated PUSH semantics.
    let start_disassemble_time = Instant::now();
    let program = Program::decode(&contract_bytecode, hardfork);
    for instruction in &program.instructions {
        // Preserve the disassembler's existing behavior for incomplete trailing PUSH data. The
        // structural frontend still retains the instruction for analysis with EVM zero-padding.
        if instruction.truncated {
            break
        }
        let opcode_name = OpCodeInfo::for_fork(instruction.opcode, hardfork)
            .map_or("unknown", |info| info.name());
        let pushed_bytes = if instruction.immediate.is_empty() {
            String::new()
        } else {
            encode_hex(&instruction.immediate)
        };
        asm.push_str(
            format!(
                "{} {} {}\n",
                if args.decimal_counter {
                    instruction.pc.to_string()
                } else {
                    format!("{:06x}", instruction.pc)
                },
                opcode_name,
                pushed_bytes
            )
            .as_str(),
        );
    }
    debug!("disassembly took {:?}", start_disassemble_time.elapsed());

    info!("disassembled {} bytes successfully", contract_bytecode.len());
    debug!("disassembly took {:?}", start_time.elapsed());
    Ok(asm)
}
