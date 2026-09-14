//! Lightweight path predicates and bit-vector range refinement.

use std::collections::{BTreeMap, BTreeSet};

use alloy::primitives::U256;

use super::{
    opcodes,
    symbolic::{AbstractValue, ExprId, ExpressionArena},
};

/// Facts known about one symbolic 256-bit value on an execution path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueConstraint {
    /// Inclusive unsigned lower bound.
    pub min: U256,
    /// Inclusive unsigned upper bound.
    pub max: U256,
    /// Values proven not equal to the expression.
    pub excluded: BTreeSet<U256>,
    /// Bits proven to be zero.
    pub known_zero_bits: U256,
    /// Bits proven to be one.
    pub known_one_bits: U256,
}

impl Default for ValueConstraint {
    fn default() -> Self {
        Self {
            min: U256::ZERO,
            max: U256::MAX,
            excluded: BTreeSet::new(),
            known_zero_bits: U256::ZERO,
            known_one_bits: U256::ZERO,
        }
    }
}

impl ValueConstraint {
    /// Return an exact value when the current facts identify one.
    pub fn exact_value(&self) -> Option<U256> {
        (self.min == self.max).then_some(self.min)
    }

    fn is_unconstrained(&self) -> bool {
        self == &Self::default()
    }

    fn is_consistent(&self) -> bool {
        self.min <= self.max &&
            self.known_zero_bits & self.known_one_bits == U256::ZERO &&
            self.exact_value().is_none_or(|value| {
                !self.excluded.contains(&value) &&
                    value & self.known_zero_bits == U256::ZERO &&
                    value & self.known_one_bits == self.known_one_bits
            })
    }

    fn join(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            excluded: self.excluded.intersection(&other.excluded).copied().collect(),
            known_zero_bits: self.known_zero_bits & other.known_zero_bits,
            known_one_bits: self.known_one_bits & other.known_one_bits,
        }
    }
}

/// Path facts retained by an abstract state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PathFacts {
    constraints: BTreeMap<ExprId, ValueConstraint>,
    equalities: BTreeSet<(ExprId, ExprId)>,
    disequalities: BTreeSet<(ExprId, ExprId)>,
}

impl PathFacts {
    /// Construct an unconstrained path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Constraint currently known for an expression.
    pub fn constraint(&self, expression: ExprId) -> Option<&ValueConstraint> {
        self.constraints.get(&expression)
    }

    /// Whether two expressions are known equal.
    pub fn are_equal(&self, left: ExprId, right: ExprId) -> bool {
        left == right || self.equalities.contains(&ordered_pair(left, right))
    }

    /// Whether two expressions are known different.
    pub fn are_different(&self, left: ExprId, right: ExprId) -> bool {
        self.disequalities.contains(&ordered_pair(left, right))
    }

    #[cfg(feature = "smt")]
    pub(crate) fn constraints(&self) -> &BTreeMap<ExprId, ValueConstraint> {
        &self.constraints
    }

    #[cfg(feature = "smt")]
    pub(crate) fn equalities(&self) -> &BTreeSet<(ExprId, ExprId)> {
        &self.equalities
    }

    #[cfg(feature = "smt")]
    pub(crate) fn disequalities(&self) -> &BTreeSet<(ExprId, ExprId)> {
        &self.disequalities
    }

    /// Intersect path knowledge at a control-flow join.
    pub(crate) fn join(&self, other: &Self) -> Self {
        let constraints = self
            .constraints
            .iter()
            .filter_map(|(expression, left)| {
                let joined = left.join(other.constraints.get(expression)?);
                (!joined.is_unconstrained()).then_some((*expression, joined))
            })
            .collect();
        Self {
            constraints,
            equalities: self.equalities.intersection(&other.equalities).copied().collect(),
            disequalities: self.disequalities.intersection(&other.disequalities).copied().collect(),
        }
    }

    /// Refine this path by assuming a branch condition is true or false.
    ///
    /// Returns false when the assumption contradicts existing facts.
    pub fn assume(
        &mut self,
        condition: &AbstractValue,
        taken: bool,
        expressions: &ExpressionArena,
    ) -> bool {
        if !condition_may_be(condition, taken, self, expressions) {
            return false
        }
        let Some(Atom::Expr(expression)) = singleton_atom(condition) else { return true };
        self.assume_expression(expression, taken, expressions)
    }

