# Typed IR Pipeline Design

## Overview

Replace string-based decompilation with a robust typed pipeline:
`VMTrace → Tokenizer → Parser → IR → Optimization Passes → Solidity Emitter`

## Architecture

### 1. Tokenizer Module (`src/ir/tokenizer.rs`)
- Convert `VMTrace` operations into typed tokens
- Token types:
  - `Opcode(u8, String)` - opcode value and name
  - `Immediate(U256)` - push values
  - `StackRef(usize)` - stack position references
  - `MemoryRef(U256)` - memory location references  
  - `StorageRef(U256)` - storage slot references
  - `Label(u128)` - jump destinations
- Preserve instruction location for debugging

### 2. Parser Module (`src/ir/parser.rs`)
- Build typed IR from token stream
- Handle stack-based to expression-based conversion
- Track data flow through operations

### 3. IR Types (`src/ir/types.rs`)

```rust
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

pub enum Stmt {
    Assign(String, Expr),
    Store(StoreType, Expr, Expr),
    If(Expr, Block, Option<Block>),
    While(Expr, Block),
    Return(Vec<Expr>),
    Revert(Vec<Expr>),
    Jump(Label),
}

pub struct Block {
    pub label: Option<Label>,
    pub stmts: Vec<Stmt>,
    pub terminator: Option<Terminator>,
}

pub struct Function {
    pub selector: Option<U256>,
    pub params: Vec<Param>,
    pub returns: Vec<SolidityType>,
    pub blocks: Vec<Block>,
}
```

### 4. Optimization Passes (`src/ir/passes/`)

#### Phase 1: Simplification
- **Constant Folding** (`constant_fold.rs`): Evaluate compile-time expressions
- **Algebraic Simplification** (`algebraic.rs`): x+0→x, x*1→x, x-x→0
- **Bitwise Simplification** (`bitwise.rs`): x&0→0, x|0→x, mask propagation
- **Strength Reduction** (`strength.rs`): x*2→x<<1, x/2→x>>1

#### Phase 2: Cleanup  
- **Dead Code Elimination** (`dce.rs`): Remove unreachable/unused code
- **Common Subexpression Elimination** (`cse.rs`): Deduplicate expressions
- **Copy Propagation** (`copy_prop.rs`): Replace copies with originals

#### Phase 3: Structuring
- **Control Flow Recovery** (`control_flow.rs`): Detect if/else/while patterns
- **Type Inference** (`type_inference.rs`): Infer variable types from usage

### 5. Solidity Emitter (`src/ir/emit.rs`)
- Convert IR to readable Solidity
- Precedence-aware parenthesis insertion
- Pretty printing with proper indentation

## Pass Pipeline Order

1. Parse VMTrace to IR
2. Constant folding
3. Algebraic simplification  
4. Bitwise simplification
5. Strength reduction
6. Dead code elimination
7. Common subexpression elimination
8. Copy propagation
9. Control flow recovery
10. Type inference
11. Emit Solidity

## Key Design Decisions

1. **Immutable IR**: Each pass returns new IR, preserving semantics
2. **256-bit arithmetic**: All operations respect EVM's wrapping behavior
3. **Explicit precedence**: No ambiguity in expression evaluation order
4. **Source mapping**: Maintain trace→IR→Solidity location mappings
5. **Fail-safe**: Fall back to low-level patterns when high-level recovery uncertain

## Testing Strategy

- Unit tests per module
- Property-based tests for passes
- Golden tests for end-to-end decompilation
- Fuzzing for edge cases
- Benchmarks for performance regression