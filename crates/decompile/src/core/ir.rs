use std::collections::HashSet;

use alloy::primitives::U256;
use heimdall_common::utils::strings::encode_hex_reduced;
use heimdall_vm::core::opcodes::{self, WrappedInput, WrappedOpcode};

/// Unary source operators.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum UnaryOp {
    LogicalNot,
    BitwiseNot,
}

/// Binary source operators, ordered independently from their EVM opcode representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum BinaryOp {
    LogicalAnd,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinaryOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::LogicalAnd => "&&",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Exp => "**",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
        }
    }

    fn precedence(self) -> u8 {
        match self {
            Self::LogicalAnd => 1,
            Self::BitOr => 2,
            Self::BitXor => 3,
            Self::BitAnd => 4,
            Self::Lt | Self::Le | Self::Gt | Self::Ge | Self::Eq | Self::Ne => 5,
            Self::Shl | Self::Shr => 6,
            Self::Add | Self::Sub => 7,
            Self::Mul | Self::Div | Self::Mod => 8,
            Self::Exp => 9,
        }
    }
}

/// A semantic path from a root storage slot to a value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum StoragePath {
    Slot { slot: Box<Expr> },
    Mapping { parent: Box<StoragePath>, key: Box<Expr> },
    DynamicArray { parent: Box<StoragePath>, index: Box<Expr> },
    Field { parent: Box<StoragePath>, offset: U256 },
    PackedField { parent: Box<StoragePath>, bit_offset: u16, bit_width: u16 },
}

impl StoragePath {
    pub(crate) fn root(&self) -> &Expr {
        match self {
            Self::Slot { slot } => slot,
            Self::Mapping { parent, .. } |
            Self::DynamicArray { parent, .. } |
            Self::Field { parent, .. } |
            Self::PackedField { parent, .. } => parent.root(),
        }
    }

    fn collect_identifiers(&self, identifiers: &mut HashSet<String>) {
        match self {
            Self::Slot { slot } => slot.collect_identifiers(identifiers),
            Self::Mapping { parent, key } => {
                parent.collect_identifiers(identifiers);
                key.collect_identifiers(identifiers);
            }
            Self::DynamicArray { parent, index } => {
                parent.collect_identifiers(identifiers);
                index.collect_identifiers(identifiers);
            }
            Self::Field { parent, .. } | Self::PackedField { parent, .. } => {
                parent.collect_identifiers(identifiers)
            }
        }
    }

    fn visit_mut(&mut self, visitor: &mut impl FnMut(&mut Expr)) {
        match self {
            Self::Slot { slot } => slot.visit_mut(visitor),
            Self::Mapping { parent, key } => {
                parent.visit_mut(visitor);
                key.visit_mut(visitor);
            }
            Self::DynamicArray { parent, index } => {
                parent.visit_mut(visitor);
                index.visit_mut(visitor);
            }
            Self::Field { parent, .. } | Self::PackedField { parent, .. } => {
                parent.visit_mut(visitor)
            }
        }
    }

    fn simplify(self) -> Self {
        match self {
            Self::Slot { slot } => Self::Slot { slot: Box::new(slot.simplify()) },
            Self::Mapping { parent, key } => {
                Self::Mapping { parent: Box::new(parent.simplify()), key: Box::new(key.simplify()) }
            }
            Self::DynamicArray { parent, index } => Self::DynamicArray {
                parent: Box::new(parent.simplify()),
                index: Box::new(index.simplify()),
            },
            Self::Field { parent, offset } => {
                Self::Field { parent: Box::new(parent.simplify()), offset }
            }
            Self::PackedField { parent, bit_offset, bit_width } => {
                Self::PackedField { parent: Box::new(parent.simplify()), bit_offset, bit_width }
            }
        }
    }

    fn render_slot(&self) -> String {
        match self {
            Self::Slot { slot } => slot.render(),
            Self::Mapping { parent, key } => {
                format!("keccak256({}, {})", key.render(), parent.render_slot())
            }
            Self::DynamicArray { parent, index } => {
                format!("keccak256({}) + {}", parent.render_slot(), index.render())
            }
            Self::Field { parent, offset } => {
                format!("{} + {}", parent.render_slot(), encode_hex_reduced(*offset))
            }
            Self::PackedField { parent, bit_offset, bit_width } => {
                format!("packed({}, {}, {})", parent.render_slot(), bit_offset, bit_width)
            }
        }
    }
}