    fn assume_expression(
        &mut self,
        expression: ExprId,
        taken: bool,
        expressions: &ExpressionArena,
    ) -> bool {
        if !self.update(expression, |constraint| {
            if taken {
                constraint.excluded.insert(U256::ZERO);
            } else {
                set_exact(constraint, U256::ZERO);
            }
        }) {
            return false
        }

        let Some(node) = expressions.get(expression) else { return true };
        match node.opcode {
            opcodes::ISZERO => {
                node.inputs.first().is_none_or(|input| self.assume(input, !taken, expressions))
            }
            opcodes::EQ if node.inputs.len() == 2 => {
                self.assume_equality(&node.inputs[0], &node.inputs[1], taken, expressions)
            }
            opcodes::LT if node.inputs.len() == 2 => {
                self.assume_order(&node.inputs[0], &node.inputs[1], taken, true)
            }
            opcodes::GT if node.inputs.len() == 2 => {
                self.assume_order(&node.inputs[0], &node.inputs[1], taken, false)
            }
            _ => true,
        }
    }

    fn assume_equality(
        &mut self,
        left: &AbstractValue,
        right: &AbstractValue,
        equal: bool,
        expressions: &ExpressionArena,
    ) -> bool {
        let (Some(left), Some(right)) = (singleton_atom(left), singleton_atom(right)) else {
            return true
        };
        match (left, right) {
            (Atom::Const(left), Atom::Const(right)) => (left == right) == equal,
            (Atom::Expr(expression), Atom::Const(value)) |
            (Atom::Const(value), Atom::Expr(expression)) => {
                let consistent = self.update(expression, |constraint| {
                    if equal {
                        set_exact(constraint, value);
                    } else {
                        constraint.excluded.insert(value);
                    }
                });
                consistent &&
                    (!equal || self.propagate_mask_equality(expression, value, expressions))
            }
            (Atom::Expr(left), Atom::Expr(right)) => {
                let pair = ordered_pair(left, right);
                if equal {
                    if self.disequalities.contains(&pair) {
                        return false
                    }
                    self.equalities.insert(pair);
                    self.merge_equal_constraints(left, right)
                } else {
                    if self.are_equal(left, right) {
                        return false
                    }
                    self.disequalities.insert(pair);
                    true
                }
            }
        }
    }

    fn assume_order(
        &mut self,
        left: &AbstractValue,
        right: &AbstractValue,
        relation_holds: bool,
        less_than: bool,
    ) -> bool {
        let (Some(left), Some(right)) = (singleton_atom(left), singleton_atom(right)) else {
            return true
        };
        let (smaller, larger) = if less_than { (left, right) } else { (right, left) };
        match (smaller, larger, relation_holds) {
            (Atom::Const(a), Atom::Const(b), holds) => (a < b) == holds,
            (Atom::Expr(expression), Atom::Const(bound), true) => {
                let Some(max) = bound.checked_sub(U256::from(1)) else { return false };
                self.update(expression, |constraint| constraint.max = constraint.max.min(max))
            }
            (Atom::Expr(expression), Atom::Const(bound), false) => {
                self.update(expression, |constraint| constraint.min = constraint.min.max(bound))
            }
            (Atom::Const(bound), Atom::Expr(expression), true) => {
                let Some(min) = bound.checked_add(U256::from(1)) else { return false };
                self.update(expression, |constraint| constraint.min = constraint.min.max(min))
            }
            (Atom::Const(bound), Atom::Expr(expression), false) => {
                self.update(expression, |constraint| constraint.max = constraint.max.min(bound))
            }
            (Atom::Expr(_), Atom::Expr(_), _) => true,
        }
    }

    fn propagate_mask_equality(
        &mut self,
        expression: ExprId,
        value: U256,
        expressions: &ExpressionArena,
    ) -> bool {
        let Some(node) = expressions.get(expression) else { return true };
        if node.opcode != opcodes::AND || node.inputs.len() != 2 {
            return true
        }
        let Some((masked, mask)) = expression_and_constant(&node.inputs[0], &node.inputs[1]) else {
            return true
        };
        if value & !mask != U256::ZERO {
            return false
        }
        self.update(masked, |constraint| {
            constraint.known_one_bits |= value;
            constraint.known_zero_bits |= mask & !value;
        })
    }

