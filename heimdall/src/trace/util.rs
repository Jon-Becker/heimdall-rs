use std::str::FromStr;

use ethers::{providers::{Provider, Http, Middleware}, abi::{AbiEncode, decode, ParamType}, types::{Address, H256}};

use heimdall_common::{io::logging::{Logger, TraceFactory}, ether::{evm::{vm::{State, Result, VM, Block}, types::display_inline}, signatures::resolve_event_signature}, utils::strings::decode_hex};

use crate::decode::decode_calldata;

#[derive(Debug, Clone)]
pub struct TxTrace {
    pub instructions: Vec<State>,
    pub result: Result,
    pub children: Vec<TxTrace>,
}

impl TxTrace {
    pub fn new(vm: &VM) -> Self {
        Self {
            instructions: Vec::new(),
            result: Result {
                gas_used: vm.gas_used,
                gas_remaining: vm.gas_remaining,
                returndata: vm.returndata.to_owned(),
                exitcode: vm.exitcode,
                events: vm.events.clone(),
                runtime: vm.timestamp.elapsed().as_secs_f64(),
                instruction: vm.instruction,
            },
            children: Vec::new(),
        }
    }
}

pub fn simulate(
    rpc_url: String,
    contract_address: String,
    calldata: String,
    contract_bytecode: String,
    from_address: String,
    origin_address: String,
    value: u128,
    gas_remaining: u128,
    block: Block,
    parent_instruction: u32,
    trace: &mut TraceFactory,
    parent_index: u32,
) -> TxTrace {
    let mut state_vec = Vec::new();
    let logger = Logger::new("TRACE").0;

    // attempt to resolve the function from calldata
    let parent_index = match decode_calldata(calldata.clone()) {
        Some(function) => {
            trace.add_call(
                parent_index,
                parent_instruction,
                contract_address.clone(),
                function.name,
                function.decoded_inputs.as_ref().unwrap().iter().map(|x| display_inline(vec![x.to_owned()])).collect(),
                "()".to_string()
            )
        }
        None => {
            trace.add_call(
                parent_index,
                parent_instruction,
                contract_address.clone(),
                calldata.replace("0x", "").get(0..8).unwrap_or("0x00000000").to_string(),
                vec![
                    calldata.replace("0x", "").get(8..).unwrap_or("").to_string()
                ],
                "()".to_string()
            )
        }
    };


    // make a new VM object
    let mut vm = VM::new(
        contract_bytecode,
        calldata,
        contract_address.clone(),
        origin_address.clone(),
        from_address.clone(),
        value,
        gas_remaining,
    );

    println!("NEW VM! \n\n");

    // create a new trace
    let mut transaction_trace = TxTrace::new(&vm);

    // update the block
    vm.block = block.clone();

    // run the VM
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {

        // first peek, and check if override is needed
        let next_operation = vm.peek();

        // override external calls
        if vec!["STATICCALL", "DELEGATECALL", "CALL", "CALLCODE"].contains(&next_operation.0.name.as_str()) {

            let operations = next_operation.2.clone();

            // parse the address interacted with
            let interacted_with = format!(
                "0x{}",
                operations[1].encode_hex().replace("0x", "").get(24..).unwrap_or("0x0000000000000000000000000000000000000000")
            );
            let target_address = match interacted_with.parse::<Address>() {
                Ok(address) => address,
                Err(_) => {
                    logger.error(&format!("failed to parse address '{}' .", &interacted_with).to_string());
                    std::process::exit(1)
                }
            };
            
            // create new runtime block
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            // fetch the bytecode at the target address
            let contract_bytecode = rt.block_on(async {

                // new RPC provider
                let provider = match Provider::<Http>::try_from(&rpc_url) {
                    Ok(provider) => provider,
                    Err(_) => {
                        logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url).to_string());
                        std::process::exit(1)
                    }
                };

                // fetch the bytecode at the target address
                let bytecode_as_bytes = match provider.get_code(target_address.clone(), None).await {
                    Ok(bytecode) => bytecode,
                    Err(e) => {
                        println!("{:?}", e);
                        println!("failed to fetch bytecode at '{}' .", interacted_with);
                        
                        // likely a value transfer
                        // TODO
                        return "".to_string();
                    }
                };

                return bytecode_as_bytes.to_string().replacen("0x", "", 1);
            });

            // recursively call simulate with the new bytecode, and args from the call
            let gas_alotted = operations[0].as_u128();
            let mut value: u128 = 0;
            let calldata;

            // extract the calldata and value
            if vec!["STATICCALL", "DELEGATECALL"].contains(&next_operation.0.name.as_str()) {
                calldata = vm.memory.read(operations[2].as_usize(), operations[3].as_usize());

            }
            else {
                value = operations[2].as_u128();
                calldata = vm.memory.read(operations[3].as_usize(), operations[4].as_usize());
            }

            println!("value: {}", value);

            // recursively call simulate
            let child_trace = simulate(
                rpc_url.clone(),
                interacted_with.clone(),
                calldata,
                contract_bytecode,
                contract_address.clone(),
                origin_address.clone(),
                value,
                gas_alotted,
                block.clone(),
                vm.instruction.try_into().unwrap(),
                trace,
                parent_index,
            );

            // store RETURNDATA in memory
            vm.memory.store(operations[operations.len() - 2].as_usize(), operations[operations.len() - 1].as_usize(), child_trace.result.returndata.clone());
            vm.extreturndata = child_trace.result.returndata.clone();

            // update the trace
            transaction_trace.children.push(child_trace);
        }

        // override SLOAD
        else if next_operation.0.name.as_str() == "SLOAD" {

            // check if we modified the slot, and if so, use that value
            // TODO save storage across vms

            // get the storage value from the node
            let target_address = match contract_address.clone().parse::<Address>() {
                Ok(address) => address,
                Err(_) => {
                    logger.error(&format!("failed to parse address '{}' .", &contract_address).to_string());
                    std::process::exit(1)
                }
            };

            // create new runtime block
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            // fetch the storage value at the target address
            let storage_value = rt.block_on(async {

                // new RPC provider
                let provider = match Provider::<Http>::try_from(&rpc_url) {
                    Ok(provider) => provider,
                    Err(_) => {
                        logger.error(&format!("failed to connect to RPC provider '{}' .", &rpc_url).to_string());
                        std::process::exit(1)
                    }
                };

                // convert the slot to a H256
                let slot = match H256::from_str(&next_operation.2[0].clone().encode_hex()) {
                    Ok(slot) => slot,
                    Err(_) => {
                        logger.error(&format!("failed to parse slot '{}' .", &next_operation.2[0].clone()).to_string());
                        std::process::exit(1)
                    }
                };

                // convert the block hash to a H256
                let block_hash = match H256::from_str(&block.hash.clone().encode_hex()) {
                    Ok(block_hash) => block_hash,
                    Err(_) => {
                        logger.error(&format!("failed to parse block hash '{}' .", &block.hash.clone()).to_string());
                        std::process::exit(1)
                    }
                };

                // fetch the storage value at the target address
                let storage_value = match provider.get_storage_at(
                    target_address.clone(), 
                    slot, 
                    Some(ethers::types::BlockId::Hash(block_hash))
                ).await {
                    Ok(storage_value) => storage_value,
                    Err(e) => {
                        println!("{:?}", e);
                        println!("failed to fetch storage value at '{}' .", next_operation.2[0].clone());
                        
                        // likely a value transfer
                        // TODO
                        return "".to_string();
                    }
                };

                return format!("{:?}", storage_value).replacen("0x", "", 1);
            });

            // store the storage value
            vm.storage.store(next_operation.2[0].clone().encode_hex(), storage_value);
        }

        // override LOG
        else if next_operation.0.name.as_str().contains("LOG") {
            let selector = next_operation.2[2].clone().encode_hex().replace("0x", "");

            match next_operation.2.get(2..) {
                Some(topics) => {

                    let mut args = topics.get(1..).unwrap_or(&[]).iter().map(|x| x.encode_hex()).collect::<Vec<String>>();
                    let data = vm.memory.read(next_operation.2[0].as_usize(), next_operation.2[1].as_usize());
                    args.push(data);

                    // resolve the event
                    match resolve_event_signature(&selector) {
                        Some(events) => {
                            match events.get(0) {
                                Some(event) => {
                                    trace.add_emission(
                                        parent_index,
                                        vm.instruction.try_into().unwrap(),
                                        event.name.clone(),
                                        args,
                                    );
                                }
                                None => {
                                    trace.add_raw_emission(
                                        parent_index,
                                        vm.instruction.try_into().unwrap(),
                                        next_operation.2[2..].iter().map(|x| x.encode_hex()).collect::<Vec<String>>(),
                                        vm.memory.read(next_operation.2[0].as_usize(), next_operation.2[1].as_usize())
                                    );
                                }
                            }
                        },
                        None => {
                            trace.add_raw_emission(
                                parent_index,
                                vm.instruction.try_into().unwrap(),
                                next_operation.2[2..].iter().map(|x| x.encode_hex()).collect::<Vec<String>>(),
                                format!("0x{}", vm.memory.read(next_operation.2[0].as_usize(), next_operation.2[1].as_usize()))
                            );
                        }
                    };
                },
                None => {
                    trace.add_raw_emission(
                        parent_index,
                        vm.instruction.try_into().unwrap(),
                        vec![],
                        format!("0x{}", vm.memory.read(next_operation.2[0].as_usize(), next_operation.2[1].as_usize()))
                    );
                    continue
                }
            };
        }

        // override REVERT
        else if next_operation.0.name.as_str().contains("REVERT") {

            // Safely convert U256 to usize
            let offset: usize = match next_operation.2[0].try_into() {
                Ok(x) => x,
                Err(_) => 0,
            };
            let size: usize = match next_operation.2[1].try_into() {
                Ok(x) => x,
                Err(_) => 0,
            };

            let revert_data = vm.memory.read(offset, size);

            if revert_data.starts_with("08c379a0") {
                let revert_string = match revert_data.get(8..) {
                    Some(data) => match decode_hex(data) {
                        Ok(hex_data) => match decode(&[ParamType::String], &hex_data) {
                            Ok(revert) => revert[0].to_string(),
                            Err(_) => "decoding error".to_string(),
                        },
                        Err(_) => "decoding error".to_string(),
                    },
                    None => "".to_string(),
                };

                trace.add_error(
                    parent_index,
                    vm.instruction.try_into().unwrap(),
                    format!("execution reverted: {}", revert_string).to_string(),
                );
            }

            // handle case with panics
            else if revert_data.starts_with("4e487b71") {
                trace.add_error(
                    parent_index,
                    vm.instruction.try_into().unwrap(),
                    "panic".to_string(),
                );
            }

            else {
                let custom_error_placeholder = match revert_data.get(0..8) {
                    Some(selector) => {
                        format!(" CustomError_{}()", selector)
                    },
                    None => "".to_string(),
                };

                trace.add_error(
                    parent_index,
                    vm.instruction.try_into().unwrap(),
                    format!("execution reverted: {}", custom_error_placeholder).to_string(),
                );
            }
        }

        let state = vm.step();
        state_vec.push(state.clone());

        if vm.exitcode != 255 || vm.returndata.len() > 0 {
            println!("storage: {:#?}\nmemory: {:#?}", vm.storage, vm.memory);
            break;
        }
    }

    // update the trace
    //trace.instructions = state_vec;
    transaction_trace.result = Result {
        gas_used: vm.gas_used,
        gas_remaining: vm.gas_remaining,
        returndata: vm.returndata.to_owned(),
        exitcode: vm.exitcode,
        events: vm.events.clone(),
        runtime: vm.timestamp.elapsed().as_secs_f64(),
        instruction: vm.instruction,
    };

    transaction_trace
}