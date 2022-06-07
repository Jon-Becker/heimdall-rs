use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {

    // The following regex is used to validate Ethereum addresses.
    pub static ref ADDRESS_REGEX: Regex = Regex::new(r"^(0x)?[0-9a-fA-F]{40}$").unwrap();

    // The following regex is used to validate raw bytecode files as targets.
    // It also restricts the file to a maximum of ~24kb, the maximum size of a
    // contract on Ethereum.
    pub static ref BYTECODE_REGEX: Regex = Regex::new(r"^(0x)?[0-9a-fA-F]{50000}$").unwrap();
}