#[macro_export]
macro_rules! info_spinner {
    () => {
        indicatif::ProgressStyle::with_template(&format!(
            "{}  {}: {}",
            colored::Colorize::dimmed($crate::utils::time::pretty_timestamp().as_str()),
            colored::Colorize::bright_cyan(colored::Colorize::bold("info")),
            "{spinner}  {msg}"
        ))
        .expect("Failed to create spinner.")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    };
}

#[macro_export]
macro_rules! debug_spinner {
    () => {
        indicatif::ProgressStyle::with_template(&format!(
            "{}  {}: {}",
            colored::Colorize::dimmed($crate::utils::time::pretty_timestamp().as_str()),
            colored::Colorize::bright_magenta(colored::Colorize::bold("debug")),
            "{spinner}  {msg}"
        ))
        .expect("Failed to create spinner.")
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
    };
}
