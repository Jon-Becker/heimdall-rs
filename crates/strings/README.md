# heimdall-strings

Extract printable ASCII runs from EVM bytecode. PUSH1–PUSH32 payloads are scanned
independently by default; `full_scan` includes all bytes. Order and duplicates are
preserved, and the default minimum length is four.

Use `strings(&args, &mut output).await` with `StringsArgs` or
`StringsArgsBuilder` for raw hex, hex files, and contract addresses. Address inputs
use the supplied RPC URL. The module does not load CLI configuration or write to
stdout; callers supply a writer and handle buffering and flushing.

For bytecode already in memory, use `write_strings` directly:

```rust
use heimdall_strings::write_strings;

let mut output = Vec::new();
write_strings(b"\x64hello", 4, false, &mut output).unwrap();
assert_eq!(output, b"hello\n");
```

The APIs are also available through `heimdall_core::heimdall_strings`. Shared
real-contract fixtures and integration tests live in `crates/core/tests/`.