/// A small, lossless expression tree used between analysis and source rendering.
///
/// `Raw` is an intentional escape hatch for expressions that do not have a source-level mapping
/// yet. Common EVM expression trees are converted directly from [`WrappedOpcode`] so subsequent
/// passes can simplify them without parsing rendered Solidity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Expr {
    Raw(String),
    Empty,
    Identifier(String),
    Literal(U256),
    Bool(bool),
    StringLiteral(String),
    Unary { op: UnaryOp, value: Box<Expr> },
    Binary { op: BinaryOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Index { base: Box<Expr>, index: Box<Expr> },
    Slice { base: Box<Expr>, start: Box<Expr>, end: Box<Expr> },
    Member { base: Box<Expr>, member: String },
    Keccak { offset: Box<Expr>, size: Box<Expr>, preimage: Option<Vec<Expr>> },
    StorageAccess(Box<StoragePath>),
    Cast { ty: String, value: Box<Expr> },
    Call { callee: String, args: Vec<Expr> },
}

impl Expr {
    pub(crate) fn raw(value: impl Into<String>) -> Self {
        Self::Raw(value.into())
    }

    pub(crate) fn identifier(value: impl Into<String>) -> Self {
        Self::Identifier(value.into())
    }

    pub(crate) fn binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Self {
        Self::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }.simplify()
    }

    pub(crate) fn index(base: impl Into<String>, index: Expr) -> Self {
        Self::Index { base: Box::new(Self::raw(base)), index: Box::new(index) }
    }

    pub(crate) fn slice(base: impl Into<String>, start: Expr, end: Expr) -> Self {
        Self::Slice { base: Box::new(Self::raw(base)), start: Box::new(start), end: Box::new(end) }
    }

    /// Convert an opcode dependency tree to Yul's functional expression form.
    pub(crate) fn from_yul_opcode(opcode: &WrappedOpcode) -> Self {
        if opcode.opcode == opcodes::PUSH0 {
            return Self::Literal(U256::ZERO);
        }
        if (opcodes::PUSH1..=opcodes::PUSH32).contains(&opcode.opcode) {
            return opcode
                .inputs
                .first()
                .map(Self::from_yul_input)
                .unwrap_or(Self::Literal(U256::ZERO));
        }

        Self::Call {
            callee: opcodes::opcode_name(opcode.opcode).to_lowercase(),
            args: opcode.inputs.iter().map(Self::from_yul_input).collect(),
        }
    }

    fn from_yul_input(input: &WrappedInput) -> Self {
        match input {
            WrappedInput::Raw(value) => Self::Literal(*value),
            WrappedInput::Opcode(opcode) => Self::from_yul_opcode(opcode),
        }
    }

    pub(crate) fn from_opcode(opcode: &WrappedOpcode) -> Self {
        let input = |index: usize| {
            opcode.inputs.get(index).map(Self::from_input).unwrap_or_else(|| Self::raw("unknown"))
        };
        let binary = |op, lhs, rhs| Self::binary(op, input(lhs), input(rhs));

        match opcode.opcode {
            opcodes::PUSH0 => Self::Literal(U256::ZERO),
            opcodes::PUSH1..=opcodes::PUSH32 => input(0),
            opcodes::ADD => binary(BinaryOp::Add, 0, 1),
            opcodes::SUB => binary(BinaryOp::Sub, 0, 1),
            opcodes::MUL => binary(BinaryOp::Mul, 0, 1),
            opcodes::DIV | opcodes::SDIV => binary(BinaryOp::Div, 0, 1),
            opcodes::MOD | opcodes::SMOD => binary(BinaryOp::Mod, 0, 1),
            opcodes::ADDMOD => Self::binary(
                BinaryOp::Mod,
                Self::binary(BinaryOp::Add, input(0), input(1)),
                input(2),
            ),
            opcodes::MULMOD => Self::binary(
                BinaryOp::Mod,
                Self::binary(BinaryOp::Mul, input(0), input(1)),
                input(2),
            ),
            opcodes::EXP => binary(BinaryOp::Exp, 0, 1),
            opcodes::LT | opcodes::SLT => binary(BinaryOp::Lt, 0, 1),
            opcodes::GT | opcodes::SGT => binary(BinaryOp::Gt, 0, 1),
            opcodes::EQ => binary(BinaryOp::Eq, 0, 1),
            opcodes::AND => binary(BinaryOp::BitAnd, 0, 1),
            opcodes::OR => binary(BinaryOp::BitOr, 0, 1),
            opcodes::XOR => binary(BinaryOp::BitXor, 0, 1),
            opcodes::SHL => binary(BinaryOp::Shl, 1, 0),
            opcodes::SHR | opcodes::SAR => binary(BinaryOp::Shr, 1, 0),
            opcodes::ISZERO => {
                Self::Unary { op: UnaryOp::LogicalNot, value: Box::new(input(0)) }.simplify()
            }
            opcodes::NOT => Self::Unary { op: UnaryOp::BitwiseNot, value: Box::new(input(0)) },
            opcodes::BYTE => input(1),
            opcodes::CLZ => Self::Call { callee: "clz".to_string(), args: vec![input(0)] },
            opcodes::CALLDATALOAD => Self::calldata_load(input(0)),
            opcodes::CALLDATASIZE => Self::identifier("msg.data.length"),
            opcodes::CODESIZE => Self::identifier("this.code.length"),
            opcodes::ADDRESS => Self::identifier("address(this)"),
            opcodes::ORIGIN => Self::identifier("tx.origin"),
            opcodes::CALLER => Self::identifier("msg.sender"),
            opcodes::CALLVALUE => Self::identifier("msg.value"),
            opcodes::COINBASE => Self::identifier("block.coinbase"),
            opcodes::TIMESTAMP => Self::identifier("block.timestamp"),
            opcodes::NUMBER => Self::identifier("block.number"),
            opcodes::PREVRANDAO => Self::identifier("block.prevrandao"),
            opcodes::GASLIMIT => Self::identifier("block.gaslimit"),
            opcodes::CHAINID => Self::identifier("block.chainid"),
            opcodes::BASEFEE => Self::identifier("block.basefee"),
            opcodes::SELFBALANCE => Self::identifier("address(this).balance"),
            opcodes::GASPRICE => Self::identifier("tx.gasprice"),
            opcodes::GAS => Self::Call { callee: "gasleft".to_string(), args: vec![] },
            opcodes::SLOAD => {
                Self::StorageAccess(Box::new(StoragePath::Slot { slot: Box::new(input(0)) }))
            }
            opcodes::TLOAD => Self::index("transient", input(0)),
            opcodes::MLOAD => Self::index("memory", input(0)),
            opcodes::MSIZE => Self::identifier("memory.length"),
            opcodes::RETURNDATASIZE => Self::identifier("ret0.length"),
            opcodes::SHA3 => Self::Keccak {
                offset: Box::new(input(0)),
                size: Box::new(input(1)),
                preimage: None,
            },
            opcodes::BLOCKHASH => {
                Self::Call { callee: "blockhash".to_string(), args: vec![input(0)] }
            }
            opcodes::BALANCE => Self::Member {
                base: Box::new(Self::Call { callee: "address".to_string(), args: vec![input(0)] }),
                member: "balance".to_string(),
            },
            opcodes::EXTCODESIZE => Self::Member {
                base: Box::new(Self::Call { callee: "address".to_string(), args: vec![input(0)] }),
                member: "code.length".to_string(),
            },
            opcodes::EXTCODEHASH => Self::Member {
                base: Box::new(Self::Call { callee: "address".to_string(), args: vec![input(0)] }),
                member: "codehash".to_string(),
            },
            _ => Self::raw(opcode.solidify()),
        }
    }

    fn from_input(input: &WrappedInput) -> Self {
        match input {
            WrappedInput::Raw(value) => Self::Literal(*value),
            WrappedInput::Opcode(opcode) => Self::from_opcode(opcode),
        }
    }

    fn mask_cast(value: Expr, mask: U256) -> Option<Self> {
        if mask == U256::MAX {
            return None;
        }

        let bytes = mask.to_be_bytes::<32>();
        let first_nonzero = bytes.iter().position(|byte| *byte != 0)?;
        if !bytes[first_nonzero..].iter().all(|byte| *byte == 0xff) {
            return None;
        }

        let width = 32 - first_nonzero;
        let ty = if width == 20 { "address".to_string() } else { format!("uint{}", width * 8) };
        Some(Self::Cast { ty, value: Box::new(value) })
    }

    fn calldata_load(offset: Expr) -> Self {
        match offset {
            Self::Literal(value) => match usize::try_from(value) {
                Ok(offset) if offset < 4 => Self::index("msg.data", Self::Literal(value)),
                Ok(offset) => Self::identifier(format!("arg{}", (offset - 4) / 32)),
                Err(_) => Self::index("msg.data", Self::Literal(value)),
            },
            offset => Self::index("msg.data", offset),
        }
    }

    /// Collect identifiers referenced by this expression.
    pub(crate) fn collect_identifiers(&self, identifiers: &mut HashSet<String>) {
        match self {
            Self::Identifier(name) => {
                identifiers.insert(name.clone());
            }
            Self::Unary { value, .. } | Self::Cast { value, .. } => {
                value.collect_identifiers(identifiers);
            }
            Self::Binary { lhs, rhs, .. } => {
                lhs.collect_identifiers(identifiers);
                rhs.collect_identifiers(identifiers);
            }
            Self::Index { base, index } => {
                base.collect_identifiers(identifiers);
                index.collect_identifiers(identifiers);
            }
            Self::Slice { base, start, end } => {
                base.collect_identifiers(identifiers);
                start.collect_identifiers(identifiers);
                end.collect_identifiers(identifiers);
            }
            Self::Member { base, .. } => base.collect_identifiers(identifiers),
            Self::Keccak { offset, size, preimage } => {
                offset.collect_identifiers(identifiers);
                size.collect_identifiers(identifiers);
                if let Some(preimage) = preimage {
                    for value in preimage {
                        value.collect_identifiers(identifiers);
                    }
                }
            }
            Self::StorageAccess(path) => path.collect_identifiers(identifiers),
            Self::Call { args, .. } => {
                for arg in args {
                    arg.collect_identifiers(identifiers);
                }
            }
            _ => {}
        }
    }

    /// Visit this expression and all descendants depth-first.
    pub(crate) fn visit_mut(&mut self, visitor: &mut impl FnMut(&mut Expr)) {
        match self {
            Self::Unary { value, .. } | Self::Cast { value, .. } => value.visit_mut(visitor),
            Self::Binary { lhs, rhs, .. } => {
                lhs.visit_mut(visitor);
                rhs.visit_mut(visitor);
            }
            Self::Index { base, index } => {
                base.visit_mut(visitor);
                index.visit_mut(visitor);
            }
            Self::Slice { base, start, end } => {
                base.visit_mut(visitor);
                start.visit_mut(visitor);
                end.visit_mut(visitor);
            }
            Self::Member { base, .. } => base.visit_mut(visitor),
            Self::Keccak { offset, size, preimage } => {
                offset.visit_mut(visitor);
                size.visit_mut(visitor);
                if let Some(preimage) = preimage {
                    for value in preimage {
                        value.visit_mut(visitor);
                    }
                }
            }
            Self::StorageAccess(path) => path.visit_mut(visitor),
            Self::Call { args, .. } => {
                for arg in args {
                    arg.visit_mut(visitor);
                }
            }
            _ => {}
        }
        visitor(self);
    }

    /// Apply local, semantics-preserving simplifications before source rendering.
    pub(crate) fn simplify(self) -> Self {
        match self {
            Self::Unary { op, value } => {
                let value = value.simplify();
                match (op, value) {
                    (UnaryOp::LogicalNot, Self::Unary { op: UnaryOp::LogicalNot, value }) => *value,
                    (UnaryOp::LogicalNot, Self::Bool(value)) => Self::Bool(!value),
                    (UnaryOp::LogicalNot, Self::Binary { op, lhs, rhs })
                        if matches!(
                            op,
                            BinaryOp::Lt |
                                BinaryOp::Le |
                                BinaryOp::Gt |
                                BinaryOp::Ge |
                                BinaryOp::Eq |
                                BinaryOp::Ne
                        ) =>
                    {
                        Self::Binary {
                            op: match op {
                                BinaryOp::Lt => BinaryOp::Ge,
                                BinaryOp::Le => BinaryOp::Gt,
                                BinaryOp::Gt => BinaryOp::Le,
                                BinaryOp::Ge => BinaryOp::Lt,
                                BinaryOp::Eq => BinaryOp::Ne,
                                BinaryOp::Ne => BinaryOp::Eq,
                                _ => unreachable!("matched a comparison operator"),
                            },
                            lhs,
                            rhs,
                        }
                    }
                    (op, value) => Self::Unary { op, value: Box::new(value) },
                }
            }
            Self::Binary { op, lhs, rhs } => {
                let lhs = lhs.simplify();
                let rhs = rhs.simplify();
                match (op, &lhs, &rhs) {
                    (
                        BinaryOp::Add | BinaryOp::BitOr | BinaryOp::BitXor,
                        _,
                        Self::Literal(value),
                    ) if value.is_zero() => lhs,
                    (
                        BinaryOp::Add | BinaryOp::BitOr | BinaryOp::BitXor,
                        Self::Literal(value),
                        _,
                    ) if value.is_zero() => rhs,
                    (BinaryOp::Sub | BinaryOp::Shl | BinaryOp::Shr, _, Self::Literal(value))
                        if value.is_zero() =>
                    {
                        lhs
                    }
                    (BinaryOp::Mul | BinaryOp::Div, _, Self::Literal(value))
                        if *value == U256::from(1) =>
                    {
                        lhs
                    }
                    (BinaryOp::Mul, Self::Literal(value), _) if *value == U256::from(1) => rhs,
                    (BinaryOp::BitAnd, _, Self::Literal(value)) if *value == U256::MAX => lhs,
                    (BinaryOp::BitAnd, Self::Literal(value), _) if *value == U256::MAX => rhs,
                    (BinaryOp::BitAnd, _, Self::Literal(mask)) => {
                        Self::mask_cast(lhs.clone(), *mask).unwrap_or_else(|| Self::Binary {
                            op,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        })
                    }
                    (BinaryOp::BitAnd, Self::Literal(mask), _) => {
                        Self::mask_cast(rhs.clone(), *mask).unwrap_or_else(|| Self::Binary {
                            op,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        })
                    }
                    (BinaryOp::Eq, ..) if lhs == rhs => Self::Bool(true),
                    _ => Self::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                }
            }
            Self::Index { base, index } => {
                Self::Index { base: Box::new(base.simplify()), index: Box::new(index.simplify()) }
            }
            Self::Slice { base, start, end } => Self::Slice {
                base: Box::new(base.simplify()),
                start: Box::new(start.simplify()),
                end: Box::new(end.simplify()),
            },
            Self::Member { base, member } => {
                Self::Member { base: Box::new(base.simplify()), member }
            }
            Self::Keccak { offset, size, preimage } => Self::Keccak {
                offset: Box::new(offset.simplify()),
                size: Box::new(size.simplify()),
                preimage: preimage.map(|values| values.into_iter().map(Self::simplify).collect()),
            },
            Self::StorageAccess(path) => Self::StorageAccess(Box::new(path.simplify())),
            Self::Cast { ty, value } => Self::Cast { ty, value: Box::new(value.simplify()) },
            Self::Call { callee, args } => {
                Self::Call { callee, args: args.into_iter().map(Self::simplify).collect() }
            }
            value => value,
        }
    }

    pub(crate) fn render(&self) -> String {
        self.render_with_precedence(0, false)
    }

    fn precedence(&self) -> u8 {
        match self {
            Self::Binary { op, .. } => op.precedence(),
            Self::Unary { .. } => 10,
            _ => 11,
        }
    }

    fn render_with_precedence(&self, parent_precedence: u8, right_child: bool) -> String {
        let rendered = match self {
            Self::Raw(value) | Self::Identifier(value) => value.clone(),
            Self::Empty => String::new(),
            Self::Literal(value) => encode_hex_reduced(*value),
            Self::Bool(value) => value.to_string(),
            Self::StringLiteral(value) => format!("{value:?}"),
            Self::Unary { op, value } => {
                let operator = match op {
                    UnaryOp::LogicalNot => "!",
                    UnaryOp::BitwiseNot => "~",
                };
                format!("{operator}{}", value.render_with_precedence(10, false))
            }
            Self::Binary { op, lhs, rhs } => format!(
                "{} {} {}",
                lhs.render_with_precedence(op.precedence(), false),
                op.symbol(),
                rhs.render_with_precedence(op.precedence(), true)
            ),
            Self::Index { base, index } => format!("{}[{}]", base.render(), index.render()),
            Self::Slice { base, start, end } => {
                format!("{}[{}:{}]", base.render(), start.render(), end.render())
            }
            Self::Member { base, member } => format!("{}.{}", base.render(), member),
            Self::Keccak { offset, size, preimage } => match preimage {
                Some(values) => format!(
                    "keccak256({})",
                    values.iter().map(Self::render).collect::<Vec<_>>().join(", ")
                ),
                None => format!(
                    "keccak256(memory[{}:{}])",
                    offset.render(),
                    Self::binary(BinaryOp::Add, *offset.clone(), *size.clone()).render()
                ),
            },
            Self::StorageAccess(path) => format!("storage[{}]", path.render_slot()),
            Self::Cast { ty, value } => format!("{ty}({})", value.render()),
            Self::Call { callee, args } => format!(
                "{callee}({})",
                args.iter().map(Self::render).collect::<Vec<_>>().join(", ")
            ),
        };

        let needs_parentheses = self.precedence() < parent_precedence ||
            (right_child &&
                self.precedence() == parent_precedence &&
                matches!(self, Self::Binary { .. }));
        if needs_parentheses {
            format!("({rendered})")
        } else {
            rendered
        }
    }
}

