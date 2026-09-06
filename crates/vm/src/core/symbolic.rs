//! Interned symbolic expressions used by abstract CFG analysis.

use std::collections::{BTreeSet, HashMap};

use alloy::primitives::U256;

use super::{abstract_state::StateVersionId, opcodes, program::DecodedInstruction};

/// Stable identifier of an expression in an [`ExpressionArena`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExprId(usize);

impl ExprId {
    /// Return this expression's arena index.
    pub fn index(self) -> usize {
        self.0
    }
}

/// A stack value represented by constants, symbolic expressions, or top.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AbstractValue {
    /// A non-empty finite set of concrete values.
    Known(BTreeSet<U256>),
    /// One or more symbolic expressions, optionally joined with concrete alternatives.
    Symbolic {
        /// Concrete alternatives retained at this join.
        constants: BTreeSet<U256>,
        /// Symbolic alternatives retained at this join.
        expressions: BTreeSet<ExprId>,
    },
    /// Any 256-bit value.
    Unknown,
}

impl AbstractValue {
    /// Construct a singleton concrete value.
    pub fn constant(value: U256) -> Self {
        Self::Known(BTreeSet::from([value]))
    }

    /// Construct a singleton symbolic value.
    pub fn expression(expression: ExprId) -> Self {
        Self::Symbolic { constants: BTreeSet::new(), expressions: BTreeSet::from([expression]) }
    }

    /// Return all alternatives when the value is entirely concrete.
    pub fn known_values(&self) -> Option<&BTreeSet<U256>> {
        match self {
            Self::Known(values) => Some(values),
            Self::Symbolic { .. } | Self::Unknown => None,
        }
    }

    /// Return the symbolic alternatives retained in this value.
    pub fn expressions(&self) -> Option<&BTreeSet<ExprId>> {
        match self {
            Self::Symbolic { expressions, .. } => Some(expressions),
            Self::Known(_) | Self::Unknown => None,
        }
    }

    pub(crate) fn join(&self, other: &Self, max_values: usize) -> Self {
        if matches!(self, Self::Unknown) || matches!(other, Self::Unknown) {
            return Self::Unknown
        }

        let (mut constants, mut expressions) = self.alternatives();
        let (other_constants, other_expressions) = other.alternatives();
        constants.extend(other_constants);
        expressions.extend(other_expressions);
        if constants.len() + expressions.len() > max_values {
            Self::Unknown
        } else if expressions.is_empty() {
            Self::Known(constants)
        } else {
            Self::Symbolic { constants, expressions }
        }
    }

    fn alternatives(&self) -> (BTreeSet<U256>, BTreeSet<ExprId>) {
        match self {
            Self::Known(constants) => (constants.clone(), BTreeSet::new()),
            Self::Symbolic { constants, expressions } => (constants.clone(), expressions.clone()),
            Self::Unknown => (BTreeSet::new(), BTreeSet::new()),
        }
    }
}

/// One hash-consed symbolic EVM operation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExpressionNode {
    /// EVM opcode producing the value.
    pub opcode: u8,
    /// Abstract operands in EVM pop order.
    pub inputs: Vec<AbstractValue>,
    /// Output position for opcodes producing more than one value.
    pub output: u8,
    /// Memory or storage version read by a stateful operation.
    pub state_version: Option<StateVersionId>,
}

/// Hash-consed expression DAG shared by all states in one analysis.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExpressionArena {
    nodes: Vec<ExpressionNode>,
    sites: Vec<BTreeSet<usize>>,
    interned: HashMap<ExpressionNode, ExprId>,
}

impl ExpressionArena {
    /// Construct an empty expression arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a node, returning the existing identifier when it was already present.
    pub fn intern(&mut self, node: ExpressionNode) -> ExprId {
        self.intern_with_site(node, None)
    }

    /// Intern an expression and record a bytecode site that computes it.
    pub fn intern_at(&mut self, node: ExpressionNode, pc: usize) -> ExprId {
        self.intern_with_site(node, Some(pc))
    }

    fn intern_with_site(&mut self, node: ExpressionNode, pc: Option<usize>) -> ExprId {
        if let Some(id) = self.interned.get(&node).copied() {
            if let Some(pc) = pc {
                self.sites[id.0].insert(pc);
            }
            return id
        }
        let id = ExprId(self.nodes.len());
        self.nodes.push(node.clone());
        self.sites.push(pc.into_iter().collect());
        self.interned.insert(node, id);
        id
    }

