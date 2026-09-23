//! Benchmark for testing decode functionality performance.

#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_decoder::{decode, DecodeArgsBuilder};
use tokio::runtime::Runtime;

fn test_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_decoder");

    let calldatas = [
        ("transfer", include_str!("../tests/testdata/decode/transfer.hex")),
        ("uniswap", include_str!("../tests/testdata/decode/uniswap_swap.hex")),
        ("seaport", include_str!("../tests/testdata/decode/seaport_simple.hex")),
    ];

    for (name, calldata) in calldatas.into_iter() {
        group.sample_size(10000);
        group.bench_with_input(BenchmarkId::from_parameter(name), &calldata, |b, c| {
            b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
                let start = std::time::Instant::now();
                let args = DecodeArgsBuilder::new()
                    .target(c.to_string())
                    .skip_resolving(true)
                    .build()
                    .expect("Failed to build DecodeArgs");
                let _ = decode(args).await;
                start.elapsed()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, test_decode);
criterion_main!(benches);
