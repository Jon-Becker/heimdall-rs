use std::str::FromStr;

/// Ethereum hard forks in chronological order.
///
/// Each hard fork may introduce new opcodes or change existing behavior.
/// Opcodes are only valid if activated at or before the configured hard fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum HardFork {
    /// Initial Ethereum release (July 2015)
    Frontier = 0,
    /// First planned hard fork (March 2016)
    Homestead = 1,
    /// DAO fork response (July 2016)
    DaoFork = 2,
    /// First of Metropolis series (October 2017)
    Byzantium = 3,
    /// Second of Metropolis series (February 2019)
    Constantinople = 4,
    /// Constantinople bug fix (February 2019)
    Petersburg = 5,
    /// October 2019 fork
    Istanbul = 6,
    /// January 2020 fork
    MuirGlacier = 7,
    /// April 2021 fork
    Berlin = 8,
    /// August 2021 fork
    London = 9,
    /// December 2021 fork
    ArrowGlacier = 10,
    /// June 2022 fork
    GrayGlacier = 11,
    /// The Merge (September 2022)
    Paris = 12,
    /// April 2023 fork
    Shanghai = 13,
    /// March 2024 fork
    Cancun = 14,
    /// May 2025 fork (Prague-Electra)
    Pectra = 15,
    /// December 2025 fork (Fulu-Osaka)
    Fusaka = 16,
    /// Automatically detect hardfork based on contract creation block
    Auto = 254,
    /// Latest hard fork
    #[default]
    Latest = 255,
}

impl HardFork {
    /// Returns the effective hard fork, resolving `Latest` and `Auto` to the actual latest fork.
    #[inline]
    pub const fn effective(self) -> Self {
        match self {
            Self::Latest | Self::Auto => Self::Fusaka,
            other => other,
        }
    }

    /// Returns true if `self` is at or after `other`.
    #[inline]
    pub const fn is_active(self, other: Self) -> bool {
        self.effective() as u8 >= other as u8
    }

    /// Returns the active hard fork for a given chain ID and block number.
    ///
    /// For post-merge forks, the `timestamp` parameter should be
    /// provided as these forks activate based on timestamp rather than block number.
    ///
    /// Returns `HardFork::Latest` for unknown chains (assumes all opcodes are active).
    pub fn from_chain(chain_id: u64, block_number: u64, timestamp: Option<u64>) -> Self {
        match chain_id {
            1 => Self::from_mainnet(block_number, timestamp),
            11155111 => Self::from_sepolia(block_number, timestamp),
            17000 => Self::from_holesky(block_number, timestamp),
            _ => Self::Latest,
        }
    }

    /// Returns the active hard fork for Ethereum mainnet.
    fn from_mainnet(block_number: u64, timestamp: Option<u64>) -> Self {
        // Post-merge forks use timestamp
        if let Some(ts) = timestamp {
            if ts >= 1764798551 {
                return Self::Fusaka;
            }
            if ts >= 1746612311 {
                return Self::Pectra;
            }
            if ts >= 1710338135 {
                return Self::Cancun;
            }
            if ts >= 1681338455 {
                return Self::Shanghai;
            }
        }

        // Pre-merge and merge forks use block number
        match block_number {
            0..=1_149_999 => Self::Frontier,
            1_150_000..=1_919_999 => Self::Homestead,
            1_920_000..=4_369_999 => Self::DaoFork,
            4_370_000..=7_279_999 => Self::Byzantium,
            // Constantinople and Petersburg activated at same block
            7_280_000..=9_068_999 => Self::Constantinople,
            9_069_000..=9_199_999 => Self::Istanbul,
            9_200_000..=12_243_999 => Self::MuirGlacier,
            12_244_000..=12_964_999 => Self::Berlin,
            12_965_000..=13_772_999 => Self::London,
            13_773_000..=15_049_999 => Self::ArrowGlacier,
            15_050_000..=15_537_393 => Self::GrayGlacier,
            // Paris (The Merge) - block 15,537,394
            _ => Self::Paris,
        }
    }

    /// Returns the active hard fork for Sepolia testnet.
    fn from_sepolia(block_number: u64, timestamp: Option<u64>) -> Self {
        // Sepolia launched post-London, so earlier forks are at block 0
        // Post-merge forks use timestamp
        if let Some(ts) = timestamp {
            if ts >= 1760427360 {
                return Self::Fusaka;
            }
            if ts >= 1741159776 {
                return Self::Pectra;
            }
            if ts >= 1706655072 {
                return Self::Cancun;
            }
            if ts >= 1677557088 {
                return Self::Shanghai;
            }
        }

        // Sepolia merge happened around block 1,450,409
        if block_number >= 1_450_409 {
            Self::Paris
        } else {
            // Pre-merge Sepolia had London-equivalent opcodes
            Self::London
        }
    }

