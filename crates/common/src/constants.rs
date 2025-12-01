use fancy_regex::Regex;
use lazy_static::lazy_static;

// Etherscan V2 API supported chain IDs
// Full list at: https://docs.etherscan.io/etherscan-v2/getting-started/supported-chains

/// Ethereum Mainnet chain ID
pub const CHAIN_ID_ETHEREUM: u64 = 1;
/// Sepolia testnet chain ID
pub const CHAIN_ID_SEPOLIA: u64 = 11155111;
/// Holesky testnet chain ID
pub const CHAIN_ID_HOLESKY: u64 = 17000;
/// Polygon Mainnet chain ID
pub const CHAIN_ID_POLYGON: u64 = 137;
/// Polygon Amoy testnet chain ID
pub const CHAIN_ID_POLYGON_AMOY: u64 = 80002;
/// BSC Mainnet chain ID
pub const CHAIN_ID_BSC: u64 = 56;
/// BSC Testnet chain ID
pub const CHAIN_ID_BSC_TESTNET: u64 = 97;
/// Arbitrum One chain ID
pub const CHAIN_ID_ARBITRUM: u64 = 42161;
/// Arbitrum Sepolia testnet chain ID
pub const CHAIN_ID_ARBITRUM_SEPOLIA: u64 = 421614;
/// Optimism chain ID
pub const CHAIN_ID_OPTIMISM: u64 = 10;
/// Optimism Sepolia testnet chain ID
pub const CHAIN_ID_OPTIMISM_SEPOLIA: u64 = 11155420;
/// Avalanche C-Chain chain ID
pub const CHAIN_ID_AVALANCHE: u64 = 43114;
/// Avalanche Fuji testnet chain ID
pub const CHAIN_ID_AVALANCHE_FUJI: u64 = 43113;
/// Fantom Opera chain ID
pub const CHAIN_ID_FANTOM: u64 = 250;
/// Base chain ID
pub const CHAIN_ID_BASE: u64 = 8453;
/// Base Sepolia testnet chain ID
pub const CHAIN_ID_BASE_SEPOLIA: u64 = 84532;
/// Linea chain ID
pub const CHAIN_ID_LINEA: u64 = 59144;
/// Scroll chain ID
pub const CHAIN_ID_SCROLL: u64 = 534352;
/// zkSync Era chain ID
pub const CHAIN_ID_ZKSYNC: u64 = 324;
/// Polygon zkEVM chain ID
pub const CHAIN_ID_POLYGON_ZKEVM: u64 = 1101;

/// List of all Etherscan V2 API supported chain IDs
pub const ETHERSCAN_SUPPORTED_CHAIN_IDS: [u64; 20] = [
    CHAIN_ID_ETHEREUM,
    CHAIN_ID_SEPOLIA,
    CHAIN_ID_HOLESKY,
    CHAIN_ID_POLYGON,
    CHAIN_ID_POLYGON_AMOY,
    CHAIN_ID_BSC,
    CHAIN_ID_BSC_TESTNET,
    CHAIN_ID_ARBITRUM,
    CHAIN_ID_ARBITRUM_SEPOLIA,
    CHAIN_ID_OPTIMISM,
    CHAIN_ID_OPTIMISM_SEPOLIA,
    CHAIN_ID_AVALANCHE,
    CHAIN_ID_AVALANCHE_FUJI,
    CHAIN_ID_FANTOM,
    CHAIN_ID_BASE,
    CHAIN_ID_BASE_SEPOLIA,
    CHAIN_ID_LINEA,
    CHAIN_ID_SCROLL,
    CHAIN_ID_ZKSYNC,
    CHAIN_ID_POLYGON_ZKEVM,
];

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
