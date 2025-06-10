//! Benchmark for testing VM performance with ten thousand hash operations.

use alloy::primitives::Address;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_common::utils::strings::decode_hex;
use heimdall_vm::core::vm::VM;
use tokio::runtime::Runtime;

fn test_ten_thousand_hashes(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_vm");

    group.sample_size(100);
    group.bench_function(BenchmarkId::from_parameter("ten_thousand_hashes"), |b| {
        b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
            // build the evm
            let mut evm = VM::new(
                &decode_hex(include_str!("./testdata/ten_thousand_hashes.hex"))
                    .expect("invalid bytecode"),
                &[],
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

criterion_group!(benches, test_ten_thousand_hashes);
criterion_main!(benches);
