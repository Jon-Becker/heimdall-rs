use std::{time::Instant, thread};

pub fn benchmark(benchmark_name: &str, runs: usize, to_bench: fn()) {
    let mut time = 0usize;
    let mut max = usize::MIN;
    let mut min = usize::MAX;

    // warm up
    thread::sleep(std::time::Duration::from_secs(2));

    for _ in 0..runs {
        let start_time = Instant::now();
        let _ = to_bench();
        let end_time = start_time.elapsed().as_millis() as usize;
        
        max = std::cmp::max(max, end_time);
        min = std::cmp::min(min, end_time);
        time += end_time;
    }

    println!(
        "{}: ~{}ms [{}ms - {}ms] with {} runs",
        benchmark_name,
        time / 25,
        min,
        max,
        runs
    );
}