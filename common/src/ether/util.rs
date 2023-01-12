use super::evm::vm::{State, Result, VM, Block};

pub fn simulate(
    contract_address: String,
    calldata: String,
    contract_bytecode: String,
    from_address: String,
    value: u128,
    gas_remaining: u128,
    block: Block,
) -> (Result, Vec<State>) {
    let mut state_vec = Vec::new();

    // make a new VM object
    let mut vm = VM::new(
        contract_bytecode,
        calldata,
        contract_address,
        from_address.clone(),
        from_address,
        value,
        gas_remaining,
    );

    // update the block
    vm.block = block;

    // run the VM
    while vm.bytecode.len() >= (vm.instruction * 2 + 2) as usize {

        // first peek, and check if override is needed
        let next_operation = vm.peek();

        if vec!["STATICCALL", "DELEGATECALL"].contains(&next_operation.0.name.as_str()) {
            println!("{:#?}", next_operation);
        }

        let state = vm.step();
        state_vec.push(state.clone());

        if vm.exitcode != 255 || vm.returndata.len() > 0 {
            break;
        }
    }

    // update the result
    let result = Result {
        gas_used: vm.gas_used,
        gas_remaining: vm.gas_remaining,
        returndata: vm.returndata.to_owned(),
        exitcode: vm.exitcode,
        events: vm.events.clone(),
        runtime: vm.timestamp.elapsed().as_secs_f64(),
        instruction: vm.instruction,
    };

    (result, state_vec)
}