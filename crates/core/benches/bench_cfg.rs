//! Benchmark for testing CFG (Control Flow Graph) functionality performance.

#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_cfg::{cfg, CfgArgsBuilder};
use tokio::runtime::Runtime;

fn test_cfg(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_cfg");

    let contracts = [
        // 0x1bf797219482a29013d804ad96d1c6f84fba4c45
        ("simple", include_str!("../tests/testdata/bytecode/ecrecover.hex")),
        // 0xE90d8Fb7B79C8930B5C8891e61c298b412a6e81a
        ("complex", include_str!("../tests/testdata/bytecode/complex_nft.hex")),
    ];

    for (name, contract) in contracts.into_iter() {
        group.sample_size(500);
        group.bench_with_input(BenchmarkId::from_parameter(name), &contract, |b, c| {
            b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
                let start = std::time::Instant::now();
                let args = CfgArgsBuilder::new()
                    .target(c.to_string())
                    .build()
                    .expect("Failed to build CfgArgs");
                let _ = cfg(args).await;
                start.elapsed()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, test_cfg);
criterion_main!(benches);
