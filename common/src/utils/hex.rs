use ethers::types::{Bloom, Bytes, H160, H256, H64, U256};

use super::strings::encode_hex;

pub trait ToLowerHex {
    fn to_lower_hex(&self) -> String;
}

impl ToLowerHex for H256 {
    fn to_lower_hex(&self) -> String {
        format!("{:#032x}", self)
    }
}

impl ToLowerHex for U256 {
    fn to_lower_hex(&self) -> String {
        format!("{:#0x}", self)
    }
}

impl ToLowerHex for H160 {
    fn to_lower_hex(&self) -> String {
        format!("{:#020x}", self)
    }
}

impl ToLowerHex for H64 {
    fn to_lower_hex(&self) -> String {
        format!("{:#016x}", self)
    }
}

impl ToLowerHex for Bloom {
    fn to_lower_hex(&self) -> String {
        format!("{:#064x}", self)
    }
}

impl ToLowerHex for Bytes {
    fn to_lower_hex(&self) -> String {
        format!("{:#0x}", self)
    }
}

impl ToLowerHex for Vec<u8> {
    fn to_lower_hex(&self) -> String {
        encode_hex(self.to_vec())
    }
}
