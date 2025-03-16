/// Trait for formatting numbers with locale-specific formatting.
///
/// This trait adds methods to format numbers in a more human-readable way,
/// such as adding thousands separators.
pub trait ToLocaleString {
    /// Formats a number with locale-specific formatting.
    ///
    /// For numbers, this adds commas as thousand separators.
    ///
    /// # Returns
    ///
    /// * `String` - The formatted string
    fn to_locale_string(&self) -> String;
}

impl ToLocaleString for usize {
    /// Add commas every 3 digits, e.g. 1000000 -> 1,000,000.
    ///
    /// ```
    /// use heimdall_common::utils::integers::ToLocaleString;
    ///
    /// assert_eq!(1000000.to_locale_string(), "1,000,000");
    /// ```
    fn to_locale_string(&self) -> String {
        let num_str = self.to_string();
        let mut result = String::new();

        for (count, c) in num_str.chars().rev().enumerate() {
            if count != 0 && count % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }

        result.chars().rev().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::integers::ToLocaleString;

    #[test]
    fn test_to_locale_string() {
        // Test case: Single-digit number
        let num = 5;
        let expected = "5".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Three-digit number
        let num = 123;
        let expected = "123".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Four-digit number
        let num = 1234;
        let expected = "1,234".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Five-digit number
        let num = 12345;
        let expected = "12,345".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Six-digit number
        let num = 123456;
        let expected = "123,456".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Seven-digit number
        let num = 1234567;
        let expected = "1,234,567".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Eight-digit number
        let num = 12345678;
        let expected = "12,345,678".to_string();
        assert_eq!(num.to_locale_string(), expected);

        // Test case: Nine-digit number
        let num = 123456789;
        let expected = "123,456,789".to_string();
        assert_eq!(num.to_locale_string(), expected);
    }
}
