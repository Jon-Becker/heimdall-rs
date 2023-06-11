pub trait ToLocaleString {
    fn to_locale_string(&self) -> String;
}

impl ToLocaleString for usize {
    // add commas every 3 digits, e.g. 1000000 -> 1,000,000.
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
