use std::{future::Future, pin::Pin};

/// Take in a non-async function and await it. This functions should be blocking.
pub fn blocking_await<F, T>(f: F) -> T
where
    F: FnOnce() -> T, {
    tokio::task::block_in_place(f)
}

/// A boxed future with a static lifetime.
///
/// This type alias is a convenience for returning a boxed future from a function.
/// The future is pinned and can be awaited.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
