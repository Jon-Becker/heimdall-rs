use chrono::Local;

/// Calculate the ETA for a process based on the number of items processed per second
pub fn calculate_eta(items_per_second: f64, items_remaining: usize) -> u128 {
    (items_remaining as f64 / items_per_second) as u128
}

/// Format seconds into a human readable ETA
///
/// ## Example
/// ```
/// use heimdall_common::utils::time::format_eta;
///
/// let eta = format_eta(86400);
/// assert_eq!(eta, "1d 0s");
///
/// let eta = format_eta(86401);
/// assert_eq!(eta, "1d 1s");
/// ```
pub fn format_eta(seconds_remaining: u128) -> String {
    let days = seconds_remaining / 86400;
    let hours = (seconds_remaining % 86400) / 3600;
    let minutes = (seconds_remaining % 3600) / 60;
    let seconds = seconds_remaining % 60;

    format!(
        "{}{}{}{}",
        if days > 0 { format!("{days}d ") } else { String::new() },
        if hours > 0 { format!("{hours}h ") } else { String::new() },
        if minutes > 0 { format!("{minutes}m ") } else { String::new() },
        if seconds > 0 { format!("{seconds}s ") } else { String::from("0s") },
    )
}

/// Get the current timestamp in a pretty format
pub fn pretty_timestamp() -> String {
    let now = Local::now();
    now.format("%d-%m-%Y %H:%M:%S.%f").to_string()
}