    /// Returns the active hard fork for Holesky testnet.
    fn from_holesky(block_number: u64, timestamp: Option<u64>) -> Self {
        // Holesky launched post-merge (September 2023)
        // Post-merge forks use timestamp
        if let Some(ts) = timestamp {
            if ts >= 1759308480 {
                return Self::Fusaka;
            }
            if ts >= 1740434112 {
                return Self::Pectra;
            }
            if ts >= 1707305664 {
                return Self::Cancun;
            }
            if ts >= 1696000704 {
                return Self::Shanghai;
            }
        }

        // Holesky launched with Shanghai-equivalent state
        let _ = block_number;
        Self::Shanghai
    }
}

impl FromStr for HardFork {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "frontier" => Ok(Self::Frontier),
            "homestead" => Ok(Self::Homestead),
            "daofork" | "dao" => Ok(Self::DaoFork),
            "byzantium" => Ok(Self::Byzantium),
            "constantinople" => Ok(Self::Constantinople),
            "petersburg" => Ok(Self::Petersburg),
            "istanbul" => Ok(Self::Istanbul),
            "muirglacier" | "muir_glacier" => Ok(Self::MuirGlacier),
            "berlin" => Ok(Self::Berlin),
            "london" => Ok(Self::London),
            "arrowglacier" | "arrow_glacier" => Ok(Self::ArrowGlacier),
            "grayglacier" | "gray_glacier" => Ok(Self::GrayGlacier),
            "paris" | "merge" => Ok(Self::Paris),
            "shanghai" => Ok(Self::Shanghai),
            "cancun" => Ok(Self::Cancun),
            "pectra" => Ok(Self::Pectra),
            "fusaka" => Ok(Self::Fusaka),
            "auto" => Ok(Self::Auto),
            "latest" => Ok(Self::Latest),
            _ => Err(format!("unknown hardfork: {}", s)),
        }
    }
}

