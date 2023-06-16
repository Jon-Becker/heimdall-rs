use crossbeam_channel::unbounded;
use std::{sync::Arc, thread};

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
        return Vec::new()
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
            tx.send(chunk_results).unwrap();
        });
        handles.push(handle);
    }

    // Wait for all threads to finish and collect the results
    let mut results = Vec::new();
    for _ in 0..num_threads {
        let chunk_results = rx.recv().unwrap();
        results.extend(chunk_results);
    }

    // Wait for all threads to finish
    for handle in handles {
        if handle.join().is_ok() {}
    }

    results
}
