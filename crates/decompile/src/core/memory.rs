use heimdall_vm::core::{
    opcodes::{self, WrappedOpcode},
    vm::State,
};

/// Bytes proven to have a concrete value on one explored execution path.
#[derive(Clone, Debug, Default)]
pub(crate) struct MemoryProvenance {
    known: Vec<bool>,
}

impl MemoryProvenance {
    pub(crate) fn is_constant(op: &WrappedOpcode) -> bool {
        (opcodes::PUSH0..=opcodes::PUSH32).contains(&op.opcode)
    }

    pub(crate) fn argument(state: &State, index: usize) -> Option<usize> {
        let instruction = &state.last_instruction;
        let operation = instruction.input_operations.get(index)?;
        if !Self::is_constant(operation) {
            return None;
        }
        instruction.inputs.get(index)?.try_into().ok()
    }

    pub(crate) fn argument_range(
        &self,
        state: &State,
        offset_index: usize,
        size_index: usize,
    ) -> Option<(usize, usize)> {
        let size = Self::argument(state, size_index)?;
        if size == 0 {
            return Some((0, 0));
        }
        let offset = Self::argument(state, offset_index)?;
        state.memory.try_read(offset, size)?;
        Some((offset, size))
    }

    fn record(&mut self, offset: usize, size: usize, known: bool) {
        if size == 0 {
            return;
        }
        let Some(end) = offset.checked_add(size) else {
            self.known.clear();
            return;
        };
        self.known.resize(self.known.len().max(end), false);
        self.known[offset..end].fill(known);
    }

    fn clear(&mut self) {
        self.known.clear();
    }

    pub(crate) fn contains(&self, offset: usize, size: usize) -> bool {
        offset.checked_add(size).is_some_and(|end| {
            size == 0 || self.known.get(offset..end).is_some_and(|bytes| bytes.iter().all(|v| *v))
        })
    }

    /// Update knownness after a memory-writing instruction and before processing its successors.
    pub(crate) fn observe_write(&mut self, state: &State) {
        let instruction = &state.last_instruction;
        match instruction.opcode {
            opcodes::MSTORE | opcodes::MSTORE8 => {
                let size = if instruction.opcode == opcodes::MSTORE { 32 } else { 1 };
                let offset = Self::argument(state, 0);
                let value_known =
                    instruction.input_operations.get(1).is_some_and(Self::is_constant);
                if let Some(offset) =
                    offset.filter(|offset| state.memory.try_read(*offset, size).is_some())
                {
                    self.record(offset, size, value_known);
                } else {
                    self.clear();
                }
            }
            opcodes::CALLDATACOPY | opcodes::CODECOPY | opcodes::RETURNDATACOPY => {
                if let Some((offset, size)) = self.argument_range(state, 0, 2) {
                    self.record(offset, size, false);
                } else {
                    self.clear();
                }
            }
            // The VM's EXTCODECOPY stub may write more than the requested size.
            opcodes::EXTCODECOPY => self.clear(),
            opcodes::MCOPY => {
                if Self::argument(state, 2) == Some(0) {
                    return;
                }
                if let (Some((dest, size)), Some(source)) =
                    (self.argument_range(state, 0, 2), Self::argument(state, 1))
                {
                    if state.memory.try_read(source, size).is_some() {
                        let source_known = self.contains(source, size);
                        self.record(dest, size, source_known);
                    } else {
                        self.clear();
                    }
                } else {
                    self.clear();
                }
            }
            _ => {}
        }
    }

    /// Calls may write returned bytes even when the VM uses a stubbed callee.
    pub(crate) fn observe_call_output(&mut self, state: &State) {
        let offset_index = match state.last_instruction.opcode {
            opcodes::CALL | opcodes::CALLCODE => 5,
            opcodes::DELEGATECALL | opcodes::STATICCALL => 4,
            _ => return,
        };
        if let Some((offset, size)) = self.argument_range(state, offset_index, offset_index + 1) {
            self.record(offset, size, false);
        } else {
            self.clear();
        }
    }
}
