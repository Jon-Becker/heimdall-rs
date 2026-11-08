use std::{sync::Arc, thread};

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
    if items.is_empty() {
        return Vec::new();
    }
    assert!(num_threads > 0, "num_threads must be greater than zero");
    let chunk_size = items.len().div_ceil(num_threads);
    let shared_f = Arc::new(f);
    let handles: Vec<_> = items
        .chunks(chunk_size)
        .map(|chunk| {
            let chunk = chunk.to_owned();
            let shared_f = Arc::clone(&shared_f);
            thread::spawn(move || chunk.into_iter().map(|item| shared_f(item)).collect::<Vec<R>>())
        })
        .collect();

    handles
        .into_iter()
        .flat_map(|handle| match handle.join() {
            Ok(results) => results,
            Err(panic) => std::panic::resume_unwind(panic),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::task_pool;

    #[test]
    fn handles_fewer_chunks_than_requested_threads() {
        assert_eq!(task_pool(vec![1, 2, 3, 4, 5], 4, |n| n * 2), vec![2, 4, 6, 8, 10]);
        assert_eq!(task_pool(vec![1, 2], 10, |n| n), vec![1, 2]);
        assert!(task_pool(Vec::<i32>::new(), 2, |n| n).is_empty());
    }

    #[test]
    #[should_panic(expected = "worker failed")]
    fn propagates_worker_panics() {
        task_pool(vec![1, 2], 2, |_| -> i32 { panic!("worker failed") });
    }
}
