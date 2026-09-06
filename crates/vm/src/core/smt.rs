//! Demand-driven SMT refinement for symbolic branches and jump targets.
//!
//! This optional module translates the expression DAG and lightweight path facts to 256-bit Z3
//! bit-vectors. Queries are issued only after the cheap abstract domains cannot decide a branch or
//! jump target.

use std::{collections::HashMap, time::Duration};

use alloy::primitives::U256;
use z3::{
    ast::{Bool, BV},
    with_z3_config, Config, SatResult, Solver,
};

use super::{
    facts::PathFacts,
    opcodes,
    program::Program,
    symbolic::{AbstractValue, ExprId, ExpressionArena, ExpressionNode},
};

/// Default timeout for one SMT query.
pub const DEFAULT_QUERY_TIMEOUT_MS: u64 = 25;
/// Default maximum jump targets enumerated by one query.
pub const DEFAULT_MAX_TARGETS: usize = 16;
/// Default maximum number of SMT queries issued by one CFG analysis.
pub const DEFAULT_MAX_QUERIES: usize = 256;

/// Limits for demand-driven SMT refinement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmtConfig {
    /// Timeout applied independently to each query.
    pub timeout_ms: u64,
    /// Maximum models enumerated for one symbolic jump target.
    pub max_targets: usize,
    /// Maximum refinement queries issued by one CFG analysis.
    pub max_queries: usize,
}

impl Default for SmtConfig {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_QUERY_TIMEOUT_MS,
            max_targets: DEFAULT_MAX_TARGETS,
            max_queries: DEFAULT_MAX_QUERIES,
        }
    }
}

/// Aggregate solver activity for one CFG analysis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SmtStats {
    /// Number of refinement queries issued.
    pub queries: usize,
    /// Queries that returned unknown, timed out, or used an unsupported expression.
    pub inconclusive: usize,
    /// Branch directions proven infeasible.
    pub infeasible_branches: usize,
    /// Concrete jump destinations recovered from models.
    pub resolved_targets: usize,
    /// Wall-clock time spent in solver-backed refinement.
    pub elapsed: Duration,
    /// Whether additional queries were skipped after reaching the analysis budget.
    pub budget_exhausted: bool,
}

/// Result of bounded jump-target enumeration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetModels {
    /// Feasible valid JUMPDEST byte offsets.
    pub targets: Vec<U256>,
    /// Whether all feasible targets were enumerated within the model limit.
    pub complete: bool,
}

/// Stateful, statistics-producing SMT query interface.
#[derive(Clone, Debug)]
pub struct SmtRefiner {
    config: SmtConfig,
    stats: SmtStats,
}

impl SmtRefiner {
    /// Construct a refiner with explicit limits.
    pub fn new(config: SmtConfig) -> Self {
        Self { config, stats: SmtStats::default() }
    }

    /// Current aggregate query statistics.
    pub fn stats(&self) -> SmtStats {
        self.stats
    }

    /// Determine whether a branch direction is feasible.
    ///
    /// Returns `None` when translation is unsupported or Z3 is inconclusive.
    pub fn branch_feasible(
        &mut self,
        condition: &AbstractValue,
        taken: bool,
        facts: &PathFacts,
        expressions: &ExpressionArena,
    ) -> Option<bool> {
        if !self.reserve_query() {
            return None
        }
        let start = std::time::Instant::now();
        let result = with_timeout(self.config.timeout_ms, || {
            let mut translator = Translator::new(expressions);
            let condition = translator.value(condition)?;
            let solver = Solver::new();
            translator.assert_facts(&solver, facts);
            solver.assert(if taken {
                condition.ne(bv(U256::ZERO))
            } else {
                condition.eq(bv(U256::ZERO))
            });
            Some(solver.check())
        });
        self.stats.elapsed += start.elapsed();
        match result {
            Some(SatResult::Sat) => Some(true),
            Some(SatResult::Unsat) => {
                self.stats.infeasible_branches += 1;
                Some(false)
            }
            Some(SatResult::Unknown) | None => {
                self.stats.inconclusive += 1;
                None
            }
        }
    }

