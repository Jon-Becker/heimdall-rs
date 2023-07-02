use std::{env, fs};

use crate::{
    constants::{ADDRESS_REGEX, BYTECODE_REGEX},
    ether::{evm::opcodes::opcode, rpc::get_code},
    io::{file::*, logging::*},
    utils::strings::{decode_hex, encode_hex},
};
use clap::{AppSettings, Parser};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Disassemble EVM bytecode to Assembly",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall disassemble <TARGET> [OPTIONS]")]
pub struct DisassemblerArgs {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// The output directory to write the disassembled bytecode to
    #[clap(long = "output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    /// The RPC provider to use for fetching target bytecode.
    #[clap(long = "rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,
}

pub fn disassemble(args: DisassemblerArgs) -> String {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, _) = Logger::new(match args.verbose.log_level() {
        Some(level) => level.as_str(),
        None => "SILENT",
    });

    // parse the output directory
    let mut output_dir: String;
    if args.output.is_empty() {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    } else {
        output_dir = args.output.clone();
    }

    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target).unwrap() {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // We are disassembling a contract address, so we need to fetch the bytecode from the RPC
        // provider.
        contract_bytecode = get_code(&args.target, &args.rpc_url, &logger);
    } else if BYTECODE_REGEX.is_match(&args.target).unwrap() {
        contract_bytecode = args.target;
    } else {
        // push the address to the output directory
        if output_dir != args.output {
            output_dir.push_str("/local");
        }

        // We are disassembling a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {
                if BYTECODE_REGEX.is_match(&contents).unwrap() && contents.len() % 2 == 0 {
                    contents
                } else {
                    logger
                        .error(&format!("file '{}' doesn't contain valid bytecode.", &args.target));
                    std::process::exit(1)
                }
            }
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &args.target));
                std::process::exit(1)
            }
        };
    }

    let mut program_counter = 0;
    let mut output: String = String::new();

    // Iterate over the bytecode, disassembling each instruction.
    let byte_array = decode_hex(&contract_bytecode.replacen("0x", "", 1)).unwrap();

    while program_counter < byte_array.len() {
        let operation = opcode(byte_array[program_counter]);
        let mut pushed_bytes: String = String::new();

        if operation.name.contains("PUSH") {
            let byte_count_to_push: u8 = operation.name.replace("PUSH", "").parse().unwrap();

            pushed_bytes = match byte_array
                .get(program_counter + 1..program_counter + 1 + byte_count_to_push as usize)
            {
                Some(bytes) => encode_hex(bytes.to_vec()),
                None => break,
            };
            program_counter += byte_count_to_push as usize;
        }

        output.push_str(
            format!("{} {} {}\n", program_counter, operation.name, pushed_bytes).as_str(),
        );
        program_counter += 1;
    }

    logger.info(&format!("disassembled {program_counter} bytes successfully."));

    // write the output to a file
    write_file(&format!("{output_dir}/bytecode.evm"), &contract_bytecode);
    let file_path = write_file(&format!("{output_dir}/disassembled.asm"), &output);
    logger.success(&format!("wrote disassembled bytecode to '{file_path}' ."));

    // log the time it took to disassemble the bytecode
    logger.debug(&format!("disassembly completed in {} ms.", now.elapsed().as_millis()));

    output
}
