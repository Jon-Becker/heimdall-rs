// Although this macro is meant to internal use only, it cannot be declared internally
// due to how rust expand macros
#[macro_export]
macro_rules! log_helper {
    ($log_fn:ident, $message:expr) => {
        $crate::utils::io::logging::Logger::default().$log_fn($message);
    };
    ($log_fn:ident, $message:expr, $($arg:tt)*) => {
        $crate::utils::io::logging::Logger::default().$log_fn(&format!($message, $($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($message:expr) => {
        $crate::log_helper!(warn, $message);
    };
    ($message:expr, $($arg:tt)*) => {
        $crate::log_helper!(warn, $message, $($arg)*);
    };
}

#[macro_export]
macro_rules! debug_max {
    ($message:expr) => {
        $crate::log_helper!(debug_max, $message);
    };
    ($message:expr, $($arg:tt)*) => {
        $crate::log_helper!(debug_max, $message, $($arg)*)
    };
}

#[macro_export]
macro_rules! info {
    ($message:expr) => {
        $crate::log_helper!(info, $message);
    };
    ($message:expr, $($arg:tt)*) => {
        $crate::log_helper!(info, $message, $($arg)*)
    };
}

#[macro_export]
macro_rules! error {
    ($message:expr) => {
        $crate::log_helper!(error, $message);
    };
    ($message:expr, $($arg:tt)*) => {
        $crate::log_helper!(error, $message, $($arg)*)
    };
}

#[macro_export]
macro_rules! debug {
    ($message:expr) => {
        $crate::log_helper!(debug, $message);
    };
    ($message:expr, $($arg:tt)*) => {
        $crate::log_helper!(debug, $message, $($arg)*)
    };
}

#[macro_export]
macro_rules! trace {
    ($message:expr) => {
        $crate::log_helper!(trace, $message)
    };
    ($message:expr, $($arg:tt)*) => {
        $create::log_helper!(trace, $message, $($arg)*)
    };
}

#[cfg(test)]
mod test {
    #[test]
    fn test_logger() {
        std::env::set_var("RUST_LOG", "MAX");

        info!("hello");
        error!("hello");
        warn!("hello");
        trace!("hello");
        debug!("hello");
        info!("hello");
    }
}
