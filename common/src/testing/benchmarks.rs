use std::{time::Instant, thread, io, io::Write};

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

    let _ = io::stdout().write_all(
        format!(
            "  {}:\n    {}ms ± {}ms per run ( with {} runs ).\n\n",
            benchmark_name,
            time / runs,
            std::cmp::max(max-(time / runs), (time / runs)-min),
            runs
        ).as_bytes()
    );
}