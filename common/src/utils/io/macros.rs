#[macro_export]
macro_rules! debug_max {
    ($message:expr) => {
        heimdall_common::utils::io::logging::Logger::default().debug_max($message);
    };
    ($message:expr, $($arg:tt)*) => {
        heimdall_common::utils::io::logging::Logger::default().debug_max(&format!($message, $($arg)*));
    };
}
