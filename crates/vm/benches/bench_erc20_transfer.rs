//! Benchmark for testing VM performance with ERC20 transfer operations.

use alloy::primitives::Address;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_common::utils::strings::decode_hex;
use heimdall_vm::core::vm::VM;
use tokio::runtime::Runtime;

fn test_erc20_transfer(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_vm");

    group.sample_size(10000);
    group.bench_function(BenchmarkId::from_parameter("erc20_transfer"), |b| {
        b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
            // build the evm
            let mut evm = VM::new(
                &decode_hex(include_str!("./testdata/weth9.hex")).expect("invalid bytecode"),
                &decode_hex("0xa9059cbb0000000000000000000000006666666b0B46056247E7D6cbdb78287F4D12574d0000000000000000000000000000000000000000000000000000000000000000").expect("invalid calldata"),
                Address::default(),
                Address::default(),
                Address::default(),
                0,
                u128::MAX,
            );

            // run the evm
            let start = std::time::Instant::now();
            let _ = evm.execute().expect("evm panic");
            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(benches, test_erc20_transfer);
criterion_main!(benches);
