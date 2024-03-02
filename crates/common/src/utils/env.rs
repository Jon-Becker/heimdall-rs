pub fn set_env(key: &str, value: &str) {
    if std::env::var(key).is_err() {
        std::env::set_var(key, value);
    }
}

pub fn get_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}
