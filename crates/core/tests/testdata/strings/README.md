# Strings integration fixtures

Unmodified deployed runtime bytecode fetched from Ethereum mainnet with
`eth_getCode(address, "0x18d4497")` at block **26,035,351**.

| Fixture | Contract address | Bytes | SHA-256 of decoded bytecode |
| --- | --- | ---: | --- |
| uniswap_v2_usdc_weth | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | 11,293 | `8b5db55fa9ab3b9527508d4abe0b39eb588bf310270c8e04b3f38214e8ba63b4` |
| dai | `0x6B175474E89094C44Da98b954EedeAC495271d0F` | 7,904 | `5ce810033426a017721777f241665c27bdd4663ca8a1c27558b4b372a1731f4e` |

The `<contract>.json` snapshots list every expected string from PUSH payloads at
minimum length 4. `<contract>.full_scan.json` retains the original full-bytecode
snapshots. Both are generated independently of the Rust extractor with Python:

```python
runs = lambda data: [s.decode("ascii") for s in re.findall(rb"[ -~]{4,}", data)]
full_scan = runs(bytecode)
push_only = []
pc = 0
while pc < len(bytecode):
    opcode = bytecode[pc]
    pc += 1
    if 0x60 <= opcode <= 0x7f:
        size = opcode - 0x5f
        push_only.extend(runs(bytecode[pc:pc + size]))
        pc += size
```

DAI has 20 PUSH strings versus 145 full-scan strings; Uniswap has 29 versus 173.
PUSH-only output retains DAI revert messages and the pair's `UniswapV2: LOCKED`
messages. Full scanning also finds long Uniswap messages stored outside PUSH
payloads. Constants and metadata can still contribute printable fragments in
either mode; the linear instruction walk does not identify executable sections.

Both core integration tests and CLI process tests compare complete output against
these files in both modes. JSON preserves trailing spaces without whitespace-only
diff warnings. No RPC credentials are required.
