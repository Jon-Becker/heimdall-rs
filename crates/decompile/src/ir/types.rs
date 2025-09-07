use alloy::primitives::U256;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Const(U256),
    Var(String),
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    UnOp(UnOp, Box<Expr>),
    Call(CallType, Vec<Expr>),
    Load(LoadType, Box<Expr>),
    Cast(SolidityType, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Slt,
    Sle,
    Sgt,
    Sge,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
    IsZero,
    Neg,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallType {
    Call(Box<Expr>, Box<Expr>, Vec<Expr>),       // address, value, args
    DelegateCall(Box<Expr>, Vec<Expr>),          // address, args
    StaticCall(Box<Expr>, Vec<Expr>),            // address, args
    Create(Box<Expr>, Box<Expr>),                // value, code
    Create2(Box<Expr>, Box<Expr>, Box<Expr>),    // value, code, salt
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LoadType {
    Memory,
    Storage,
    Calldata,
    Transient,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SolidityType {
    Uint(usize),
    Int(usize),
    Address,
    Bool,
    Bytes(usize),
    BytesDynamic,
    String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Assign(String, Expr),
    Store(StoreType, Expr, Expr),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    Return(Vec<Expr>),
    Revert(Vec<Expr>),
    Jump(Label),
    Call(CallType),
    Log(usize, Vec<Expr>), // LOG0-LOG4
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreType {
    Memory,
    Storage,
    Transient,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Label(pub u128);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub label: Option<Label>,
    pub stmts: Vec<Stmt>,
    pub terminator: Option<Terminator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Return(Vec<Expr>),
    Revert(Vec<Expr>),
    Jump(Label),
    ConditionalJump(Expr, Label, Option<Label>),
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub selector: Option<U256>,
    pub params: Vec<Param>,
    pub returns: Vec<SolidityType>,
    pub blocks: Vec<Block>,
    pub modifiers: Vec<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: SolidityType,
    pub location: ParamLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamLocation {
    Memory,
    Calldata,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Public,
    External,
    Internal,
    Private,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Public
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Exp => write!(f, "**"),
            BinOp::And => write!(f, "&"),
            BinOp::Or => write!(f, "|"),
            BinOp::Xor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"),
            BinOp::Shr => write!(f, ">>"),
            BinOp::Sar => write!(f, ">>"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Ge => write!(f, ">="),
            BinOp::Slt => write!(f, "<"),
            BinOp::Sle => write!(f, "<="),
            BinOp::Sgt => write!(f, ">"),
            BinOp::Sge => write!(f, ">="),
        }
    }
}

impl BinOp {
    pub fn precedence(&self) -> u8 {
        match self {
            BinOp::Exp => 14,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 13,
            BinOp::Add | BinOp::Sub => 12,
            BinOp::Shl | BinOp::Shr | BinOp::Sar => 11,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Slt | BinOp::Sle | BinOp::Sgt | BinOp::Sge => 10,
            BinOp::Eq | BinOp::Ne => 9,
            BinOp::And => 8,
            BinOp::Xor => 7,
            BinOp::Or => 6,
        }
    }

    pub fn is_associative(&self) -> bool {
        matches!(self, BinOp::Add | BinOp::Mul | BinOp::And | BinOp::Or | BinOp::Xor)
    }

    pub fn is_commutative(&self) -> bool {
        matches!(
            self,
            BinOp::Add | BinOp::Mul | BinOp::And | BinOp::Or | BinOp::Xor | BinOp::Eq | BinOp::Ne
        )
    }
}