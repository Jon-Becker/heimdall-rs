#[cfg(test)]
mod tests {
    use std::thread;

    use crate::testing::benchmarks::benchmark;

    #[test]
    fn test_benchmark() {
        // Test case: Single run
        let benchmark_name = "Test Benchmark";
        let runs = 10;
        let to_bench = || {
            // Code to benchmark
            thread::sleep(std::time::Duration::from_millis(200));
        };
        benchmark(benchmark_name, runs, to_bench);
    }
}