    /// Enumerate feasible valid jump destinations for a symbolic target.
    ///
    /// Returns `None` when no symbolic alternative can be translated or Z3 is inconclusive.
    pub fn jump_targets(
        &mut self,
        target: &AbstractValue,
        facts: &PathFacts,
        expressions: &ExpressionArena,
        program: &Program,
    ) -> Option<TargetModels> {
        if !self.reserve_query() {
            return None
        }
        let start = std::time::Instant::now();
        let result = with_timeout(self.config.timeout_ms, || {
            let valid = program
                .blocks
                .iter()
                .filter(|block| program.is_valid_jumpdest(block.start_pc))
                .map(|block| U256::from(block.start_pc))
                .collect::<Vec<_>>();
            if valid.is_empty() {
                return Some(TargetModels { targets: Vec::new(), complete: true })
            }

            let (constants, symbolic) = alternatives(target);
            let mut targets = constants
                .into_iter()
                .filter(|target| {
                    usize::try_from(*target).ok().is_some_and(|pc| program.is_valid_jumpdest(pc))
                })
                .collect::<Vec<_>>();
            let mut translated_any = false;
            let mut complete = true;

            for expression in symbolic {
                if targets.len() >= self.config.max_targets {
                    complete = false;
                    break
                }
                let mut translator = Translator::new(expressions);
                let Some(target) = translator.expression(expression) else { continue };
                translated_any = true;
                let solver = Solver::new();
                translator.assert_facts(&solver, facts);
                let candidates =
                    valid.iter().map(|candidate| target.eq(bv(*candidate))).collect::<Vec<_>>();
                solver.assert(Bool::or(&candidates));

                loop {
                    match solver.check() {
                        SatResult::Sat => {
                            let model = solver.get_model()?;
                            let value =
                                model.eval(&target, true).and_then(|value| parse_bv(&value))?;
                            if !targets.contains(&value) {
                                targets.push(value);
                            }
                            solver.assert(target.ne(bv(value)));
                            if targets.len() >= self.config.max_targets {
                                complete = solver.check() == SatResult::Unsat;
                                break
                            }
                        }
                        SatResult::Unsat => break,
                        SatResult::Unknown => return None,
                    }
                }
            }

            if !translated_any && targets.is_empty() {
                None
            } else {
                targets.sort_unstable();
                targets.dedup();
                Some(TargetModels { targets, complete })
            }
        });
        self.stats.elapsed += start.elapsed();
        match result {
            Some(models) => {
                self.stats.resolved_targets += models.targets.len();
                Some(models)
            }
            None => {
                self.stats.inconclusive += 1;
                None
            }
        }
    }

    fn reserve_query(&mut self) -> bool {
        if self.stats.queries >= self.config.max_queries {
            self.stats.budget_exhausted = true;
            return false
        }
        self.stats.queries += 1;
        true
    }
}

fn with_timeout<T: Send + Sync>(timeout_ms: u64, query: impl FnOnce() -> T + Send + Sync) -> T {
    let mut config = Config::new();
    config.set_timeout_msec(timeout_ms);
    with_z3_config(&config, query)
}

fn alternatives(value: &AbstractValue) -> (Vec<U256>, Vec<ExprId>) {
    match value {
        AbstractValue::Known(values) => (values.iter().copied().collect(), Vec::new()),
        AbstractValue::Symbolic { constants, expressions } => {
            (constants.iter().copied().collect(), expressions.iter().copied().collect())
        }
        AbstractValue::Unknown => (Vec::new(), Vec::new()),
    }
}

struct Translator<'a> {
    expressions: &'a ExpressionArena,
    translated: HashMap<ExprId, BV>,
}

impl<'a> Translator<'a> {
    fn new(expressions: &'a ExpressionArena) -> Self {
        Self { expressions, translated: HashMap::new() }
    }

    fn value(&mut self, value: &AbstractValue) -> Option<BV> {
        match value {
            AbstractValue::Known(values) if values.len() == 1 => {
                Some(bv(values.first().copied()?))
            }
            AbstractValue::Symbolic { constants, expressions }
                if constants.is_empty() && expressions.len() == 1 =>
            {
                self.expression(*expressions.first()?)
            }
            _ => None,
        }
    }

    fn expression(&mut self, expression: ExprId) -> Option<BV> {
        if let Some(translated) = self.translated.get(&expression) {
            return Some(translated.clone())
        }
        let node = self.expressions.get(expression)?;
        let translated = self.node(expression, node)?;
        self.translated.insert(expression, translated.clone());
        Some(translated)
    }