impl From<&WrappedOpcode> for Expr {
    fn from(opcode: &WrappedOpcode) -> Self {
        Self::from_opcode(opcode)
    }
}

/// Source syntax selected when lowering the shared IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenderTarget {
    Solidity,
    Yul,
}

/// A structured source-level statement. Rendering is deliberately delayed until all analysis
/// heuristics have run, so control-flow and assignments can be transformed without parsing text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Statement {
    Assign {
        target: Expr,
        value: Expr,
    },
    DeclareAssign {
        ty: String,
        target: Expr,
        value: Expr,
    },
    If {
        condition: Expr,
    },
    /// Marker separating flattened branch children before control-flow structuring.
    Else,
    IfElse {
        condition: Expr,
        then_body: Vec<Statement>,
        else_body: Vec<Statement>,
    },
    IfRevertElse {
        condition: Expr,
        offset: Expr,
        size: Expr,
    },
    Require {
        condition: Expr,
        reason: Option<Expr>,
    },
    Return(Expr),
    Revert(Option<Expr>),
    Emit {
        event: String,
        args: Vec<Expr>,
        comment: Option<String>,
    },
    ExternalCall {
        address: Expr,
        function: String,
        args: Vec<Expr>,
        gas: Option<Expr>,
        value: Option<Expr>,
        comment: Option<String>,
    },
    Expression(Expr),
    /// Captures a SHA3 operation while its memory preimage is still available.
    KeccakSnapshot(Expr),
    AssemblyAssign {
        target: String,
        function: String,
        args: Vec<Expr>,
    },
    Noop,
    CloseBlock,
}

