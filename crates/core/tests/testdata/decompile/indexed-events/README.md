# Indexed event regression fixtures

These Ethereum logs were fetched with `eth_getLogs` and checked against each transaction receipt.
Tests use the stored logs offline; they do not need RPC credentials or a signature service.
Every log retains its emitter, block hash, transaction hash, log index, topics, and data.

| Contract | Event | Expected indexed positions (zero-based) | Receipt |
| --- | --- | --- | --- |
| `0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d` | `Transfer(address,address,uint256)` | 0,1,2 | [transaction](https://etherscan.io/tx/0x9442f3dca0f7bcd9b2bee35200040ad80125abdee5fe376207106f71ac8299a8) |
| `0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d` | `ApprovalForAll(address,address,bool)` | 0,1 | [transaction](https://etherscan.io/tx/0x141ee981461ab8f6bb87e3a8d59f0cd6316a5298923d402907e0e77dcd84650f) |
| `0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401` | `NameWrapped(bytes32,bytes,address,uint32,uint64)` | 0 | [transaction](https://etherscan.io/tx/0x48889bf4df41bd020c0496db7ad3071ecfbe5952ffb67eeb18eec2b15351cab1) |
| `0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401` | `NameUnwrapped(bytes32,address)` | 0 | [transaction](https://etherscan.io/tx/0x0e8bf6a49b89c8202fec75fede8ab3de1271d2cce502387e1d3c4ad9779f32a3) |
| `0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2` | `Transfer(address,address,uint256)` | Ambiguous; leave unchanged | [transaction](https://etherscan.io/tx/0x6e9011b12b7e66e6fba61ec9b9b0c9eecfa6b1aa5edfd47eb06dcb089209212b) |

The two BAYC events cover ERC-721’s three-indexed `Transfer` and mixed `ApprovalForAll` layout.
ENS NameWrapper covers dynamic event data and a hash topic that cannot fit in an address.
WETH uses the same `Transfer(address,address,uint256)` signature as ERC-721 but a different layout.
Its sampled words admit several ABI layouts. The test requires no correction rather than an ERC-20 convention guess.

The mock-RPC decompilation test replays a captured BAYC log and a small emitter bytecode program.
It checks the optional RPC path and all output formats; it does not claim to reproduce BAYC runtime bytecode.
