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
        if days > 0 { format!("{days}d ") } else { String::new() },
        if hours > 0 { format!("{hours}h ") } else { String::new() },
        if minutes > 0 { format!("{minutes}m ") } else { String::new() },
        if seconds > 0 { format!("{seconds}s ") } else { String::from("0s") },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_eta() {
        assert_eq!(calculate_eta(2.5, 10), 4);
        assert_eq!(calculate_eta(0.5, 100), 200);
        assert_eq!(calculate_eta(1.0, 0), 0);
        assert_eq!(calculate_eta(0.0, 100), std::u128::MAX);
        assert_eq!(calculate_eta(10.0, std::usize::MAX), 1844674407370955264);
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(0), "0s");
        assert_eq!(format_eta(59), "59s ");
        assert_eq!(format_eta(60), "1m 0s");
        assert_eq!(format_eta(3600), "1h 0s");
        assert_eq!(format_eta(3665), "1h 1m 5s ");
        assert_eq!(format_eta(86400), "1d 0s");
        assert_eq!(format_eta(172800), "2d 0s");
        assert_eq!(format_eta(180065), "2d 2h 1m 5s ");
    }
}