impl Statement {
    pub(crate) fn visit_exprs_mut(&mut self, visitor: &mut impl FnMut(&mut Expr)) {
        match self {
            Self::Assign { target, value } | Self::DeclareAssign { target, value, .. } => {
                target.visit_mut(visitor);
                value.visit_mut(visitor);
            }
            Self::If { condition } => condition.visit_mut(visitor),
            Self::IfElse { condition, then_body, else_body } => {
                condition.visit_mut(visitor);
                for statement in then_body.iter_mut().chain(else_body.iter_mut()) {
                    statement.visit_exprs_mut(visitor);
                }
            }
            Self::IfRevertElse { condition, offset, size } => {
                condition.visit_mut(visitor);
                offset.visit_mut(visitor);
                size.visit_mut(visitor);
            }
            Self::Require { condition, reason } => {
                condition.visit_mut(visitor);
                if let Some(reason) = reason {
                    reason.visit_mut(visitor);
                }
            }
            Self::Return(value) | Self::Expression(value) | Self::KeccakSnapshot(value) => {
                value.visit_mut(visitor)
            }
            Self::Revert(reason) => {
                if let Some(reason) = reason {
                    reason.visit_mut(visitor);
                }
            }
            Self::Emit { args, .. } | Self::AssemblyAssign { args, .. } => {
                for arg in args {
                    arg.visit_mut(visitor);
                }
            }
            Self::ExternalCall { address, args, gas, value, .. } => {
                address.visit_mut(visitor);
                for arg in args {
                    arg.visit_mut(visitor);
                }
                if let Some(gas) = gas {
                    gas.visit_mut(visitor);
                }
                if let Some(value) = value {
                    value.visit_mut(visitor);
                }
            }
            Self::Else | Self::Noop | Self::CloseBlock => {}
        }
    }

