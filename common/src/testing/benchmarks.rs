use rayon::prelude::*;
use std::{
    io::{self, Write},
    thread,
    time::Instant,
};

pub fn benchmark(benchmark_name: &str, runs: usize, to_bench: fn()) {
    // warm up
    thread::sleep(std::time::Duration::from_secs(2));

    let results = (0..runs)
        .into_par_iter()
        .map(|_| {
            let start_time = Instant::now();
            let _ = to_bench();
            let end_time = start_time.elapsed().as_millis() as usize;
            (end_time, end_time, end_time)
        })
        .reduce(
            || (0, usize::MIN, usize::MAX),
            |acc, x| {
                (
                    acc.0 + x.0,
                    std::cmp::max(acc.1, x.1),
                    std::cmp::min(acc.2, x.2),
                )
            },
        );

    let _ = io::stdout().write_all(
        format!(
            "  {}:\n    {}ms ± {}ms per run ( with {} runs ).\n\n",
            benchmark_name,
            results.0 / runs,
            std::cmp::max(
                results.1 - (results.0 / runs),
                (results.0 / runs) - results.2
            ),
            runs
        )
        .as_bytes(),
    );
}
