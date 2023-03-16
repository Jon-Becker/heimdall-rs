pub fn calculate_eta(items_per_second: f64, items_remaining: usize) -> u128 {

    (items_remaining as f64 / items_per_second) as u128
}

pub fn format_eta(seconds_remaining: u128) -> String {
    let days = seconds_remaining / 86400;
    let hours = (seconds_remaining % 86400) / 3600;
    let minutes = (seconds_remaining % 3600) / 60;
    let seconds = seconds_remaining % 60;

    format!(
        "{}{}{}{}",
        if days > 0 { format!("{}d ", days) } else { String::new() },
        if hours > 0 { format!("{}h ", hours) } else { String::new() },
        if minutes > 0 { format!("{}m ", minutes) } else { String::new() },
        if seconds > 0 { format!("{}s ", seconds) } else { String::from("0s") },
    )
}