# heimdall-core

This crate is the core of the Heimdall library. It contains all module implementations, such as decompilation, disassembly, decoding, etc.

## String extraction benchmarks

```sh
cargo bench -p heimdall-core --bench bench_strings
```

Benchmarks PUSH-only and full-scan extraction of the pinned DAI and Uniswap V2
USDC/WETH fixtures at the default minimum length of four. Criterion reports time
per extraction and throughput in input bytes per second. Fixture decoding and
buffer allocation happen outside the timed loop; extraction and writes into a
reused memory buffer are measured. Inputs and output are passed through
`black_box` to keep the work observable. No RPC, file reads, or terminal output
are included in the measurement.
