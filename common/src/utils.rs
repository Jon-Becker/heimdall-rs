
use ethers::{prelude::{I256, U256}};

// Convert an unsigned integer into a signed one
pub fn sign_uint(unsigned: U256) -> I256 {
    return I256::from_raw(U256::from(unsigned))
}