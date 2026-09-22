//! Benchmarks for string extraction from deployed contract bytecode.

#![allow(missing_docs)]

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use heimdall_common::{ether::bytecode::write_strings, utils::strings::decode_hex};

fn bench_strings(c: &mut Criterion) {
    let mut group = c.benchmark_group("heimdall_strings");
    let contracts = [
        ("dai", include_str!("../tests/testdata/strings/dai.hex")),
        (
            "uniswap_v2_usdc_weth",
            include_str!("../tests/testdata/strings/uniswap_v2_usdc_weth.hex"),
        ),
    ];

    for (name, hex) in contracts {
        let bytecode = decode_hex(hex).expect("invalid fixture bytecode");
        group.throughput(Throughput::Bytes(bytecode.len() as u64));

        for (mode, full_scan) in [("push_only", false), ("full_scan", true)] {
            // Reuse enough space for every input byte plus a final newline.
            let mut output = Vec::with_capacity(bytecode.len() + 1);
            group.bench_with_input(BenchmarkId::new(mode, name), &bytecode, |b, bytecode| {
                b.iter(|| {
                    output.clear();
                    write_strings(
                        black_box(bytecode.as_slice()),
                        black_box(4),
                        black_box(full_scan),
                        &mut output,
                    )
                    .expect("failed to extract strings");
                    black_box(output.as_slice());
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_strings);
criterion_main!(benches);