    fn node(&mut self, expression: ExprId, node: &ExpressionNode) -> Option<BV> {
        if node.state_version.is_some() || is_symbolic_leaf(node.opcode) {
            return Some(BV::new_const(format!("e{}", expression.index()), 256))
        }
        let inputs =
            node.inputs.iter().map(|input| self.value(input)).collect::<Option<Vec<_>>>()?;
        let zero = bv(U256::ZERO);
        let one = bv(U256::from(1));
        let bool_bv = |condition: Bool| condition.ite(&one, &zero);
        match (node.opcode, inputs.as_slice()) {
            (opcodes::ADD, [a, b]) => Some(a.bvadd(b)),
            (opcodes::MUL, [a, b]) => Some(a.bvmul(b)),
            (opcodes::SUB, [a, b]) => Some(a.bvsub(b)),
            (opcodes::DIV, [a, b]) => Some(b.eq(&zero).ite(&zero, &a.bvudiv(b))),
            (opcodes::SDIV, [a, b]) => Some(b.eq(&zero).ite(&zero, &a.bvsdiv(b))),
            (opcodes::MOD, [a, b]) => Some(b.eq(&zero).ite(&zero, &a.bvurem(b))),
            (opcodes::SMOD, [a, b]) => Some(b.eq(&zero).ite(&zero, &a.bvsrem(b))),
            (opcodes::ADDMOD, [a, b, modulus]) => {
                let wide_modulus = modulus.zero_ext(256);
                let result = a.zero_ext(256).bvadd(b.zero_ext(256)).bvurem(&wide_modulus);
                Some(modulus.eq(&zero).ite(&zero, &result.extract(255, 0)))
            }
            (opcodes::MULMOD, [a, b, modulus]) => {
                let wide_modulus = modulus.zero_ext(256);
                let result = a.zero_ext(256).bvmul(b.zero_ext(256)).bvurem(&wide_modulus);
                Some(modulus.eq(&zero).ite(&zero, &result.extract(255, 0)))
            }
            (opcodes::LT, [a, b]) => Some(bool_bv(a.bvult(b))),
            (opcodes::GT, [a, b]) => Some(bool_bv(a.bvugt(b))),
            (opcodes::SLT, [a, b]) => Some(bool_bv(a.bvslt(b))),
            (opcodes::SGT, [a, b]) => Some(bool_bv(a.bvsgt(b))),
            (opcodes::EQ, [a, b]) => Some(bool_bv(a.eq(b))),
            (opcodes::ISZERO, [value]) => Some(bool_bv(value.eq(&zero))),
            (opcodes::AND, [a, b]) => Some(a.bvand(b)),
            (opcodes::OR, [a, b]) => Some(a.bvor(b)),
            (opcodes::XOR, [a, b]) => Some(a.bvxor(b)),
            (opcodes::NOT, [value]) => Some(value.bvnot()),
            (opcodes::SHL, [shift, value]) => Some(value.bvshl(shift)),
            (opcodes::SHR, [shift, value]) => Some(value.bvlshr(shift)),
            (opcodes::SAR, [shift, value]) => Some(value.bvashr(shift)),
            _ => None,
        }
    }

    fn assert_facts(&mut self, solver: &Solver, facts: &PathFacts) {
        for (expression, constraint) in facts.constraints() {
            let Some(value) = self.expression(*expression) else { continue };
            solver.assert(value.bvuge(bv(constraint.min)));
            solver.assert(value.bvule(bv(constraint.max)));
            for excluded in &constraint.excluded {
                solver.assert(value.ne(bv(*excluded)));
            }
            if !constraint.known_zero_bits.is_zero() {
                solver.assert(value.bvand(bv(constraint.known_zero_bits)).eq(bv(U256::ZERO)));
            }
            if !constraint.known_one_bits.is_zero() {
                solver.assert(
                    value.bvand(bv(constraint.known_one_bits)).eq(bv(constraint.known_one_bits)),
                );
            }
        }
        for (left, right) in facts.equalities() {
            if let (Some(left), Some(right)) = (self.expression(*left), self.expression(*right)) {
                solver.assert(left.eq(right));
            }
        }
        for (left, right) in facts.disequalities() {
            if let (Some(left), Some(right)) = (self.expression(*left), self.expression(*right)) {
                solver.assert(left.ne(right));
            }
        }
    }
}

