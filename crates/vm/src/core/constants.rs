use std::str::FromStr;

use ethers::types::U256;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref COINBASE_ADDRESS: U256 =
        U256::from_str("0x6865696d64616c6c00000000636f696e62617365")
            .expect("failed to parse coinbase address");
    pub static ref CREATE_ADDRESS: U256 =
        U256::from_str("0x6865696d64616c6c000000000000637265617465")
            .expect("failed to parse create address");
    pub static ref CREATE2_ADDRESS: U256 =
        U256::from_str("0x6865696d64616c6c000000000063726561746532")
            .expect("failed to parse create2 address");
}