    pub(crate) fn simplify(self) -> Self {
        match self {
            Self::Assign { target, value } => {
                Self::Assign { target: target.simplify(), value: value.simplify() }
            }
            Self::DeclareAssign { ty, target, value } => {
                Self::DeclareAssign { ty, target: target.simplify(), value: value.simplify() }
            }
            Self::If { condition } => Self::If { condition: condition.simplify() },
            Self::IfElse { condition, then_body, else_body } => Self::IfElse {
                condition: condition.simplify(),
                then_body: then_body.into_iter().map(Self::simplify).collect(),
                else_body: else_body.into_iter().map(Self::simplify).collect(),
            },
            Self::IfRevertElse { condition, offset, size } => Self::IfRevertElse {
                condition: condition.simplify(),
                offset: offset.simplify(),
                size: size.simplify(),
            },
            Self::Require { condition, reason } => Self::Require {
                condition: condition.simplify(),
                reason: reason.map(Expr::simplify),
            },
            Self::Return(value) => Self::Return(value.simplify()),
            Self::Revert(reason) => Self::Revert(reason.map(Expr::simplify)),
            Self::Emit { event, args, comment } => {
                Self::Emit { event, args: args.into_iter().map(Expr::simplify).collect(), comment }
            }
            Self::ExternalCall { address, function, args, gas, value, comment } => {
                Self::ExternalCall {
                    address: address.simplify(),
                    function,
                    args: args.into_iter().map(Expr::simplify).collect(),
                    gas: gas.map(Expr::simplify),
                    value: value.map(Expr::simplify),
                    comment,
                }
            }
            Self::Expression(value) => Self::Expression(value.simplify()),
            Self::KeccakSnapshot(value) => Self::KeccakSnapshot(value.simplify()),
            Self::AssemblyAssign { target, function, args } => Self::AssemblyAssign {
                target,
                function,
                args: args.into_iter().map(Expr::simplify).collect(),
            },
            statement => statement,
        }
    }

