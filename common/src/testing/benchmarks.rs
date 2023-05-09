use std::{time::Instant, thread, io, io::Write};

pub fn benchmark(benchmark_name: &str, runs: usize, to_bench: fn()) {
    let mut time = 0usize;
    let mut times = Vec::with_capacity(runs);
    let mut max = usize::MIN;
    let mut min = usize::MAX;

    // warm up
    thread::sleep(std::time::Duration::from_secs(2));

    for _ in 0..runs {
        let start_time = Instant::now();
        to_bench();
        let end_time = start_time.elapsed().as_micros() as usize;
        
        max = std::cmp::max(max, end_time);
        min = std::cmp::min(min, end_time);
        time += end_time;
        times.push(end_time);
    }

    let mean = time / runs;
    let variance = times
        .iter()
        .map(|x| {
            let x_i64 = *x as i64;
            let diff = x_i64 - mean as i64;
            diff * diff
        })
        .sum::<i64>()
        / (runs - 1) as i64;
    let std_dev = f64::sqrt(variance as f64);

    let _ = io::stdout().write_all(
        format!(
            "  {}:\n    {}μs ± {:.0}μs per run ( with {} runs ).\n\n",
            benchmark_name,
            mean,
            std_dev,
            runs
        ).as_bytes()
    );
}