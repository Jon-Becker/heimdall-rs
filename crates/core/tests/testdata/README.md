# Shared test fixtures

Integration tests, unit tests, and Criterion benchmarks across the workspace keep
substantial static inputs and expected output here. Use `include_str!` (or
`include_bytes!` for binary data) with a path relative to the consuming Rust file.
This keeps fixtures independent of the working directory and avoids adding file
I/O to benchmarks. Reuse an existing fixture when its contents match.

Small, self-contained unit-test values, assertions, and programmatically built
inputs stay beside the test logic.

| Directory | Contents |
| --- | --- |
| `abi/` | Shared Solidity function signatures |
| `bytecode/` | Contract bytecode shared by CFG, decompile, and disassemble benchmarks |
| `cfg/` | CFG inputs and expected DOT fragments |
| `common/` | Bytecode retrieval inputs |
| `decode/` | Calldata for integration tests and benchmarks; ABI decoder cases in `abi/` |
| `decompile/` | Compiler, ABI, and deterministic-output regression bytecode |
| `disassemble/` | Bytecode inputs and exact assembly output |
| `vm/` | VM regression and benchmark inputs; memory snapshots in `memory/` |

`txids.json` is the existing transaction dataset for the ignored heavy integration
tests. The optional `largest1k` dataset remains an external download, as documented
by those tests.

These fixtures were extracted from the existing tests without changing their
contents. Preserve prefixes, letter case, whitespace, and trailing newlines (or
their absence). In particular, `.asm` snapshots preserve spaces before line
breaks, and `.dot` fragments retain Graphviz escapes. Do not trim or normalize
expected output when loading it.

The VM benchmark inputs `fib.hex`, `weth9.hex`, and `ten_thousand_hashes.hex` were
moved here from `crates/vm/benches/testdata/` without modification.
