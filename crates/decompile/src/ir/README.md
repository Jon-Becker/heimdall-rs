# IR-Based Decompiler Pipeline

## Overview

This module implements a robust, typed intermediate representation (IR) pipeline for EVM bytecode decompilation, replacing the previous string-based approach.

## Architecture

```
VMTrace → Tokenizer → Parser → IR → Optimization Passes → Solidity Emitter
```

## Components

### Tokenizer (`tokenizer.rs`)
Converts raw VM trace operations into typed tokens:
- Opcodes with metadata
- Immediate values (PUSH operations)
- Stack operations (DUP/SWAP)
- Labels for jump destinations

### Parser (`parser.rs`)
Transforms token stream into typed IR:
- Converts stack-based operations to expression trees
- Builds control flow blocks
- Tracks data dependencies

### IR Types (`types.rs`)
Core data structures:
- `Expr`: Expression nodes (constants, variables, operations)
- `Stmt`: Statements (assignments, stores, control flow)
- `Block`: Basic blocks with optional labels
- `Function`: Top-level function representation

### Optimization Passes (`passes/`)

#### Phase 1: Simplification
- **Constant Folding**: Evaluates compile-time expressions
- **Algebraic Simplification**: Removes redundant operations (x+0→x)
- **Bitwise Simplification**: Optimizes bitwise operations, converts masks to casts
- **Strength Reduction**: Converts expensive ops to cheaper ones (x*2→x<<1)

#### Phase 2: Cleanup
- **Dead Code Elimination**: Removes unreachable code
- **Common Subexpression Elimination**: Deduplicates expressions
- **Copy Propagation**: Replaces copies with originals

#### Phase 3: Structuring
- **Control Flow Recovery**: Reconstructs if/else/while patterns
- **Type Inference**: Deduces variable types from usage

### Solidity Emitter (`emit.rs`)
Converts optimized IR to readable Solidity:
- Precedence-aware parenthesis insertion
- Proper indentation and formatting
- Type-aware code generation

## Usage

```rust
use heimdall_decompiler::ir::{decompile_trace};
use heimdall_vm::ext::exec::VMTrace;

let trace: VMTrace = /* ... */;
let solidity_code = decompile_trace(&trace)?;
```

## Key Features

1. **Type Safety**: No more string manipulation errors
2. **Semantic Preservation**: All optimizations maintain 256-bit wrapping semantics
3. **Testability**: Each component is independently testable
4. **Performance**: Linear complexity, minimal allocations
5. **Extensibility**: Easy to add new optimization passes

## Testing

Run tests:
```bash
cargo test -p heimdall-decompiler --lib ir::tests
```

Run benchmarks:
```bash
cargo bench -p heimdall-decompiler
```

## Future Improvements

- Complete implementation of DCE, CSE, and copy propagation
- Enhanced control flow recovery for complex patterns
- Better type inference with constraint solving
- Support for more EVM opcodes
- Integration with existing decompiler infrastructure