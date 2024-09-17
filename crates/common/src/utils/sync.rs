/// Take in a non-async function and await it. This functions should be blocking.
pub fn blocking_await<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    tokio::task::block_in_place(f)
}
