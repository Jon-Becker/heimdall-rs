# Strings integration fixtures

Unmodified deployed runtime bytecode fetched from Ethereum mainnet with
`eth_getCode(address, "0x18d4497")` at block **26,035,351**.

| Fixture | Contract address | Bytes | SHA-256 of decoded bytecode |
| --- | --- | ---: | --- |
| uniswap_v2_usdc_weth | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | 11,293 | `8b5db55fa9ab3b9527508d4abe0b39eb588bf310270c8e04b3f38214e8ba63b4` |
| dai | `0x6B175474E89094C44Da98b954EedeAC495271d0F` | 7,904 | `5ce810033426a017721777f241665c27bdd4663ca8a1c27558b4b372a1731f4e` |

The `.json` files list every expected string at minimum length 4, generated
independently of the Rust extractor with Python:

```python
[run.decode("ascii") for run in re.findall(rb"[ -~]{4,}", bytecode)]
```

These snapshots intentionally retain printable opcode bytes, duplicates, and
adjacent strings without non-printable separators. DAI contains revert messages
such as `Dai/insufficient-balance`; the pair contains `UniswapV2: LOCKED` and
`UniswapV2: INVALID_SIGNATURE`. Both core integration tests and CLI process tests
compare their entire output against these files. JSON preserves trailing spaces
without whitespace-only diff warnings. No RPC credentials are required.
