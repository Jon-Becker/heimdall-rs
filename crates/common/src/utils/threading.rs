use crossbeam_channel::unbounded;
use eyre::Result;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

/// A simple thread pool implementation that takes a vector of items, splits them into chunks, and
/// processes each chunk in a separate thread. The results are collected and returned.
///
/// ```
/// use heimdall_common::utils::threading::task_pool;
///
/// let items = vec![1, 2, 3, 4, 5];
/// let num_threads = 2;
/// let mut results = task_pool(items, num_threads, |item| item * 2);
///
/// // sort
/// results.sort();
///
/// assert_eq!(results, vec![2, 4, 6, 8, 10]);
/// ```
pub fn task_pool<
    T: Clone + Send + Sync + 'static,
    R: Send + 'static,
    F: Fn(T) -> R + Send + Sync + 'static,
>(
    items: Vec<T>,
    num_threads: usize,
    f: F,
) -> Vec<R> {
    // if items is empty, return empty results
    if items.is_empty() {
        return Vec::new();
    }

    let (tx, rx) = unbounded();
    let mut handles = Vec::new();

    // Split items into chunks for each thread to process
    let chunk_size = (items.len() + num_threads - 1) / num_threads;
    let chunks = items.chunks(chunk_size);

    // Share ownership of f across threads with Arc
    let shared_f = Arc::new(f);

    for chunk in chunks {
        let chunk = chunk.to_owned();
        let tx = tx.clone();
        // Share ownership of shared_f with each thread with Arc
        let shared_f = Arc::clone(&shared_f);
        let handle = thread::spawn(move || {
            let chunk_results: Vec<R> = chunk.into_iter().map(|item| shared_f(item)).collect();
            let _ = tx.send(chunk_results);
        });
        handles.push(handle);
    }

    // Wait for all threads to finish and collect the results
    let mut results = Vec::new();
    for _ in 0..num_threads {
        let chunk_results = match rx.recv() {
            Ok(chunk_results) => chunk_results,
            Err(_) => continue,
        };
        results.extend(chunk_results);
    }

    // Wait for all threads to finish
    for handle in handles {
        if handle.join().is_ok() {}
    }

    results
}

/// Takes a function and some arguments, and runs the function in a separate thread. If the function
/// doesnt finish within the given timeout, the thread is killed, and the function returns None.
pub fn run_with_timeout<T, F>(f: F, timeout: Duration) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = unbounded();
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&flag);

    let handle = thread::spawn(move || {
        if !flag_clone.load(Ordering::Relaxed) {
            let result = f();
            let _ = tx.send(result);
        }
    });

    let result = rx.recv_timeout(timeout);
    if result.is_err() {
        flag.store(true, Ordering::Relaxed);
        return Err(eyre::eyre!("timed out"));
    }

    handle.join().ok();
    result.map_err(|e| eyre::eyre!(e))
}

#[cfg(test)]
mod tests {
    use crate::utils::threading::*;

    #[test]
    fn test_task_pool_with_single_thread() {
        // Test case with a single thread
        let items = vec![1, 2, 3, 4, 5];
        let num_threads = 1;
        let expected_results = vec![2, 4, 6, 8, 10];

        // Define a simple function to double the input
        let f = |x: i32| x * 2;

        let mut results = task_pool(items, num_threads, f);
        results.sort();
        assert_eq!(results, expected_results);
    }

    #[test]
    fn test_task_pool_with_multiple_threads() {
        // Test case with multiple threads
        let items = vec![1, 2, 3, 4, 5];
        let num_threads = 3;
        let expected_results = vec![2, 4, 6, 8, 10];

        // Define a simple function to double the input
        let f = |x: i32| x * 2;

        let mut results = task_pool(items, num_threads, f);
        results.sort();
        assert_eq!(results, expected_results);
    }

    #[test]
    fn test_task_pool_with_empty_items() {
        // Test case with empty items vector
        let items: Vec<i32> = Vec::new();
        let num_threads = 2;

        // Define a simple function to double the input
        let f = |x: i32| x * 2;

        let results = task_pool(items, num_threads, f);
        assert!(results.is_empty());
    }

    #[test]
    fn test_run_with_timeout() {
        // Test case with a function that finishes within the timeout
        let timeout = std::time::Duration::from_secs(1);
        let f = || 1;
        let result = run_with_timeout(f, timeout);
        assert_eq!(result.unwrap(), 1);

        // Test case with a function that doesnt finish within the timeout
        let timeout = std::time::Duration::from_millis(1);
        let f = || std::thread::sleep(std::time::Duration::from_secs(1));
        let result = run_with_timeout(f, timeout);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_with_timeout_with_panic() {
        // Test case with a function that panics
        let timeout = std::time::Duration::from_secs(1);
        let f = || panic!("test");
        let result = run_with_timeout(f, timeout);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_with_timeout_with_args() {
        // Test case with a function that takes arguments
        let timeout = std::time::Duration::from_secs(1);
        let f = |x: i32| x * 2;
        let result = run_with_timeout(move || f(2), timeout);
        assert_eq!(result.unwrap(), 4);
    }

    #[test]
    fn test_run_with_timeout_infinite_loop() {
        // Test case with a function that runs an infinite loop
        let timeout = std::time::Duration::from_secs(1);
        let f = || loop {};
        let result = run_with_timeout(f, timeout);
        assert!(result.is_err());
    }
}