    /// Look up an interned expression.
    pub fn get(&self, id: ExprId) -> Option<&ExpressionNode> {
        self.nodes.get(id.0)
    }

    /// Bytecode sites known to compute an expression.
    pub fn sites(&self, id: ExprId) -> Option<&BTreeSet<usize>> {
        self.sites.get(id.0)
    }

    /// Number of unique expression nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the arena contains no expressions.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Compute a deterministic opcode result or preserve it as a symbolic expression.
pub(crate) fn operation_result(
    instruction: &DecodedInstruction,
    inputs: Vec<AbstractValue>,
    output: u8,
    arena: &mut ExpressionArena,
    max_values: usize,
) -> AbstractValue {
    if inputs.iter().any(|input| matches!(input, AbstractValue::Unknown)) {
        return AbstractValue::Unknown
    }

    if let Some(values) = evaluate_constants(instruction.opcode, &inputs, max_values) {
        return AbstractValue::Known(values)
    }

    if is_symbolically_stable(instruction.opcode) {
        return AbstractValue::expression(arena.intern_at(
            ExpressionNode { opcode: instruction.opcode, inputs, output, state_version: None },
            instruction.pc,
        ))
    }

    AbstractValue::Unknown
}

/// Preserve a memory or storage read against a specific persistent state version.
pub(crate) fn stateful_operation_result(
    instruction: &DecodedInstruction,
    inputs: Vec<AbstractValue>,
    state_version: StateVersionId,
    arena: &mut ExpressionArena,
) -> AbstractValue {
    if inputs.iter().any(|input| matches!(input, AbstractValue::Unknown)) {
        return AbstractValue::Unknown
    }
    AbstractValue::expression(arena.intern_at(
        ExpressionNode {
            opcode: instruction.opcode,
            inputs,
            output: 0,
            state_version: Some(state_version),
        },
        instruction.pc,
    ))
}

fn evaluate_constants(
    opcode: u8,
    inputs: &[AbstractValue],
    max_values: usize,
) -> Option<BTreeSet<U256>> {
    let alternatives =
        inputs.iter().map(AbstractValue::known_values).collect::<Option<Vec<_>>>()?;
    if alternatives.iter().any(|values| values.is_empty()) {
        return None
    }

    let mut combinations = vec![Vec::new()];
    for values in alternatives {
        let mut next = Vec::new();
        for prefix in &combinations {
            for value in values {
                if next.len() > max_values.saturating_mul(max_values.max(1)) {
                    return None
                }
                let mut combination = prefix.clone();
                combination.push(*value);
                next.push(combination);
            }
        }
        combinations = next;
    }

    let mut results = BTreeSet::new();
    for inputs in combinations {
        results.insert(evaluate_constant(opcode, &inputs)?);
        if results.len() > max_values {
            return None
        }
    }
    Some(results)
}

fn evaluate_constant(opcode: u8, inputs: &[U256]) -> Option<U256> {
    let boolean = |value: bool| if value { U256::from(1) } else { U256::ZERO };
    let [a, b, ..] = inputs else {
        return match (opcode, inputs) {
            (opcodes::ISZERO, [value]) => Some(boolean(value.is_zero())),
            (opcodes::NOT, [value]) => Some(!*value),
            _ => None,
        }
    };

    match opcode {
        opcodes::ADD => Some(a.overflowing_add(*b).0),
        opcodes::MUL => Some(a.overflowing_mul(*b).0),
        opcodes::SUB => Some(a.overflowing_sub(*b).0),
        opcodes::DIV => Some(if b.is_zero() { U256::ZERO } else { *a / *b }),
        opcodes::MOD => Some(if b.is_zero() { U256::ZERO } else { *a % *b }),
        opcodes::EXP => Some(a.overflowing_pow(*b).0),
        opcodes::LT => Some(boolean(a < b)),
        opcodes::GT => Some(boolean(a > b)),
        opcodes::EQ => Some(boolean(a == b)),
        opcodes::AND => Some(*a & *b),
        opcodes::OR => Some(*a | *b),
        opcodes::XOR => Some(*a ^ *b),
        opcodes::BYTE => Some(if *a >= U256::from(32) {
            U256::ZERO
        } else {
            (*b >> (248usize.saturating_sub(usize::try_from(*a).ok()? * 8))) & U256::from(0xff)
        }),
        opcodes::SHL => Some(if *a >= U256::from(256) {
            U256::ZERO
        } else {
            b.overflowing_shl(usize::try_from(*a).ok()?).0
        }),
        opcodes::SHR => {
            Some(if *a >= U256::from(256) { U256::ZERO } else { *b >> usize::try_from(*a).ok()? })
        }
        opcodes::ADDMOD if inputs.len() == 3 => Some(if inputs[2].is_zero() {
            U256::ZERO
        } else {
            inputs[0].add_mod(inputs[1], inputs[2])
        }),
        opcodes::MULMOD if inputs.len() == 3 => Some(if inputs[2].is_zero() {
            U256::ZERO
        } else {
            inputs[0].mul_mod(inputs[1], inputs[2])
        }),
        _ => None,
    }
}

fn is_symbolically_stable(opcode: u8) -> bool {
    matches!(
        opcode,
        opcodes::ADD..=opcodes::SIGNEXTEND |
            opcodes::LT..=opcodes::SAR |
            opcodes::ADDRESS |
            opcodes::ORIGIN |
            opcodes::CALLER |
            opcodes::CALLVALUE |
            opcodes::CALLDATALOAD |
            opcodes::CALLDATASIZE |
            opcodes::GASPRICE |
            opcodes::BLOCKHASH |
            opcodes::COINBASE |
            opcodes::TIMESTAMP |
            opcodes::NUMBER |
            opcodes::PREVRANDAO |
            opcodes::GASLIMIT |
            opcodes::CHAINID |
            opcodes::BASEFEE |
            opcodes::BLOBHASH |
            opcodes::BLOBBASEFEE
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instruction(pc: usize, opcode: u8) -> DecodedInstruction {
        DecodedInstruction { pc, opcode, immediate: Vec::new(), truncated: false }
    }

    #[test]
    fn interns_structurally_equal_expressions() {
        let mut arena = ExpressionArena::new();
        let node = ExpressionNode {
            opcode: opcodes::CALLER,
            inputs: vec![],
            output: 0,
            state_version: None,
        };
        let id = arena.intern_at(node.clone(), 7);
        assert_eq!(id, arena.intern_at(node, 11));
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.sites(id), Some(&BTreeSet::from([7, 11])));
    }

    #[test]
    fn folds_evm_modular_arithmetic() {
        let mut arena = ExpressionArena::new();
        let result = operation_result(
            &instruction(0, opcodes::ADD),
            vec![AbstractValue::constant(U256::MAX), AbstractValue::constant(U256::from(1))],
            0,
            &mut arena,
            8,
        );
        assert_eq!(result, AbstractValue::constant(U256::ZERO));
        assert!(arena.is_empty());
    }

    #[test]
    fn preserves_operations_over_symbolic_inputs() {
        let mut arena = ExpressionArena::new();
        let caller = operation_result(&instruction(0, opcodes::CALLER), vec![], 0, &mut arena, 8);
        let masked = operation_result(
            &instruction(1, opcodes::AND),
            vec![caller.clone(), AbstractValue::constant(U256::from(0xffff))],
            0,
            &mut arena,
            8,
        );

        assert!(matches!(caller, AbstractValue::Symbolic { .. }));
        assert!(matches!(masked, AbstractValue::Symbolic { .. }));
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn joins_concrete_and_symbolic_alternatives_without_losing_either() {
        let mut arena = ExpressionArena::new();
        let symbolic = operation_result(&instruction(4, opcodes::CALLER), vec![], 0, &mut arena, 8);
        let joined = AbstractValue::constant(U256::from(7)).join(&symbolic, 8);

        let AbstractValue::Symbolic { constants, expressions } = joined else {
            panic!("mixed join should remain symbolic")
        };
        assert_eq!(constants, BTreeSet::from([U256::from(7)]));
        assert_eq!(expressions.len(), 1);
    }

    #[test]
    fn does_not_conflate_stateful_reads() {
        let mut arena = ExpressionArena::new();
        let result = operation_result(
            &instruction(0, opcodes::SLOAD),
            vec![AbstractValue::constant(U256::ZERO)],
            0,
            &mut arena,
            8,
        );
        assert_eq!(result, AbstractValue::Unknown);
        assert!(arena.is_empty());
    }
}
