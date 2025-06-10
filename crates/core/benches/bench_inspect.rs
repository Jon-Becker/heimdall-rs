//! Benchmark for testing inspect functionality performance.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use heimdall_inspect::{inspect, InspectArgsBuilder};
use tokio::runtime::Runtime;

fn test_inspect(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_inspect");

    let txids = [
        ("simple", "0x37321f192623002fc4b398b90ea825c37f81e29526fd355cff93ef6962fc0fba"),
        ("complex", "0xa5f676d0ee4c23cc1ccb0b802be5aaead5827a3337c06e9da8b0a85dfa3e7dd5"),
    ];

    for (name, txid) in txids.into_iter() {
        group.sample_size(100);
        group.bench_with_input(BenchmarkId::from_parameter(name), &txid, |b, c| {
            b.to_async::<Runtime>(Runtime::new().unwrap()).iter(|| async {
                let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| {
                    println!("RPC_URL not set, skipping bench");
                    std::process::exit(0);
                });

                let start = std::time::Instant::now();
                let args = InspectArgsBuilder::new()
                    .target(c.to_string())
                    .rpc_url(rpc_url)
                    .skip_resolving(true)
                    .build()
                    .expect("Failed to build InspectArgs");
                let _ = inspect(args).await;
                start.elapsed()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, test_inspect);
criterion_main!(benches);
