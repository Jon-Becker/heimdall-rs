use eyre::Result;

use crate::core::log::Log;

use super::super::core::VM;

/// LOG0-LOG4 - Append log record with N topics
pub fn log_n(vm: &mut VM, topic_count: u8) -> Result<()> {
    let offset = vm.stack.pop()?.value;
    let size = vm.stack.pop()?.value;
    let topics = vm.stack.pop_n(topic_count as usize)?.iter().map(|x| x.value).collect();

    // Safely convert U256 to usize
    let offset: usize = offset.try_into().unwrap_or(usize::MAX);
    let size: usize = size.try_into().unwrap_or(usize::MAX);

    let data = vm.memory.read(offset, size);

    // consume dynamic gas
    let gas_cost = (375 * (topic_count as u128)) +
        8 * (size as u128) +
        vm.memory.expansion_cost(offset, size);
    vm.consume_gas(gas_cost);

    // no need for a panic check because the length of events should never be larger
    // than a u128
    vm.events.push(Log::new(
        vm.events
            .len()
            .try_into()
            .expect("impossible case: log_index is larger than u128::MAX"),
        topics,
        &data,
    ));
    Ok(())
}
