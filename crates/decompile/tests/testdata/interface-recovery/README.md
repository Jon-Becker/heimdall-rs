# Offline interface recovery cases

These fixtures contain captured deployed runtime bytecode, chain/address provenance, Keccak-256 digests of decoded bytes, selected expected selectors, and observed overload declarations. They do not require RPC access, an explorer, a signature service, or pre-existing local caches. Captures date from 19 October 2026. The failures first reproduced on v0.9.2; source owners were also checked against main `9de15de029ac1128c7c7566672d472f104a9747a`.

There are 74 runtime fixtures: 55 Base, 15 Ethereum, and four Monad addresses. The scanner test checks 94 selected address-selector entries. These are positive expectations, not a complete verified ABI for every fixture. Separate minimal fixtures check exact selector sets and entry points, including negative controls for body constants, PUSH data, nonzero calldata offsets, and unrelated comparisons.

## Reproduce

```sh
cargo test -p heimdall-decompiler --test interface_recovery
cargo test -p heimdall-decompiler --lib
cargo test -p heimdall-common --lib signature_cache_regressions
cargo test -p heimdall-vm --lib
```

The new integration target is separate from the older network integration target. An absent `RPC_URL` cannot make these new tests exit successfully without running their assertions.

## Dispatcher examples

The corpus includes 41 selected SUB/fallthrough omissions, 19 short-push omissions, one zero-selector omission, and fourteen omissions caused by a memory-derived calldata offset. Those 75 omissions are a subset of the 94 selected entries; the other entries exercise previously discovered functions involved in lookup, overload, or argument-inference examples.

- Base `0x0c7558f634b3465ff7637500d5a710731393c8f2`: selector `0xf2fde38b` uses SUB and falls through on equality.
- Base `0xe819c3445f505d2b8f47b8cc91d4a40b29ea4994`: selector `0x00761111` uses PUSH3 instead of PUSH4.
- Ethereum `0x6b712120e610c572eb4ff1c4745ea796fd94f20a`: fourteen selectors use a calldata offset loaded through memory. Rendering produces `msg.data[var_b]` after assigning zero, so source-string matching misses the dispatcher.

Base `0x434ec1685630d347c85327d8aab97ad69e7aa050` also exposes an invalid-fallback jump while probing candidate selectors. An out-of-range destination previously overflowed the VM instruction pointer. JUMP/JUMPI now reject missing destinations; false JUMPI conditions still ignore their destination. Regression tests cover oversized values, code-end offsets, non-JUMPDEST bytes, and the complete captured decompilation.

The isolated test also covers XOR, inverted equality, PUSH1/PUSH2/PUSH3/PUSH4, leading zeros, PUSH0 equality, and ISZERO for selector zero. The structural matcher follows observed calldata-zero loads and their SHR/DIV projections; it does not treat every short constant as a function selector. This does not claim support for every possible custom dispatcher.