    pub(crate) fn render_lines(&self, target: RenderTarget) -> Vec<String> {
        if let Self::IfElse { condition, then_body, else_body } = self {
            let condition = match target {
                RenderTarget::Solidity => format!("if ({}) {{", condition.render()),
                RenderTarget::Yul => format!("if {} {{", condition.render()),
            };
            let mut lines = vec![condition];
            lines.extend(then_body.iter().flat_map(|statement| statement.render_lines(target)));
            if !else_body.is_empty() {
                lines.push("} else {".to_string());
                lines.extend(else_body.iter().flat_map(|statement| statement.render_lines(target)));
            }
            lines.push("}".to_string());
            lines
        } else {
            vec![self.render(target)]
        }
    }

    pub(crate) fn render(&self, target: RenderTarget) -> String {
        match (target, self) {
            (RenderTarget::Yul, Self::If { condition }) => {
                format!("if {} {{", condition.render())
            }
            (RenderTarget::Yul, Self::IfRevertElse { condition, offset, size }) => format!(
                "if {} {{ revert({}, {}); }} else {{",
                condition.render(),
                offset.render(),
                size.render()
            ),
            (RenderTarget::Yul, Self::Expression(expr)) => expr.render(),
            (_, statement) => statement.render_solidity(),
        }
    }

