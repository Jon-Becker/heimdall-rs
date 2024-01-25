#[macro_export]
macro_rules! debug_max {
    ($message:expr) => {
        $crate::utils::io::logging::Logger::default().debug($message);
    };
    ($message:expr, $($arg:tt)*) => {
        $crate::utils::io::logging::Logger::default().debug(&format!($message, $($arg)*))
    };
}
