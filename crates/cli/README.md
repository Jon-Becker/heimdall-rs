# heimdall-cli

This crate is a very simple clap-based CLI that allows you to interact with the Heimdall library by wrapping [heimdall-core](../core/README.md) modules.

## Extract strings

`heimdall strings <TARGET>` extracts printable ASCII runs (`0x20`–`0x7e`) from PUSH1–PUSH32 payloads,
one string per line, with a minimum length of 4. Use `-n` / `--min-length` to change the
minimum. Strings retain their bytecode order and duplicates. Empty input or no matches
produces no output and succeeds.

```sh
heimdall strings 0x6448656c6c6f0064776f726c64
# Hello
# world
heimdall strings bytecode.hex -n 8
heimdall strings bytecode.hex --full-scan
heimdall strings 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 --rpc-url https://eth.example.com
```

Targets accept hex bytecode (with or without `0x`), a file containing hex bytecode,
or a contract address using `--rpc-url` or the configured RPC provider. Local hex and
file inputs work offline. Redirect stdout to save the strings to a file.

By default, each PUSH payload is scanned separately, reducing printable opcode noise.
Truncated PUSH instructions scan only their available payload bytes. Strings stored
outside PUSH payloads may be missed; use `--full-scan` to scan every byte like Unix
`strings`, including opcodes and metadata. Either mode can include printable fragments
of constants or metadata. This does not reconstruct strings assembled at runtime or
decode Unicode.