    fn render_solidity(&self) -> String {
        match self {
            Self::Assign { target, value } => {
                if let Expr::Binary { op, lhs, rhs } = value {
                    let compound = match op {
                        BinaryOp::Add => Some("+="),
                        BinaryOp::Sub => Some("-="),
                        BinaryOp::Mul => Some("*="),
                        BinaryOp::Div => Some("/="),
                        BinaryOp::Mod => Some("%="),
                        _ => None,
                    };
                    if let Some(compound) = compound {
                        if &**lhs == target {
                            return format!("{} {compound} {};", target.render(), rhs.render());
                        }
                        if matches!(op, BinaryOp::Add | BinaryOp::Mul) && &**rhs == target {
                            return format!("{} {compound} {};", target.render(), lhs.render());
                        }
                    }
                }
                format!("{} = {};", target.render(), value.render())
            }
            Self::DeclareAssign { ty, target, value } => {
                format!("{ty} {} = {};", target.render(), value.render())
            }
            Self::If { condition } => format!("if ({}) {{", condition.render()),
            Self::Else => "} else {".to_string(),
            Self::IfElse { .. } => self.render_lines(RenderTarget::Solidity).join("\n"),
            Self::IfRevertElse { condition, offset, size } => format!(
                "if ({}) {{ revert({}, {}); }} else {{",
                condition.render(),
                offset.render(),
                size.render()
            ),
            Self::Require { condition, reason } => match reason {
                Some(reason) => format!("require({}, {});", condition.render(), reason.render()),
                None => format!("require({});", condition.render()),
            },
            Self::Return(value) => format!("return {};", value.render()),
            Self::Revert(reason) => match reason {
                Some(reason @ Expr::Call { .. }) => format!("revert {};", reason.render()),
                Some(reason) => format!("revert({});", reason.render()),
                None => "revert();".to_string(),
            },
            Self::Emit { event, args, comment } => {
                let mut output = format!(
                    "emit {event}({});",
                    args.iter().map(Expr::render).collect::<Vec<_>>().join(", ")
                );
                if let Some(comment) = comment {
                    output.push_str(" // ");
                    output.push_str(comment);
                }
                output
            }
            Self::ExternalCall { address, function, args, gas, value, comment } => {
                if function == "transfer" {
                    let amount = args.first().map(Expr::render).unwrap_or_else(|| "0".to_string());
                    return format!("payable(address({})).transfer({amount});", address.render());
                }

                let mut options = Vec::new();
                if let Some(gas) = gas {
                    options.push(format!("gas: {}", gas.render()));
                }
                if let Some(value) = value {
                    options.push(format!("value: {}", value.render()));
                }
                let options = if options.is_empty() {
                    String::new()
                } else {
                    format!("{{ {} }}", options.join(", "))
                };
                let comment =
                    comment.as_ref().map(|comment| format!(" // {comment}")).unwrap_or_default();
                format!(
                    "(bool success, bytes memory ret0) = address({}).{function}{options}({});{comment}",
                    address.render(),
                    args.iter().map(Expr::render).collect::<Vec<_>>().join(", ")
                )
            }
            Self::Expression(expr) => format!("{};", expr.render()),
            Self::AssemblyAssign { target, function, args } => format!(
                "assembly {{ {target} := {function}({}) }}",
                args.iter().map(Expr::render).collect::<Vec<_>>().join(", ")
            ),
            Self::KeccakSnapshot(_) | Self::Noop => String::new(),
            Self::CloseBlock => "}".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::primitives::U256;
    use heimdall_vm::core::opcodes::{self, WrappedInput, WrappedOpcode};

    use super::{BinaryOp, Expr, RenderTarget, Statement};

    #[test]
    fn renders_structured_assignment() {
        let statement = Statement::Assign {
            target: Expr::index("storage", Expr::Literal(U256::from(2))),
            value: Expr::identifier("arg0"),
        };
        assert_eq!(statement.render(RenderTarget::Solidity), "storage[0x02] = arg0;");
    }

    #[test]
    fn renders_compound_assignment() {
        let target = Expr::index("balances", Expr::identifier("msg.sender"));
        let statement = Statement::Assign {
            target: target.clone(),
            value: Expr::Binary {
                op: BinaryOp::Sub,
                lhs: Box::new(target),
                rhs: Box::new(Expr::identifier("amount")),
            },
        };
        assert_eq!(statement.render(RenderTarget::Solidity), "balances[msg.sender] -= amount;");
    }

    #[test]
    fn renders_yul_condition_without_solidity_parentheses() {
        let condition =
            WrappedOpcode::new(opcodes::EQ, vec![U256::from(7).into(), U256::from(1).into()]);
        let statement = Statement::If { condition: Expr::from_yul_opcode(&condition) };
        assert_eq!(statement.render(RenderTarget::Yul), "if eq(0x07, 0x01) {");
    }

    #[test]
    fn renders_escaped_require_reason() {
        let statement = Statement::Require {
            condition: Expr::binary(
                BinaryOp::Eq,
                Expr::identifier("msg.sender"),
                Expr::identifier("owner"),
            ),
            reason: Some(Expr::StringLiteral("not \"owner\"".to_string())),
        };
        assert_eq!(
            statement.render(RenderTarget::Solidity),
            "require(msg.sender == owner, \"not \\\"owner\\\"\");"
        );
    }

    #[test]
    fn preserves_operator_precedence() {
        let expression = Expr::binary(
            BinaryOp::Mul,
            Expr::binary(BinaryOp::Add, Expr::identifier("a"), Expr::identifier("b")),
            Expr::identifier("c"),
        );
        assert_eq!(expression.render(), "(a + b) * c");
    }

    #[test]
    fn converts_and_simplifies_wrapped_opcode() {
        let add = WrappedOpcode::new(
            opcodes::ADD,
            vec![
                WrappedInput::Opcode(Arc::new(WrappedOpcode::new(
                    opcodes::CALLDATALOAD,
                    vec![U256::from(4).into()],
                ))),
                U256::ZERO.into(),
            ],
        );
        assert_eq!(Expr::from_opcode(&add).render(), "arg0");
    }

    #[test]
    fn renders_structured_external_call() {
        let statement = Statement::ExternalCall {
            address: Expr::identifier("target"),
            function: "foo".to_string(),
            args: vec![Expr::identifier("arg0")],
            gas: Some(Expr::Call { callee: "gasleft".to_string(), args: vec![] }),
            value: Some(Expr::Literal(U256::from(1))),
            comment: Some("call".to_string()),
        };
        assert_eq!(
            statement.render(RenderTarget::Solidity),
            "(bool success, bytes memory ret0) = address(target).foo{ gas: gasleft(), value: 0x01 }(arg0); // call"
        );
    }

    #[test]
    fn renders_addmod_with_correct_grouping() {
        let addmod = WrappedOpcode::new(
            opcodes::ADDMOD,
            vec![U256::from(1).into(), U256::from(2).into(), U256::from(3).into()],
        );
        assert_eq!(Expr::from_opcode(&addmod).render(), "(0x01 + 0x02) % 0x03");
    }

    #[test]
    fn renders_value_transfer() {
        let statement = Statement::ExternalCall {
            address: Expr::identifier("recipient"),
            function: "transfer".to_string(),
            args: vec![Expr::identifier("amount")],
            gas: None,
            value: None,
            comment: None,
        };
        assert_eq!(
            statement.render(RenderTarget::Solidity),
            "payable(address(recipient)).transfer(amount);"
        );
    }

    #[test]
    fn simplifies_low_bit_mask_to_cast() {
        let masked = Expr::binary(
            BinaryOp::BitAnd,
            Expr::identifier("arg0"),
            Expr::Literal(U256::from(0xffff_u64)),
        );
        assert_eq!(masked.render(), "uint16(arg0)");
    }

    #[test]
    fn simplifies_negated_comparison() {
        let equality = WrappedOpcode::new(
            opcodes::EQ,
            vec![
                WrappedInput::Opcode(Arc::new(WrappedOpcode::new(
                    opcodes::CALLDATALOAD,
                    vec![U256::from(4).into()],
                ))),
                U256::from(7).into(),
            ],
        );
        let is_zero =
            WrappedOpcode::new(opcodes::ISZERO, vec![WrappedInput::Opcode(Arc::new(equality))]);
        assert_eq!(Expr::from_opcode(&is_zero).render(), "arg0 != 0x07");
    }
}
