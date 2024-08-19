pub mod solidity;
pub mod yul;

/// The ToSolidity trait is used to convert a nested structure into a Solidity-like string.
pub trait ToSolidity {
    /// Converts the structure into a Solidity-like string.
    fn to_solidity(&self) -> String;
}

/// The ToYul trait is used to convert a nested structure into a Yul-like string.
pub trait ToYul {
    /// Converts the structure into a Yul-like string.
    fn to_yul(&self) -> String;
}
