//! Benchmark for testing decompile functionality performance.

#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_decompiler::{decompile, DecompilerArgsBuilder};
use tokio::runtime::Runtime;

fn test_decompile(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_decompiler");

    let contracts = [
        // 0x1bf797219482a29013d804ad96d1c6f84fba4c45
        ("simple", include_str!("../tests/testdata/bytecode/ecrecover.hex")),
        // 0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a
        ("complex", include_str!("../tests/testdata/bytecode/complex_nft.hex")),
    ];

    // output yul
    for (name, contract) in contracts.into_iter() {
        group.sample_size(500);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("yul_{}", name)),
            &contract,
            |b, c| {
                b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
                    let start = std::time::Instant::now();
                    let args = DecompilerArgsBuilder::new()
                        .target(c.to_string())
                        .skip_resolving(true)
                        .include_yul(true)
                        .build()
                        .expect("Failed to build DecompilerArgs");
                    let _ = decompile(args).await;
                    start.elapsed()
                });
            },
        );
    }

    // output sol
    for (name, contract) in contracts.into_iter() {
        group.sample_size(100);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("sol_{}", name)),
            &contract,
            |b, c| {
                b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
                    let start = std::time::Instant::now();
                    let args = DecompilerArgsBuilder::new()
                        .target(c.to_string())
                        .skip_resolving(true)
                        .include_solidity(true)
                        .build()
                        .expect("Failed to build DecompilerArgs");
                    let _ = decompile(args).await;
                    start.elapsed()
                });
            },
        );
    }

    // output abi
    for (name, contract) in contracts.into_iter() {
        group.sample_size(100);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("abi_{}", name)),
            &contract,
            |b, c| {
                b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
                    let start = std::time::Instant::now();
                    let args = DecompilerArgsBuilder::new()
                        .target(c.to_string())
                        .skip_resolving(true)
                        .build()
                        .expect("Failed to build DecompilerArgs");
                    let _ = decompile(args).await;
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, test_decompile);
criterion_main!(benches);
