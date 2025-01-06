use alloy::primitives::U256;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct JumpFrame {
    pub pc: u128,
    pub jumpdest: U256,
    pub stack_depth: usize,
    pub jump_taken: bool,
}

impl JumpFrame {
    pub(super) fn new(pc: u128, jumpdest: U256, stack_depth: usize, jump_taken: bool) -> Self {
        Self { pc, jumpdest, stack_depth, jump_taken }
    }
}
