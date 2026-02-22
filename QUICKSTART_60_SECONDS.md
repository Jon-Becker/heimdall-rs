# Quickstart: Analyzing Your First Smart Contract in 60 Seconds

This guide gets you from zero to analyzing smart contracts in under a minute.

## Prerequisites

- Rust installed (`curl https://sh.rustup.rs -sSf | sh`)
- A terminal

## Installation (20 seconds)

```bash
# Install bifrost (Heimdall's installer)
curl -L http://get.heimdall.rs | bash

# Install Heimdall
bifrost
```

Open a new terminal for the `heimdall` command to be available.

## Your First Analysis (40 seconds)

### Example 1: Decompile a Contract

Let's analyze Uniswap V2 Router:

```bash
heimdall decompile 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D \
  --rpc-url https://eth.llamarpc.com
```

**What you get:**
- Decompiled Solidity-like code
- Function signatures
- Storage layout
- Control flow analysis

**Output structure:**
```
output/
├── decompile/
│   ├── 0x7a25.../
│   │   ├── decompiled.sol      # Main output
│   │   ├── functions.json      # Function metadata
│   │   └── storage.json        # Storage variables
```

### Example 2: Decode a Transaction

Got a mysterious transaction? Decode it:

```bash
heimdall decode 0xYOUR_TX_HASH \
  --rpc-url https://eth.llamarpc.com
```

**Output:**
```
Function: swapExactTokensForTokens
Parameters:
  amountIn: 1000000000000000000 (1.0 tokens)
  amountOutMin: 950000000000000000 (0.95 tokens)
  path: [0xA0b8..., 0x6B17...]
  to: 0x742d...
  deadline: 1709500000
```

### Example 3: Disassemble Bytecode

Understand what's happening under the hood:

```bash
heimdall disassemble 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D \
  --rpc-url https://eth.llamarpc.com
```

**Output:**
```
0x000: PUSH1 0x80
0x002: PUSH1 0x40
0x004: MSTORE
0x005: CALLVALUE
...
```

## Common Patterns

### Analyze a Local Contract

```bash
# From bytecode file
heimdall decompile --bytecode $(cat contract.bin)

# From Solidity compiler output
heimdall decompile --bytecode $(solc --bin contract.sol | tail -1)
```

### Generate Control Flow Graph

```bash
heimdall cfg 0xCONTRACT_ADDRESS \
  --rpc-url https://eth.llamarpc.com \
  --output graph.dot

# Convert to image (requires graphviz)
dot -Tpng graph.dot -o graph.png
```

### Dump Contract Storage

```bash
heimdall dump 0xCONTRACT_ADDRESS \
  --rpc-url https://eth.llamarpc.com \
  --slots 0-100
```

## Configuration

Create `~/.heimdall/config.toml`:

```toml
[general]
default_rpc_url = "https://eth.llamarpc.com"
output_dir = "./heimdall-output"
verbosity = "info"

[decompile]
include_solidity = true
include_yul = false
simplify_output = true
```

## Troubleshooting

### Error: "Failed to fetch bytecode"

**Problem:** RPC endpoint unreachable or contract doesn't exist.

**Solution:**
```bash
# Verify contract exists
cast code 0xADDRESS --rpc-url https://eth.llamarpc.com

# Try different RPC
heimdall decompile 0xADDRESS --rpc-url https://cloudflare-eth.com
```

### Error: "Decompilation failed"

**Problem:** Contract uses complex/obfuscated bytecode.

**Solution:**
```bash
# Try disassembly first to understand structure
heimdall disassemble 0xADDRESS --rpc-url https://eth.llamarpc.com

# Use verbose mode for debugging
heimdall decompile 0xADDRESS --rpc-url https://eth.llamarpc.com -vvv
```

### Error: "Transaction not found"

**Problem:** Transaction hash invalid or on different network.

**Solution:**
```bash
# Specify correct network
heimdall decode 0xTX_HASH --rpc-url https://arbitrum.llamarpc.com  # Arbitrum
heimdall decode 0xTX_HASH --rpc-url https://polygon-rpc.com          # Polygon
```

## Next Steps

1. **Advanced Decompilation**: Check [Advanced Usage](https://jbecker.dev/r/heimdall-rs/wiki/decompile)
2. **Custom Modules**: Learn to extend Heimdall at [Module Development](https://jbecker.dev/r/heimdall-rs/wiki/contributing)
3. **Integration**: Use Heimdall in CI/CD pipelines (see examples/ci)

## Why This Matters

### Security Auditing
```bash
# Quickly verify a contract's claimed functionality
heimdall decompile 0xSUSPICIOUS_CONTRACT --rpc-url https://eth.llamarpc.com
```

### Reverse Engineering
```bash
# Understand how a protocol works without source code
heimdall decompile 0xPROTOCOL_CONTRACT --rpc-url https://eth.llamarpc.com
```

### Transaction Analysis
```bash
# Debug failed transactions
heimdall decode 0xFAILED_TX --rpc-url https://eth.llamarpc.com --trace
```

## ASCII Art: Heimdall's Analysis Pipeline

```
┌─────────────┐
│ Bytecode    │
│ (on-chain)  │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Disassemble │ ← Extract opcodes
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Build CFG   │ ← Control flow graph
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Symbolic    │ ← Analyze execution paths
│ Execution   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Decompile   │ ← Generate readable code
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Output:     │
│ - .sol file │
│ - Storage   │
│ - Functions │
└─────────────┘
```

## Real-World Example: Analyzing a Hack

Let's analyze the infamous Tornado Cash contract:

```bash
heimdall decompile 0xd90e2f925DA726b50C4Ed8D0Fb90Ad053324F31b \
  --rpc-url https://eth.llamarpc.com \
  --output tornado-analysis

# Check the output
cat tornado-analysis/decompiled.sol | head -50
```

You'll see:
- Merkle tree verification logic
- Zero-knowledge proof verification
- Deposit/withdrawal mechanisms
- Event emissions

**Time elapsed:** 45 seconds from installation to analysis complete.

## Cheat Sheet

```bash
# Decompile
heimdall decompile 0xADDR --rpc-url URL

# Decode transaction
heimdall decode 0xTX --rpc-url URL

# Disassemble
heimdall disassemble 0xADDR --rpc-url URL

# Control flow graph
heimdall cfg 0xADDR --rpc-url URL

# Dump storage
heimdall dump 0xADDR --rpc-url URL

# Get help
heimdall --help
heimdall COMMAND --help
```

---

**Stuck?** Check the [Troubleshooting Wiki](https://jbecker.dev/r/heimdall-rs/wiki/troubleshooting) or open an [issue](https://jbecker.dev/r/heimdall-rs/issues).
