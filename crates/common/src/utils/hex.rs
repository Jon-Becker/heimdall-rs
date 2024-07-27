use super::strings::encode_hex;
use alloy::primitives::{Address, Bytes, FixedBytes, I256, U256};

/// A convenience function which encodes a given EVM type into a sized, lowercase hex string.
pub trait ToLowerHex {
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
        format!("{:#032x}", self)
    }
}

impl ToLowerHex for I256 {
    fn to_lower_hex(&self) -> String {
        format!("{:#032x}", self)
    }
}

impl ToLowerHex for FixedBytes<20> {
    fn to_lower_hex(&self) -> String {
        format!("{:#020x}", self)
    }
}

impl ToLowerHex for Vec<u8> {
    fn to_lower_hex(&self) -> String {
        encode_hex(self)
    }
}

impl ToLowerHex for FixedBytes<32> {
    fn to_lower_hex(&self) -> String {
        format!("{:#032x}", self)
    }
}

impl ToLowerHex for Address {
    fn to_lower_hex(&self) -> String {
        format!("{:#020x}", self)
    }
}