    fn merge_equal_constraints(&mut self, left: ExprId, right: ExprId) -> bool {
        let left_constraint = self.constraints.get(&left).cloned().unwrap_or_default();
        let right_constraint = self.constraints.get(&right).cloned().unwrap_or_default();
        let merged = intersect_constraints(left_constraint, right_constraint);
        if !merged.is_consistent() {
            return false
        }
        if !merged.is_unconstrained() {
            self.constraints.insert(left, merged.clone());
            self.constraints.insert(right, merged);
        }
        true
    }

    fn update(&mut self, expression: ExprId, update: impl FnOnce(&mut ValueConstraint)) -> bool {
        let constraint = self.constraints.entry(expression).or_default();
        update(constraint);
        constraint.is_consistent()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Atom {
    Const(U256),
    Expr(ExprId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Truthiness {
    True,
    False,
    Unknown,
}

/// Determine whether a condition can have the requested branch truth value.
pub fn condition_may_be(
    condition: &AbstractValue,
    taken: bool,
    facts: &PathFacts,
    expressions: &ExpressionArena,
) -> bool {
    match truthiness(condition, facts, expressions, &mut BTreeSet::new()) {
        Truthiness::True => taken,
        Truthiness::False => !taken,
        Truthiness::Unknown => true,
    }
}

fn truthiness(
    value: &AbstractValue,
    facts: &PathFacts,
    expressions: &ExpressionArena,
    visiting: &mut BTreeSet<ExprId>,
) -> Truthiness {
    match value {
        AbstractValue::Known(values) => from_possibilities(
            values.iter().any(|value| !value.is_zero()),
            values.contains(&U256::ZERO),
        ),
        AbstractValue::Unknown => Truthiness::Unknown,
        AbstractValue::Symbolic { constants, expressions: alternatives } => {
            let mut may_true = constants.iter().any(|value| !value.is_zero());
            let mut may_false = constants.contains(&U256::ZERO);
            for expression in alternatives {
                match expression_truthiness(*expression, facts, expressions, visiting) {
                    Truthiness::True => may_true = true,
                    Truthiness::False => may_false = true,
                    Truthiness::Unknown => {
                        may_true = true;
                        may_false = true;
                    }
                }
            }
            from_possibilities(may_true, may_false)
        }
    }
}

fn expression_truthiness(
    expression: ExprId,
    facts: &PathFacts,
    expressions: &ExpressionArena,
    visiting: &mut BTreeSet<ExprId>,
) -> Truthiness {
    if !visiting.insert(expression) {
        return Truthiness::Unknown
    }
    let result = if let Some(constraint) = facts.constraint(expression) {
        if constraint.exact_value() == Some(U256::ZERO) {
            Truthiness::False
        } else if constraint.min > U256::ZERO || constraint.excluded.contains(&U256::ZERO) {
            Truthiness::True
        } else {
            expression_semantics(expression, facts, expressions, visiting)
        }
    } else {
        expression_semantics(expression, facts, expressions, visiting)
    };
    visiting.remove(&expression);
    result
}

fn expression_semantics(
    expression: ExprId,
    facts: &PathFacts,
    expressions: &ExpressionArena,
    visiting: &mut BTreeSet<ExprId>,
) -> Truthiness {
    let Some(node) = expressions.get(expression) else { return Truthiness::Unknown };
    match node.opcode {
        opcodes::ISZERO => {
            node.inputs.first().map_or(Truthiness::Unknown, |input| {
                match truthiness(input, facts, expressions, visiting) {
                    Truthiness::True => Truthiness::False,
                    Truthiness::False => Truthiness::True,
                    Truthiness::Unknown => Truthiness::Unknown,
                }
            })
        }
        opcodes::EQ if node.inputs.len() == 2 => {
            compare_equality(&node.inputs[0], &node.inputs[1], facts)
        }
        opcodes::LT if node.inputs.len() == 2 => {
            compare_order(&node.inputs[0], &node.inputs[1], facts, true)
        }
        opcodes::GT if node.inputs.len() == 2 => {
            compare_order(&node.inputs[0], &node.inputs[1], facts, false)
        }
        opcodes::AND if node.inputs.len() == 2 => {
            let Some((expression, mask)) =
                expression_and_constant(&node.inputs[0], &node.inputs[1])
            else {
                return Truthiness::Unknown
            };
            let constraint = facts.constraint(expression).cloned().unwrap_or_default();
            if constraint.known_zero_bits & mask == mask {
                Truthiness::False
            } else if constraint.known_one_bits & mask != U256::ZERO {
                Truthiness::True
            } else {
                Truthiness::Unknown
            }
        }
        _ => Truthiness::Unknown,
    }
}

fn compare_equality(left: &AbstractValue, right: &AbstractValue, facts: &PathFacts) -> Truthiness {
    let (Some(left), Some(right)) = (singleton_atom(left), singleton_atom(right)) else {
        return Truthiness::Unknown
    };
    match (left, right) {
        (Atom::Const(left), Atom::Const(right)) => bool_truth(left == right),
        (Atom::Expr(left), Atom::Expr(right)) if facts.are_equal(left, right) => Truthiness::True,
        (Atom::Expr(left), Atom::Expr(right)) if facts.are_different(left, right) => {
            Truthiness::False
        }
        (Atom::Expr(left), Atom::Expr(right)) => {
            let left = facts.constraint(left).cloned().unwrap_or_default();
            let right = facts.constraint(right).cloned().unwrap_or_default();
            if left.max < right.min || right.max < left.min {
                Truthiness::False
            } else {
                Truthiness::Unknown
            }
        }
        (Atom::Expr(expression), Atom::Const(value)) |
        (Atom::Const(value), Atom::Expr(expression)) => {
            let constraint = facts.constraint(expression).cloned().unwrap_or_default();
            if constraint.exact_value() == Some(value) {
                Truthiness::True
            } else if value < constraint.min ||
                value > constraint.max ||
                constraint.excluded.contains(&value)
            {
                Truthiness::False
            } else {
                Truthiness::Unknown
            }
        }
    }
}

fn compare_order(
    left: &AbstractValue,
    right: &AbstractValue,
    facts: &PathFacts,
    less_than: bool,
) -> Truthiness {
    let (Some(left), Some(right)) = (singleton_atom(left), singleton_atom(right)) else {
        return Truthiness::Unknown
    };
    let (left, right) = if less_than { (left, right) } else { (right, left) };
    let left = atom_constraint(left, facts);
    let right = atom_constraint(right, facts);
    if left.max < right.min {
        Truthiness::True
    } else if left.min >= right.max {
        Truthiness::False
    } else {
        Truthiness::Unknown
    }
}

fn atom_constraint(atom: Atom, facts: &PathFacts) -> ValueConstraint {
    match atom {
        Atom::Const(value) => ValueConstraint { min: value, max: value, ..Default::default() },
        Atom::Expr(expression) => facts.constraint(expression).cloned().unwrap_or_default(),
    }
}

fn singleton_atom(value: &AbstractValue) -> Option<Atom> {
    match value {
        AbstractValue::Known(values) if values.len() == 1 => {
            values.first().copied().map(Atom::Const)
        }
        AbstractValue::Symbolic { constants, expressions }
            if constants.is_empty() && expressions.len() == 1 =>
        {
            expressions.first().copied().map(Atom::Expr)
        }
        _ => None,
    }
}

fn expression_and_constant(left: &AbstractValue, right: &AbstractValue) -> Option<(ExprId, U256)> {
    match (singleton_atom(left)?, singleton_atom(right)?) {
        (Atom::Expr(expression), Atom::Const(mask)) |
        (Atom::Const(mask), Atom::Expr(expression)) => Some((expression, mask)),
        _ => None,
    }
}

fn intersect_constraints(mut left: ValueConstraint, right: ValueConstraint) -> ValueConstraint {
    left.min = left.min.max(right.min);
    left.max = left.max.min(right.max);
    left.excluded.extend(right.excluded);
    left.known_zero_bits |= right.known_zero_bits;
    left.known_one_bits |= right.known_one_bits;
    left
}

fn set_exact(constraint: &mut ValueConstraint, value: U256) {
    constraint.min = constraint.min.max(value);
    constraint.max = constraint.max.min(value);
    constraint.known_one_bits |= value;
    constraint.known_zero_bits |= !value;
}

fn ordered_pair(left: ExprId, right: ExprId) -> (ExprId, ExprId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn from_possibilities(may_true: bool, may_false: bool) -> Truthiness {
    match (may_true, may_false) {
        (true, false) => Truthiness::True,
        (false, true) => Truthiness::False,
        _ => Truthiness::Unknown,
    }
}

fn bool_truth(value: bool) -> Truthiness {
    if value {
        Truthiness::True
    } else {
        Truthiness::False
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::symbolic::ExpressionNode;

    fn expression(
        arena: &mut ExpressionArena,
        opcode: u8,
        inputs: Vec<AbstractValue>,
    ) -> AbstractValue {
        let id = arena.intern(ExpressionNode {
            opcode,
            inputs,
            output: 0,
            state_version: None,
            effect_site: None,
        });
        AbstractValue::expression(id)
    }

    fn id(value: &AbstractValue) -> ExprId {
        *value.expressions().expect("symbolic value").first().expect("expression")
    }

    #[test]
    fn propagates_zero_facts_through_iszero() {
        let mut arena = ExpressionArena::new();
        let caller = expression(&mut arena, opcodes::CALLER, vec![]);
        let condition = expression(&mut arena, opcodes::ISZERO, vec![caller.clone()]);
        let mut facts = PathFacts::new();

        assert!(facts.assume(&condition, true, &arena));
        assert_eq!(
            facts.constraint(id(&caller)).and_then(ValueConstraint::exact_value),
            Some(U256::ZERO)
        );
        assert!(!condition_may_be(&caller, true, &facts, &arena));
    }

    #[test]
    fn narrows_unsigned_ranges_from_comparisons() {
        let mut arena = ExpressionArena::new();
        let size = expression(&mut arena, opcodes::CALLDATASIZE, vec![]);
        let condition = expression(
            &mut arena,
            opcodes::LT,
            vec![size.clone(), AbstractValue::constant(U256::from(36))],
        );
        let mut facts = PathFacts::new();

        assert!(facts.assume(&condition, false, &arena));
        assert_eq!(facts.constraint(id(&size)).expect("size range").min, U256::from(36));
        assert!(!condition_may_be(&condition, true, &facts, &arena));
    }

    #[test]
    fn records_equality_and_disequality_relations() {
        let mut arena = ExpressionArena::new();
        let left = expression(&mut arena, opcodes::CALLER, vec![]);
        let right = expression(&mut arena, opcodes::ORIGIN, vec![]);
        let condition = expression(&mut arena, opcodes::EQ, vec![left.clone(), right.clone()]);
        let mut equal = PathFacts::new();
        let mut different = PathFacts::new();

        assert!(equal.assume(&condition, true, &arena));
        assert!(equal.are_equal(id(&left), id(&right)));
        assert!(different.assume(&condition, false, &arena));
        assert!(different.are_different(id(&left), id(&right)));
    }

    #[test]
    fn propagates_masked_bit_constraints() {
        let mut arena = ExpressionArena::new();
        let value = expression(
            &mut arena,
            opcodes::CALLDATALOAD,
            vec![AbstractValue::constant(U256::from(4))],
        );
        let masked = expression(
            &mut arena,
            opcodes::AND,
            vec![value.clone(), AbstractValue::constant(U256::from(0xff))],
        );
        let condition = expression(
            &mut arena,
            opcodes::EQ,
            vec![masked, AbstractValue::constant(U256::from(0x42))],
        );
        let mut facts = PathFacts::new();

        assert!(facts.assume(&condition, true, &arena));
        let constraint = facts.constraint(id(&value)).expect("masked value facts");
        assert_eq!(constraint.known_one_bits & U256::from(0xff), U256::from(0x42));
        assert_eq!(constraint.known_zero_bits & U256::from(0xff), U256::from(0xbd));
    }

    #[test]
    fn joins_only_facts_shared_by_both_paths() {
        let mut arena = ExpressionArena::new();
        let value = expression(&mut arena, opcodes::CALLER, vec![]);
        let mut left = PathFacts::new();
        let mut right = PathFacts::new();
        left.update(id(&value), |constraint| constraint.min = U256::from(10));
        right.update(id(&value), |constraint| constraint.min = U256::from(20));

        let joined = left.join(&right);
        assert_eq!(joined.constraint(id(&value)).expect("joined range").min, U256::from(10));
    }

    #[test]
    fn rejects_contradictory_assumptions() {
        let mut arena = ExpressionArena::new();
        let value = expression(&mut arena, opcodes::CALLER, vec![]);
        let mut facts = PathFacts::new();
        assert!(facts.assume(&value, false, &arena));
        assert!(!facts.assume(&value, true, &arena));

        let mut ranged = PathFacts::new();
        assert!(ranged.update(id(&value), |constraint| constraint.min = U256::from(10)));
        assert!(!ranged.assume(&value, false, &arena));
    }
}
