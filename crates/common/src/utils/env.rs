/// Sets an environment variable if it's not already set.
///
/// # Arguments
///
/// * `key` - The environment variable name
/// * `value` - The value to set
pub fn set_env(key: &str, value: &str) {
    if std::env::var(key).is_err() {
        std::env::set_var(key, value);
    }
}

/// Gets the value of an environment variable.
///
/// # Arguments
///
/// * `key` - The environment variable name to retrieve
///
/// # Returns
///
/// * `Option<String>` - The environment variable value if it exists
pub fn get_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}