impl std::fmt::Display for HardFork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frontier => write!(f, "frontier"),
            Self::Homestead => write!(f, "homestead"),
            Self::DaoFork => write!(f, "daofork"),
            Self::Byzantium => write!(f, "byzantium"),
            Self::Constantinople => write!(f, "constantinople"),
            Self::Petersburg => write!(f, "petersburg"),
            Self::Istanbul => write!(f, "istanbul"),
            Self::MuirGlacier => write!(f, "muirglacier"),
            Self::Berlin => write!(f, "berlin"),
            Self::London => write!(f, "london"),
            Self::ArrowGlacier => write!(f, "arrowglacier"),
            Self::GrayGlacier => write!(f, "grayglacier"),
            Self::Paris => write!(f, "paris"),
            Self::Shanghai => write!(f, "shanghai"),
            Self::Cancun => write!(f, "cancun"),
            Self::Pectra => write!(f, "pectra"),
            Self::Fusaka => write!(f, "fusaka"),
            Self::Auto => write!(f, "auto"),
            Self::Latest => write!(f, "latest"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::chains;

    #[test]
    fn test_mainnet_frontier() {
        assert_eq!(HardFork::from_chain(1, 0, None), HardFork::Frontier);
        assert_eq!(HardFork::from_chain(1, 1_000_000, None), HardFork::Frontier);
    }

    #[test]
    fn test_mainnet_homestead() {
        assert_eq!(HardFork::from_chain(1, 1_150_000, None), HardFork::Homestead);
        assert_eq!(HardFork::from_chain(1, 1_500_000, None), HardFork::Homestead);
    }

    #[test]
    fn test_mainnet_byzantium() {
        assert_eq!(HardFork::from_chain(1, 4_370_000, None), HardFork::Byzantium);
        assert_eq!(HardFork::from_chain(1, 5_000_000, None), HardFork::Byzantium);
    }

    #[test]
    fn test_mainnet_constantinople() {
        assert_eq!(HardFork::from_chain(1, 7_280_000, None), HardFork::Constantinople);
    }

    #[test]
    fn test_mainnet_istanbul() {
        assert_eq!(HardFork::from_chain(1, 9_069_000, None), HardFork::Istanbul);
    }

    #[test]
    fn test_mainnet_london() {
        assert_eq!(HardFork::from_chain(1, 12_965_000, None), HardFork::London);
    }

    #[test]
    fn test_mainnet_paris() {
        assert_eq!(HardFork::from_chain(1, 15_537_394, None), HardFork::Paris);
        assert_eq!(HardFork::from_chain(1, 16_000_000, None), HardFork::Paris);
    }

    #[test]
    fn test_mainnet_shanghai() {
        // Shanghai activated at timestamp 1681338455
        assert_eq!(HardFork::from_chain(1, 17_000_000, Some(1681338455)), HardFork::Shanghai);
        assert_eq!(HardFork::from_chain(1, 17_000_000, Some(1700000000)), HardFork::Shanghai);
    }

    #[test]
    fn test_mainnet_cancun() {
        // Cancun activated at timestamp 1710338135
        assert_eq!(HardFork::from_chain(1, 19_000_000, Some(1710338135)), HardFork::Cancun);
        assert_eq!(HardFork::from_chain(1, 20_000_000, Some(1720000000)), HardFork::Cancun);
    }

    #[test]
    fn test_mainnet_pectra() {
        // Pectra activated at timestamp 1746612311 (May 7, 2025)
        assert_eq!(HardFork::from_chain(1, 22_000_000, Some(1746612311)), HardFork::Pectra);
        assert_eq!(HardFork::from_chain(1, 22_000_000, Some(1750000000)), HardFork::Pectra);
    }

    #[test]
    fn test_mainnet_fusaka() {
        // Fusaka activated at timestamp 1764798551 (December 3, 2025)
        assert_eq!(HardFork::from_chain(1, 23_000_000, Some(1764798551)), HardFork::Fusaka);
        assert_eq!(HardFork::from_chain(1, 24_000_000, Some(1800000000)), HardFork::Fusaka);
    }

    #[test]
    fn test_sepolia() {
        // Pre-merge Sepolia
        assert_eq!(HardFork::from_chain(11155111, 1_000_000, None), HardFork::London);
        // Post-merge Sepolia
        assert_eq!(HardFork::from_chain(11155111, 2_000_000, None), HardFork::Paris);
        // Shanghai
        assert_eq!(HardFork::from_chain(11155111, 3_000_000, Some(1677557088)), HardFork::Shanghai);
        // Cancun
        assert_eq!(HardFork::from_chain(11155111, 5_000_000, Some(1706655072)), HardFork::Cancun);
        // Pectra
        assert_eq!(HardFork::from_chain(11155111, 7_000_000, Some(1741159776)), HardFork::Pectra);
        // Fusaka
        assert_eq!(HardFork::from_chain(11155111, 8_000_000, Some(1760427360)), HardFork::Fusaka);
    }

    #[test]
    fn test_unknown_chain_returns_latest() {
        // Unknown chains default to Latest
        assert_eq!(HardFork::from_chain(999999, 0, None), HardFork::Latest);
        // L2s default to Latest
        assert_eq!(HardFork::from_chain(chains::POLYGON, 0, None), HardFork::Latest);
        assert_eq!(HardFork::from_chain(chains::ARBITRUM, 0, None), HardFork::Latest);
        assert_eq!(HardFork::from_chain(chains::OPTIMISM, 0, None), HardFork::Latest);
        assert_eq!(HardFork::from_chain(chains::BASE, 0, None), HardFork::Latest);
    }

    #[test]
    fn test_hardfork_ordering() {
        // Test that hardfork ordering is correct
        assert!(HardFork::Fusaka.is_active(HardFork::Pectra));
        assert!(HardFork::Pectra.is_active(HardFork::Cancun));
        assert!(HardFork::Cancun.is_active(HardFork::Shanghai));
        assert!(HardFork::Shanghai.is_active(HardFork::London));
        assert!(HardFork::London.is_active(HardFork::Istanbul));
        assert!(HardFork::Istanbul.is_active(HardFork::Constantinople));
        assert!(HardFork::Constantinople.is_active(HardFork::Byzantium));
        assert!(HardFork::Byzantium.is_active(HardFork::Homestead));
        assert!(HardFork::Homestead.is_active(HardFork::Frontier));

        // Earlier forks should not activate later opcodes
        assert!(!HardFork::Frontier.is_active(HardFork::Homestead));
        assert!(!HardFork::Byzantium.is_active(HardFork::Constantinople));
        assert!(!HardFork::Cancun.is_active(HardFork::Pectra));
        assert!(!HardFork::Pectra.is_active(HardFork::Fusaka));
    }
}
