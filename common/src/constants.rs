use fancy_regex::Regex;
use lazy_static::lazy_static;

lazy_static! {

    // The following regex is used to validate Ethereum addresses.
    pub static ref ADDRESS_REGEX: Regex = Regex::new(r"^(0x)?[0-9a-fA-F]{40}$").unwrap();

    // The following regex is used to validate Ethereum transaction hashes.
    pub static ref TRANSACTION_HASH_REGEX: Regex = Regex::new(r"^(0x)?[0-9a-fA-F]{64}$").unwrap();

    // The following regex is used to validate raw bytecode files as targets.
    // It also restricts the file to a maximum of ~24kb, the maximum size of a
    // contract on Ethereum.
    pub static ref BYTECODE_REGEX: Regex = Regex::new(r"^(0x)?[0-9a-fA-F]{0,50000}$").unwrap();

    // The following regex is used to reduce null byte prefixes
    pub static ref REDUCE_HEX_REGEX: Regex = Regex::new(r"^0x(00)*").unwrap();

    // The following regex is used as a search pattern for words
    pub static ref WORD_REGEX: Regex = Regex::new(r"0x[0-9a-fA-F]{0,64}").unwrap();

    // The following regex is used to find type castings
    pub static ref TYPE_CAST_REGEX: Regex = Regex::new(r"(address\(|string\(|bool\(|bytes(\d*)\(|uint(\d*)\(|int(\d*)\()(?!\))").unwrap();

    // The following regex is used to find memory length accesses
    pub static ref MEMLEN_REGEX: Regex = Regex::new(r"memory\[memory\[[0-9x]*\]\]").unwrap();

    // The following regex is used to find memory accesses
    pub static ref MEMORY_REGEX: Regex = Regex::new(r"memory\[\(?[0-9x]*\]").unwrap();

    // The following regex is used to find storage accesses
    pub static ref STORAGE_REGEX: Regex = Regex::new(r"storage\[\(?[0-9x]*\]").unwrap();
}
