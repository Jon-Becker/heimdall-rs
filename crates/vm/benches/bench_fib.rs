//! Benchmark for testing VM performance with Fibonacci sequence calculations.

use alloy::primitives::Address;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_common::utils::strings::decode_hex;
use heimdall_vm::core::vm::VM;
use tokio::runtime::Runtime;

fn test_fib(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_vm");

    group.sample_size(500);
    group.bench_function(BenchmarkId::from_parameter("fib"), |b| {
        b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
            // build the evm
            let mut evm = VM::new(
                &decode_hex(include_str!("./testdata/fib.hex")).expect("invalid bytecode"),
                &decode_hex("0x0000000000000000000000000000000000000000000000000000000000000064")
                    .expect("invalid calldata"),
                Address::default(),
                Address::default(),
                Address::default(),
                0,
                u128::MAX,
            );

            // run the evm
            let start = std::time::Instant::now();
            let resp = evm.execute().expect("evm panic");

            assert_eq!(
                resp.returndata,
                &[
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 19, 51,
                    219, 118, 167, 197, 148, 191, 195
                ]
            );

            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(benches, test_fib);
criterion_main!(benches);
