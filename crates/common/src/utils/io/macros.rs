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
