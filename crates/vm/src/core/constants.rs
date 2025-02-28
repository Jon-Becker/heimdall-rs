use alloy::primitives::U256;
use std::str::FromStr;

use lazy_static::lazy_static;

lazy_static! {
    /// The address used for the coinbase in EVM execution.
    ///
    /// In the Ethereum context, this would typically be the address of the miner/validator
    /// who receives the block reward. In Heimdall, this is a constant value used for
    /// consistency in simulation.
    pub static ref COINBASE_ADDRESS: U256 =
        U256::from_str("0x6865696d64616c6c00000000636f696e62617365")
            .expect("failed to parse coinbase address");

    /// The address used for standard contract creation (CREATE opcode).
    ///
    /// This is a constant used when simulating the CREATE opcode's behavior
    /// in contract deployment scenarios.
    pub static ref CREATE_ADDRESS: U256 =
        U256::from_str("0x6865696d64616c6c000000000000637265617465")
            .expect("failed to parse create address");

    /// The address used for CREATE2 contract creation.
    ///
    /// This is a constant used when simulating the CREATE2 opcode's behavior,
    /// which allows for deterministic contract addresses based on deployment parameters.
    pub static ref CREATE2_ADDRESS: U256 =
        U256::from_str("0x6865696d64616c6c000000000063726561746532")
            .expect("failed to parse create2 address");
}
