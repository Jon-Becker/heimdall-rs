/// Creates a spinner with an INFO-level style for progress indicators.
///
/// This macro generates a progress style with a timestamp, an "INFO" label,
/// and a spinning animation character that can be used with the indicatif
/// crate's ProgressBar to show ongoing operations.
///
/// # Returns
///
/// * `ProgressStyle` - A styled progress indicator for info-level messages
#[macro_export]
macro_rules! info_spinner {
    () => {
        indicatif::ProgressStyle::with_template(&format!(
            "{}  {} {}",
            colored::Colorize::dimmed($crate::utils::time::pretty_timestamp().as_str()),
            colored::Colorize::bright_green(colored::Colorize::bold("INFO")),
            "{spinner}  {msg}"
        ))
        .expect("Failed to create spinner.")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    };
}

/// Creates a spinner with a DEBUG-level style for progress indicators.
///
/// This macro generates a progress style with a timestamp, a "DEBUG" label,
/// and a spinning animation character that can be used with the indicatif
/// crate's ProgressBar to show ongoing operations at debug level.
///
/// # Returns
///
/// * `ProgressStyle` - A styled progress indicator for debug-level messages
#[macro_export]
macro_rules! debug_spinner {
    () => {
        indicatif::ProgressStyle::with_template(&format!(
            "{} {} {}",
            colored::Colorize::dimmed($crate::utils::time::pretty_timestamp().as_str()),
            colored::Colorize::bright_purple(colored::Colorize::bold("DEBUG")),
            "{spinner}  {msg}"
        ))
        .expect("Failed to create spinner.")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    };
}
