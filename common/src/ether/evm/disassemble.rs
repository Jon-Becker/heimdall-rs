use std::env;
use std::fs;

use clap::{AppSettings, Parser};
use ethers::{
    core::types::{Address},
    providers::{Middleware, Provider, Http},
};
use crate::{
    consts::{ ADDRESS_REGEX, BYTECODE_REGEX },
    io::{ logging::*, file::* },
    ether::evm::{ opcodes::opcode }
};


#[derive(Debug, Clone, Parser)]
#[clap(about = "Disassemble EVM bytecode to Assembly",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall disassemble <TARGET> [OPTIONS]")]
pub struct DisassemblerArgs {
    // The target to decompile, either a file, contract address, or ENS name.
    #[clap(required=true)]
    pub target: String,

    // Set the output verbosity level, 1 - 5.
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
    
    // The output directory to write the decompiled files to
    #[clap(long="output", short, default_value = "", hide_default_value = true)]
    pub output: String,

    // The RPC provider to use for fetching target bytecode.
    #[clap(long="rpc-url", short, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    // When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

}


pub fn disassemble(args: DisassemblerArgs) {
    use std::time::Instant;
    let now = Instant::now();

    let (logger, _)= Logger::new(args.verbose.log_level().unwrap().as_str());

    // parse the output directory
    let mut output_dir: String;
    if &args.output.len() <= &0 {
        output_dir = match env::current_dir() {
            Ok(dir) => dir.into_os_string().into_string().unwrap(),
            Err(_) => {
                logger.error("failed to get current directory.");
                std::process::exit(1);
            }
        };
        output_dir.push_str("/output");
    }
    else {
        output_dir = args.output.clone();
    }

    let contract_bytecode: String;
    if ADDRESS_REGEX.is_match(&args.target) {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str(&format!("/{}", &args.target));
        }

        // create new runtime block
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // We are decompiling a contract address, so we need to fetch the bytecode from the RPC provider.
        contract_bytecode = rt.block_on(async {

            // make sure the RPC provider isn't empty
            if &args.rpc_url.len() <= &0 {
                logger.error("disassembling an on-chain contract requires an RPC provider. Use `heimdall disassemble --help` for more information.");
                std::process::exit(1);
            }

            // create new provider
            let provider = match Provider::<Http>::try_from(&args.rpc_url) {
                Ok(provider) => provider,
                Err(_) => {
                    logger.error(&format!("failed to connect to RPC provider '{}' .", &args.rpc_url).to_string());
                    std::process::exit(1)
                }
            };

            // safely unwrap the address
            let address = match args.target.parse::<Address>() {
                Ok(address) => address,
                Err(_) => {
                    logger.error(&format!("failed to parse address '{}' .", &args.target).to_string());
                    std::process::exit(1)
                }
            };

            // fetch the bytecode at the address
            let bytecode_as_bytes = match provider.get_code(address, None).await {
                Ok(bytecode) => bytecode,
                Err(_) => {
                    logger.error(&format!("failed to fetch bytecode from '{}' .", &args.target).to_string());
                    std::process::exit(1)
                }
            };
            return bytecode_as_bytes.to_string().replacen("0x", "", 1);
        });
        
    }
    else {

        // push the address to the output directory
        if &output_dir != &args.output {
            output_dir.push_str("/local");
        }

        // We are decompiling a file, so we need to read the bytecode from the file.
        contract_bytecode = match fs::read_to_string(&args.target) {
            Ok(contents) => {                
                if BYTECODE_REGEX.is_match(&contents) && contents.len() % 2 == 0 {
                    contents.replacen("0x", "", 1)
                }
                else {
                    logger.error(&format!("file '{}' doesn't contain valid bytecode.", &args.target).to_string());
                    std::process::exit(1)
                }
            },
            Err(_) => {
                logger.error(&format!("failed to open file '{}' .", &args.target).to_string());
                std::process::exit(1)
            }
        };
    }

    let mut program_counter = 0;
    let mut output: String = String::new();

    // Iterate over the bytecode, disassembling each instruction.
    let byte_array = contract_bytecode.chars()
        .collect::<Vec<char>>()
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<String>>();
    
    while program_counter < byte_array.len() {
        
        let operation = opcode(&byte_array[program_counter]);
        let mut pushed_bytes: String = String::new();

        if operation.name.contains("PUSH") {
            let byte_count_to_push: u8 = operation.name.replace("PUSH", "").parse().unwrap();
            pushed_bytes = byte_array[program_counter + 1..program_counter + 1 + byte_count_to_push as usize].join("");
            program_counter += byte_count_to_push as usize;
        }
        

        output.push_str(format!("{} {} {}\n", program_counter, operation.name, pushed_bytes).as_str());
        program_counter += 1;
    }

    logger.success(&format!("disassembled {} bytes successfully.", program_counter).to_string());

    write_file(&String::from(format!("{}/bytecode.evm", &output_dir)), &contract_bytecode);    
    let file_path = write_file(&String::from(format!("{}/disassembled.asm", &output_dir)), &output);
    logger.info(&format!("wrote disassembled bytecode to '{}' .", file_path).to_string());

    let elapsed = now.elapsed();
    logger.debug(&format!("disassembly completed in {} ms.", elapsed.as_millis()).to_string());
    return
}