use fancy_regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    /// The following regex is used to extract constructor bytecode information
    pub static ref CONSTRUCTOR_REGEX: Regex = Regex::new(r"(?:5b)?(?:60([a-f0-9]{2})|61([a-f0-9_]{4})|62([a-f0-9_]{6}))80(?:60([a-f0-9]{2})|61([a-f0-9_]{4})|62([a-f0-9_]{6}))6000396000f3fe").expect("failed to compile regex");

    /// The following regex is used as a search pattern for words
    pub static ref WORD_REGEX: Regex = Regex::new(r"0x[0-9a-fA-F]{0,64}").expect("failed to compile regex");

    /// The following regex is used to find type castings
    pub static ref TYPE_CAST_REGEX: Regex = Regex::new(r"(address\(|string\(|bool\(|bytes(\d*)\(|uint(\d*)\(|int(\d*)\()(?!\))").expect("failed to compile regex");

    /// The following regex is used to find memory length accesses
    pub static ref MEMLEN_REGEX: Regex = Regex::new(r"memory\[memory\[[0-9x]*\]\]").expect("failed to compile regex");

    /// The following regex is used to find memory accesses
    pub static ref MEMORY_REGEX: Regex = Regex::new(r"memory\[\(?[0-9x]*\]").expect("failed to compile regex");

    /// The following regex is used to find storage accesses
    pub static ref STORAGE_REGEX: Regex = Regex::new(r"storage\[\(?[0-9x]*\]").expect("failed to compile regex");

    /// The following regex is used to find bitwise & operations
    pub static ref AND_BITMASK_REGEX: Regex =
        Regex::new(r"\(0x([a-fA-F0-9]{2}){1,32}\) & ").expect("failed to compile regex");

    /// The following regex is used to find bitwise & operations
    pub static ref AND_BITMASK_REGEX_2: Regex =
        Regex::new(r" & \(0x([a-fA-F0-9]{2}){1,32}\)").expect("failed to compile regex");

    /// The following regex is used to find non-zero bytes
    pub static ref NON_ZERO_BYTE_REGEX: Regex =
        Regex::new(r"[a-fA-F0-9][a-fA-F1-9]").expect("failed to compile regex");

    /// The following regex is used to find division by one
    pub static ref DIV_BY_ONE_REGEX: Regex =
        Regex::new(r" \/ 0x01(?!\d)").expect("failed to compile regex");

    /// The following regex is used to find multiplication by one
    pub static ref MUL_BY_ONE_REGEX: Regex =
        Regex::new(r"\b0x01\b\s*\*\s*| \*\s*\b0x01\b").expect("failed to compile regex");

    /// The following regex is used to find enclosed expressions (in parentheses)
    pub static ref ENCLOSED_EXPRESSION_REGEX: Regex =
        Regex::new(r"\(.*\)").expect("failed to compile regex");
}