fn is_symbolic_leaf(opcode: u8) -> bool {
    matches!(
        opcode,
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

fn bv(value: U256) -> BV {
    BV::from_str(256, &value.to_string()).expect("U256 is a valid 256-bit numeral")
}

fn parse_bv(value: &BV) -> Option<U256> {
    let rendered = value.to_string();
    if let Some(hex) = rendered.strip_prefix("#x") {
        U256::from_str_radix(hex, 16).ok()
    } else if let Some(binary) = rendered.strip_prefix("#b") {
        U256::from_str_radix(binary, 2).ok()
    } else {
        U256::from_str_radix(&rendered, 10).ok()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::core::symbolic::ExpressionNode;

    fn expression(
        arena: &mut ExpressionArena,
        opcode: u8,
        inputs: Vec<AbstractValue>,
    ) -> AbstractValue {
        AbstractValue::expression(arena.intern(ExpressionNode {
            opcode,
            inputs,
            output: 0,
            state_version: None,
            effect_site: None,
        }))
    }

    #[test]
    fn proves_arithmetic_branch_infeasible() {
        let mut arena = ExpressionArena::new();
        let x = expression(
            &mut arena,
            opcodes::CALLDATALOAD,
            vec![AbstractValue::constant(U256::from(4))],
        );
        let added = expression(
            &mut arena,
            opcodes::ADD,
            vec![x.clone(), AbstractValue::constant(U256::from(1))],
        );
        let condition = expression(&mut arena, opcodes::EQ, vec![added, x]);
        let mut refiner = SmtRefiner::new(SmtConfig::default());

        assert_eq!(
            refiner.branch_feasible(&condition, true, &PathFacts::new(), &arena),
            Some(false)
        );
        assert_eq!(refiner.stats().infeasible_branches, 1);
    }

    #[test]
    fn enumerates_only_valid_symbolic_jump_targets() {
        let mut arena = ExpressionArena::new();
        let selector = expression(
            &mut arena,
            opcodes::CALLDATALOAD,
            vec![AbstractValue::constant(U256::ZERO)],
        );
        let masked = expression(
            &mut arena,
            opcodes::AND,
            vec![selector, AbstractValue::constant(U256::from(1))],
        );
        let target = expression(
            &mut arena,
            opcodes::ADD,
            vec![masked, AbstractValue::constant(U256::from(8))],
        );
        let program = Program::decode(
            &[
                opcodes::STOP,
                opcodes::STOP,
                opcodes::STOP,
                opcodes::STOP,
                opcodes::STOP,
                opcodes::STOP,
                opcodes::STOP,
                opcodes::STOP,
                opcodes::JUMPDEST,
                opcodes::JUMPDEST,
            ],
            crate::core::hardfork::HardFork::Latest,
        );
        let mut refiner = SmtRefiner::new(SmtConfig::default());
        let models = refiner
            .jump_targets(&target, &PathFacts::new(), &arena, &program)
            .expect("translatable target");

        assert_eq!(
            models.targets.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([U256::from(8), U256::from(9)])
        );
        assert!(models.complete);
    }

    #[test]
    fn enforces_per_analysis_query_budget() {
        let mut arena = ExpressionArena::new();
        let condition = expression(&mut arena, opcodes::CALLVALUE, vec![]);
        let mut refiner = SmtRefiner::new(SmtConfig { max_queries: 1, ..Default::default() });

        assert!(refiner.branch_feasible(&condition, true, &PathFacts::new(), &arena).is_some());
        assert_eq!(refiner.branch_feasible(&condition, false, &PathFacts::new(), &arena), None);
        assert!(refiner.stats().budget_exhausted);
        assert_eq!(refiner.stats().queries, 1);
    }

    #[test]
    fn path_facts_constrain_solver_models() {
        let mut arena = ExpressionArena::new();
        let value = expression(&mut arena, opcodes::CALLVALUE, vec![]);
        let expression_id = *value.expressions().expect("symbolic").first().expect("id");
        let mut facts = PathFacts::new();
        facts.assume(&value, false, &arena);
        let condition =
            expression(&mut arena, opcodes::EQ, vec![value, AbstractValue::constant(U256::ZERO)]);
        let mut refiner = SmtRefiner::new(SmtConfig::default());

        assert!(facts.constraint(expression_id).is_some());
        assert_eq!(refiner.branch_feasible(&condition, false, &facts, &arena), Some(false));
    }
}
