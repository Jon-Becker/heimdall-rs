# heimdall-decompiler

Decompiles EVM bytecode to human-readable representations

ABI inference uses symbolic calldata offsets and length checks to distinguish byte tails from
word arrays. Without signature resolution, byte tails default to `bytes` (also compatible with
`string`) and word arrays default to `uint256[]`. Optimized code can erase unused array element
types and nesting; the generated ABI preserves the observed shape but cannot recover those
source types without additional evidence.
