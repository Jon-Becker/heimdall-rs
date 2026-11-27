use std::fmt::Write as FmtWrite;

/// Decode a hex string into a bytearray
pub(crate) fn decode_hex(s: &str) -> Result<Vec<u8>, &'static str> {
    if !s.len().is_multiple_of(2) {
        return Err("hex input has an odd length");
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).ok_or("invalid hex digit")?;
            let low = (pair[1] as char).to_digit(16).ok_or("invalid hex digit")?;
            Ok((high * 16 + low) as u8)
        })
        .collect()
}

/// Encode a bytearray into a hex string
pub(crate) fn encode_hex(s: Vec<u8>) -> String {
    s.iter().fold(String::new(), |mut acc: String, b| {
        write!(acc, "{b:02x}").expect("unable to write");
        acc
    })
}

/// Prettify bytes into a human-readable format
pub(crate) fn prettify_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        let kb = bytes / 1024;
        format!("{kb} KB")
    } else if bytes < 1024 * 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        format!("{mb} MB")
    } else {
        let gb = bytes / (1024 * 1024 * 1024);
        format!("{gb} GB")
    }
}

#[cfg(test)]
mod tests {
    use crate::util::*;

    #[test]
    fn test_decode_hex_valid_hex() {
        let hex = "48656c6c6f20576f726c64"; // "Hello World" in hex
        let result = decode_hex(hex);
        assert_eq!(result, Ok(vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100]));
    }

    #[test]
    fn test_decode_hex_invalid_hex() {
        let hex = "48656c6c6f20576f726c4G"; // Invalid hex character 'G'
        let result = decode_hex(hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_hex() {
        let bytes = vec![72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100];
        let result = encode_hex(bytes);
        assert_eq!(result, "48656c6c6f20576f726c64");
    }

    #[test]
    fn test_prettify_bytes_less_than_1_kb() {
        let bytes = 500;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "500 B");
    }

    #[test]
    fn test_prettify_bytes_less_than_1_mb() {
        let bytes = 500_000;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "488 KB");
    }

    #[test]
    fn test_prettify_bytes_less_than_1_gb() {
        let bytes = 500_000_000;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "476 MB");
    }

    #[test]
    fn test_prettify_bytes_greater_than_1_gb() {
        let bytes = 5_000_000_000;
        let result = prettify_bytes(bytes);
        assert_eq!(result, "4 GB");
    }
}
