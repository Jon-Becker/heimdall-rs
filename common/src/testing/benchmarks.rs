use std::{io, io::Write, thread, time::Instant};

#[allow(dead_code)]
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
        let end_time = start_time.elapsed().as_nanos() as usize;

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
        .sum::<i64>() /
        (runs - 1) as i64;
    let std_dev = f64::sqrt(variance as f64) as usize;

    let _ = io::stdout().write_all(
        format!(
            "  {}:\n    {} ± {} per run ( with {} runs ).\n\n",
            benchmark_name,
            format_nanos(mean),
            format_nanos(std_dev),
            runs
        )
        .as_bytes(),
    );
}

#[allow(dead_code)]
fn format_nanos(nanos: usize) -> String {
    let mut nanos = nanos;
    let mut micros = 0;
    let mut millis = 0;
    let mut secs = 0;
    let mut mins = 0;
    let mut hours = 0;

    if nanos >= 1000 {
        micros = nanos / 1000;
        nanos %= 1000;
    }

    if micros >= 1000 {
        millis = micros / 1000;
        micros %= 1000;
    }

    if millis >= 1000 {
        secs = millis / 1000;
        millis %= 1000;
    }

    if secs >= 60 {
        mins = secs / 60;
        secs %= 60;
    }

    if mins >= 60 {
        hours = mins / 60;
        mins %= 60;
    }

    let mut result = String::new();

    if hours > 0 {
        result.push_str(&format!("{}h ", hours));
    }

    if mins > 0 {
        result.push_str(&format!("{}m ", mins));
    }

    if secs > 0 {
        result.push_str(&format!("{}s ", secs));
    }

    if millis > 0 {
        result.push_str(&format!("{}ms ", millis));
    }

    if micros > 0 {
        result.push_str(&format!("{}μs ", micros));
    }

    if nanos > 0 {
        result.push_str(&format!("{}ns", nanos));
    }

    result
}
