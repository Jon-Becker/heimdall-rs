pub trait ToLocaleString {
    fn to_locale_string(&self) -> String;
}

impl ToLocaleString for usize {
    // add commas every 3 digits, e.g. 1000000 -> 1,000,000.
    fn to_locale_string(&self) -> String {
        let num_str = self.to_string();
        let mut result = String::new();
        let mut count = 0;

        for c in num_str.chars().rev() {
            if count != 0 && count % 3 == 0 {
                result.push(',');
            }
            result.push(c);
            count += 1;
        }

        result.chars().rev().collect()
    }
}
