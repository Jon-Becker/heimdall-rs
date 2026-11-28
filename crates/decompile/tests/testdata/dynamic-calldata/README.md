# Dynamic calldata fixtures

`DynamicCalldata.sol` contains the two minimal examples from
[issue #771](https://github.com/Jon-Becker/heimdall-rs/issues/771). The `.hex` files
contain deployed runtime bytecode, without a `0x` prefix, generated with solc
`0.8.30+commit.73712a01`, optimizer enabled with 200 runs, Cancun, and CBOR metadata
disabled. Tests use these checked-in runtimes and require no compiler, RPC, or
signature service.

To regenerate with native solc 0.8.30, run from this directory:

```sh
solc --base-path . --standard-json < input.json > /tmp/dynamic-calldata-output.json
python3 - <<'PY'
import json
from pathlib import Path

output = json.loads(Path('/tmp/dynamic-calldata-output.json').read_text())
assert not any(e['severity'] == 'error' for e in output.get('errors', []))
for name, contract in output['contracts']['DynamicCalldata.sol'].items():
    Path(f'{name}.hex').write_text(contract['evm']['deployedBytecode']['object'] + '\n')
PY
```

SHA-256 digests of the decoded runtime bytes:

- `ArbitraryCallVulnerable` (452 bytes):
  `e29043cee61582b23dc4df6d46cd3db9e987f7ff5b3001eda0ffc663173cbc44`
- `AbiInferenceMatrix` (464 bytes):
  `d7bf5cb15f983c04d8c518f15cacd5b3a5a2df95bf0a8bcb53d2e6b193c7b2b7`

The first digest matches the report. The report's matrix digest
`71cd5c558190926a4dc247fbd9b4564485b8469cf1fcef662c7dcf7aae2700f2`
does not match compilation of its supplied minimal source under these settings.
The fixture retains the reported selector `0x8484e693` and all four dynamic heads.

The matrix's unused `address[]` and `uint256[][]` arguments use the same runtime
decoder: each checks that its offset plus 32 plus `length * 32` fits in calldata.
Their element type and nesting are unavailable without signature resolution or
additional element accesses. The `bytes` and `string` arguments also share a
single decoder, with an unscaled byte length. The regression therefore expects
`(bytes,uint256[],uint256[],bytes)`, preserving the observable dynamic shapes and
arity without claiming to recover erased source types.

As a cross-check, changing those three unused parameters to
`uint256[]`, `uint256[]`, and `bytes` produces identical deployed bytecode except
for the selector (`0x16c3d4bc` instead of `0x8484e693`).
