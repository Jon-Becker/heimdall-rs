use super::strings::encode_hex;
use alloy::primitives::{Address, Bytes, FixedBytes, U256};

/// A convenience trait which encodes a given EVM type into a sized, lowercase hex string.
pub trait ToLowerHex {
    /// Converts the value to a lowercase hexadecimal string representation.
    ///
    /// # Returns
    ///
    /// * `String` - The lowercase hexadecimal representation
    fn to_lower_hex(&self) -> String;
}

impl ToLowerHex for Bytes {
    fn to_lower_hex(&self) -> String {
        encode_hex(self)
    }
}

impl ToLowerHex for bytes::Bytes {
    fn to_lower_hex(&self) -> String {
        encode_hex(self)
    }
}

impl ToLowerHex for U256 {
    fn to_lower_hex(&self) -> String {
        encode_hex(&self.to_be_bytes_vec())
    }
}

impl ToLowerHex for FixedBytes<20> {
    fn to_lower_hex(&self) -> String {
        format!("{self:#020x}")
    }
}

impl ToLowerHex for Vec<u8> {
    fn to_lower_hex(&self) -> String {
        encode_hex(self)
    }
}

impl ToLowerHex for FixedBytes<32> {
    fn to_lower_hex(&self) -> String {
        format!("{self:#032x}")
    }
}

impl ToLowerHex for Address {
    fn to_lower_hex(&self) -> String {
        format!("{self:#020x}")
    }
}