Related open work: [#685](https://github.com/Jon-Becker/heimdall-rs/pull/685) covers Vyper inverse dispatch; [#708](https://github.com/Jon-Becker/heimdall-rs/pull/708) covers leading-zero PUSH4 string normalization. These cases overlap, but shorter PUSH instructions and memory-derived offsets also need coverage. This implementation uses opcode dependencies rather than rendered-expression substring comparisons.

## ABI overload examples

The following seven same-name families appeared in generated source while the raw ABI retained only one member. The ABI tests construct every family, test both input orders, retain detailed selector/signature pairing, and deduplicate exact repeated entries. Source declarations here describe observed decompiler output; they do not prove every inferred type matches original source.

| Chain | Address | Family |
|---|---|---|
| base | `0x14f36be359b87e5842a097a10de2c3ade4256891` | `setTargetFunctionRole` |
| base | `0x6103f652a4c4d6f13bc1d78a7103e54d8ed13b48` | `isClassificationValid` |
| base | `0x6998be5fea8d8725a739e9941953ee9a1461332c` | `isPaused`, `pause` |
| base | `0xcbfacf64179602ea8929a4a3913bb181eff48b29` | `deactivatePoints` |
| base | `0xe068e83e2fb9b8d35f48814c3cb00906a3b79e36` | `getGnsPriceUsd` |
| monad | `0x331e73e661c2d7097781e00b4bf57b3dcbcd7415` | `lpUnrealizedPnlUsd` |

The families are `setTargetFunctionRole(address,bytes4[],uint64)` / `(address,bytes4,uint8)`, `getGnsPriceUsd(uint8)` / `(uint8,uint256)`, `isClassificationValid(address,bytes32)` / `(address)`, `deactivatePoints()` / `(address)`, `isPaused(bytes4)` / `()`, `pause(uint256)` / `(bytes4,uint256)`, and `lpUnrealizedPnlUsd(address)` / `()`.

Error/event overload tests also check distinct signatures under one name and observations made only in a fallback. They preserve entries; they do not reconstruct indexed layouts or missing error argument schemas.

## Signature lookup examples

These captured runtimes produced unresolved `0x095ea7b3` declarations even when their inferred inputs matched `address,uint256`:

- base `0x2e0e05342443f9138372f82265108c8cc6ce2b7a`
- base `0x50d802a2ae34832f7096db140c5d976cbc7ffec8`
- base `0x5ea8a5b4a854402eca8a7ad35a14932a6dbfa736`
- base `0x66eb029597a7e48696ce3a860169403b6bc40266`
- base `0x91a805f2ad9325504199f49bc33023b04c9cfeda`
- ethereum `0x6a33dbefa2502bee9fae77f78039a855d445f71d`
- ethereum `0x6bfc971ff46934817ebe5b218da5d9b0bf1ed077`
- ethereum `0xb5d1eea7c0d57a6643830db8dea1062e16b2daf3`

A local reproduction found a cached negative lookup despite a fresh lookup containing `approve(address,uint256)`. The cache tests seed a negative response without HTTP, check that it is retried, check positive-cache reuse, and check that absent/error responses do not create a new cached miss. This reproduces one failure mechanism; it does not establish that every unresolved declaration on these addresses had the same cache cause.

A second lookup defect affects positive results: the custom signature serializers write maps with ABI input objects, while binary deserialization expects struct fields with input strings. Actual ResolvedFunction/ResolvedError/ResolvedLog cache records fail to round-trip. The shared serializer now preserves the human-readable JSON format but writes binary-compatible string inputs and struct fields. Tests cover all three real record types and JSON shape. Supplied ABI signatures can then resolve offline through their cache entries.

Successful nonempty candidates retain the existing cache lifetime. Negative lookups are no longer cached, so repeated unknown signatures can cause additional lookup requests. Request errors still propagate. Type compatibility tests include the real `0x095ea7b3` collision `sign_szabo_bytecode(bytes16,uint128)`, a wrong-selector candidate, and indistinguishable same-type name collisions. A four-byte selector cannot uniquely prove an original source name.

## Dynamic bytes example

Ethereum implementation `0x390eae68465090e8a1e4b202b5ccb6ae3142583f` produces `Unresolved_4f1ef286(address,uint256)` before this change. The corresponding function signature is `upgradeToAndCall(address,bytes)`. A verified counterpart is Base implementation `0x620244706ba6c771ec417662a0bfb6fe6d1c5ae2`; the captured executable runtimes agree after terminal metadata and the deployment self-address are normalized. Both runtime fixtures are included. `base-upgrade-reference.json` records the relevant verified ABI entries for reference; inference tests do not feed it to the decompiler.

The copy reads its length at `4 + calldataload(36)` and its data at `36 + calldataload(36)`. The regression decompiles the runtime with signature lookup disabled and asserts `address,bytes`, so providing a known ABI cannot hide the inference failure. Separate candidate tests accept `upgradeToAndCall(address,bytes)` and reject incompatible signatures. The inferred head/tail distinction excludes tail loads from new head arguments. Byte-counted copies distinguish bytes/string layouts from word-counted array copies. Dynamic length evidence supports array candidates without accepting a known byte-counted payload as an array.

This is bounded inference for these layouts, not a claim that every dynamic tuple, static array, custom encoding, or source-level bytes/string distinction is recoverable. Input names remain `arg0`, `arg1`: selector databases contain canonical signatures, not original parameter names.

The CLI also resolves `upgradeToAndCall` by name when supplied reference signatures use the repaired binary cache. This checks names and inputs only. The captured function can still receive a spurious inferred `uint256` output; canonical signature lookup does not supply return types. Return inference remains separate work.

## Known event problem: not addressed

The same Ethereum implementation and its verified Base counterpart expose `RoleGranted(bytes32,address,address)`. The verified event has all three inputs indexed; current ABI construction emits resolved event parameters with `indexed: false`. The topic hash does not encode indexed positions. Reconstructing them requires carrying LOG topic/data observations through analysis; historical logs alone can remain ambiguous. That recovery is outside this PR. Anonymous-event inference and missing event/error parameter inference also remain separate work.

## Complete address index

`cases.json` is the machine-readable manifest. Digests describe decoded runtime bytes, excluding the trailing fixture newline. Full source bundles, creation bytecode, credentials, RPC endpoints, and application metadata are not included.

| Chain | Address | Selected selectors | Observed mechanism |
|---|---|---|---|
| base | `0x0039ef7aca47d14090518bc91b22fe822cefa912` | 1 | subtraction_dispatch_not_recognized |
| base | `0x039aa58a8394b0ea7c53f0f4283e241d2608e2d6` | 1 | subtraction_dispatch_not_recognized |
| base | `0x054e2da15f52a4317265c6f39c19b68d0e29f4ca` | 1 | subtraction_dispatch_not_recognized |
| base | `0x0c7558f634b3465ff7637500d5a710731393c8f2` | 1 | subtraction_dispatch_not_recognized |
| base | `0x108315c4447de7a166fb3c892ff810afad80363b` | 1 | subtraction_dispatch_not_recognized |
| base | `0x13262cdd9004d6cb905c156a4a1647510eeda9b1` | 1 | subtraction_dispatch_not_recognized |
| base | `0x14f36be359b87e5842a097a10de2c3ade4256891` | 1 | same_name_overload_replaced |
| base | `0x169709875f8adbd72d23fb03c45bb6af29087196` | 1 | subtraction_dispatch_not_recognized |
| base | `0x195eb3f5d3e7cb7d35051f6ea37c5f6470647cb6` | 1 | subtraction_dispatch_not_recognized |
| base | `0x1a5a52831b49fdedf9de7ead38642aa497f7c05e` | 1 | subtraction_dispatch_not_recognized |
| base | `0x20e838dec4c8a5809ca350a5026bc6b000f2b17d` | 1 | short_or_zero_selector_not_discovered |
| base | `0x2452b04dfe96270b7de90c84519da7f3a132e73e` | 1 | subtraction_dispatch_not_recognized |
| base | `0x26074fdb30f30c030a6fd0d88551585939a14b22` | 1 | short_or_zero_selector_not_discovered |
| base | `0x2e0e05342443f9138372f82265108c8cc6ce2b7a` | 1 | independent_approve_name_resolution |
| base | `0x318c11a1ab1ff8f290723b3b3f3dc417c1bbeba3` | 1 | subtraction_dispatch_not_recognized |
| base | `0x34216b25255ffbc121b9b23c730af1467e0c26a7` | 1 | subtraction_dispatch_not_recognized |
| base | `0x35251169363ddccfe1ebdaef91a5f237ca4004e9` | 1 | subtraction_dispatch_not_recognized |
| base | `0x3be6de9e5ad7222550c49fb4ebed0a8defad9528` | 1 | subtraction_dispatch_not_recognized |
| base | `0x3f7ef4c10ddd9b77fcc795258853294b8417e2eb` | 1 | subtraction_dispatch_not_recognized |
| base | `0x434ec1685630d347c85327d8aab97ad69e7aa050` | 1 | short_or_zero_selector_not_discovered |
| base | `0x46eb37efff620159fe394347069cb930bb63f7d0` | 1 | subtraction_dispatch_not_recognized |
| base | `0x50d802a2ae34832f7096db140c5d976cbc7ffec8` | 1 | independent_approve_name_resolution |
| base | `0x51d5314052ab906d197d64f6c14e171b0d5fe6b5` | 1 | subtraction_dispatch_not_recognized |
| base | `0x5d0d5cbbfa2ff20c7669618d4642074b4151a565` | 1 | subtraction_dispatch_not_recognized |
| base | `0x5ea8a5b4a854402eca8a7ad35a14932a6dbfa736` | 1 | independent_approve_name_resolution |
| base | `0x6103f652a4c4d6f13bc1d78a7103e54d8ed13b48` | 3 | same_name_overload_replaced, short_or_zero_selector_not_discovered |
| base | `0x620244706ba6c771ec417662a0bfb6fe6d1c5ae2` | 2 | verified_reference_for_dynamic_bytes |
| base | `0x66b20db3e80de14bbec32e8e01a8cc31a370d327` | 1 | short_or_zero_selector_not_discovered |
| base | `0x66eb029597a7e48696ce3a860169403b6bc40266` | 2 | independent_approve_name_resolution, short_or_zero_selector_not_discovered |
| base | `0x6998be5fea8d8725a739e9941953ee9a1461332c` | 2 | same_name_overload_replaced |
| base | `0x6f56e561c1089bd4c480ee56907e6588357f74db` | 1 | subtraction_dispatch_not_recognized |
| base | `0x783fc71b68f48c8f28e72e7011f0188f8d3e6822` | 1 | subtraction_dispatch_not_recognized |
| base | `0x82fbf93b400e75d76e46e1fb84b55fd8e6f45f90` | 1 | subtraction_dispatch_not_recognized |
| base | `0x83f3504c3ec33d7c46914656a1e79cce636fb399` | 1 | subtraction_dispatch_not_recognized |
| base | `0x864ef309c30dc03f5b218401383e7475cdeb960d` | 1 | subtraction_dispatch_not_recognized |
| base | `0x888a7e5d0a6feb1ad856404b276e965ef8a99491` | 1 | short_or_zero_selector_not_discovered |
| base | `0x91a805f2ad9325504199f49bc33023b04c9cfeda` | 1 | independent_approve_name_resolution |
| base | `0x92a4930a5b6d22748c40eee82bb4c5bd31362d5a` | 1 | subtraction_dispatch_not_recognized |
| base | `0x93ca322589751b5ad3854b9331b584cc68e037aa` | 1 | subtraction_dispatch_not_recognized |
| base | `0x93e44a0dd96add2e1b9e224ca2e1ab87b3723a0b` | 1 | short_or_zero_selector_not_discovered |
| base | `0x95af7c7654cb2b7da81982cded0cd88e56bfa21a` | 1 | subtraction_dispatch_not_recognized |
| base | `0xb0db1e21422def60cd9922955cdb9e2a7dc93aa8` | 1 | subtraction_dispatch_not_recognized |
| base | `0xb4963732f02bd659d6e4751d4baa4d18613ff15e` | 1 | subtraction_dispatch_not_recognized |
| base | `0xb96d385d29e9209fdae8f1e5c1af344661ea1178` | 1 | subtraction_dispatch_not_recognized |
| base | `0xcbfacf64179602ea8929a4a3913bb181eff48b29` | 1 | same_name_overload_replaced |
| base | `0xd93e888d408b0f6f8216fab06f22cd0453bcca76` | 1 | subtraction_dispatch_not_recognized |
| base | `0xda4efc48e339a11fcc057959375ecc8d78aea860` | 1 | short_or_zero_selector_not_discovered |
| base | `0xdd90a13501458c1aeab187cbf66d5119afcfe11b` | 1 | subtraction_dispatch_not_recognized |
| base | `0xdd9e1bccee29ed8b6d72ab505b5b5527c8baf58b` | 1 | short_or_zero_selector_not_discovered |
| base | `0xe068e83e2fb9b8d35f48814c3cb00906a3b79e36` | 1 | same_name_overload_replaced |
| base | `0xe1977938f43ff236116c4718e6b5b8e6436d798b` | 1 | short_or_zero_selector_not_discovered |
| base | `0xe819c3445f505d2b8f47b8cc91d4a40b29ea4994` | 1 | short_or_zero_selector_not_discovered |
| base | `0xf32e12a21fb8fcc1f2e012b52308d7d6998fd93e` | 1 | subtraction_dispatch_not_recognized |
| base | `0xf528340d8c2e0e9a5e90e8db86b3d783793c449b` | 1 | subtraction_dispatch_not_recognized |
| base | `0xfb52b4dec98d25a3d433982f3f10a0dee306b4b9` | 1 | subtraction_dispatch_not_recognized |
| ethereum | `0x015e29f1a7dc97ea069d8fcd4e7a5753dd5a80c5` | 1 | subtraction_dispatch_not_recognized |
| ethereum | `0x06e0c6ea7a0287cfd1bd84adaa6062225166eeaa` | 1 | short_or_zero_selector_not_discovered |
| ethereum | `0x390eae68465090e8a1e4b202b5ccb6ae3142583f` | 2 | dynamic_bytes_argument_inferred_as_uint256 |
| ethereum | `0x50a15277aa8bcb0c6e3837eb4c6836134c89d4a3` | 1 | subtraction_dispatch_not_recognized |
| ethereum | `0x53302aa9c82464e489e7cdf2025388e68d021554` | 2 | short_or_zero_selector_not_discovered |
| ethereum | `0x6a33dbefa2502bee9fae77f78039a855d445f71d` | 1 | independent_approve_name_resolution |
| ethereum | `0x6b712120e610c572eb4ff1c4745ea796fd94f20a` | 14 | memory_offset_dispatch_not_recognized |
| ethereum | `0x6bfc971ff46934817ebe5b218da5d9b0bf1ed077` | 1 | independent_approve_name_resolution |
| ethereum | `0x7117b8558251476140ee193fec361654af9be67e` | 1 | short_or_zero_selector_not_discovered |
| ethereum | `0x7721bcb1c2e5307cbcea6eb2a359b7f0781a8725` | 1 | subtraction_dispatch_not_recognized |
| ethereum | `0x7ad90330f3518c32362583072de24af3d9df7b91` | 1 | short_or_zero_selector_not_discovered |
| ethereum | `0xa341c5da8db25924edcbf00698784b5d801d88a2` | 1 | subtraction_dispatch_not_recognized |
| ethereum | `0xb5d1eea7c0d57a6643830db8dea1062e16b2daf3` | 1 | independent_approve_name_resolution |
| ethereum | `0xc9968d9bbaba9462a5d228be6617c870a2118082` | 1 | short_or_zero_selector_not_discovered |
| ethereum | `0xe1046fcb1057ef82b68f3a6b8ebb0e411cf334e0` | 1 | subtraction_dispatch_not_recognized |
| monad | `0x31ed51e2f5be99db9b12ccc62a8f6f612985aa37` | 1 | subtraction_dispatch_not_recognized |
| monad | `0x331e73e661c2d7097781e00b4bf57b3dcbcd7415` | 1 | same_name_overload_replaced |
| monad | `0x65d49ccc99b65291079d442812be7b7921ccf3e3` | 1 | short_or_zero_selector_not_discovered |
| monad | `0xcd3d92676aae4335b90452365b99b306a25cd2d6` | 1 | subtraction_dispatch_not_recognized |
