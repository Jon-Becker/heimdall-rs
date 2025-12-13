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
    /// DAO fork response (July 2016) - no opcode changes
    DaoFork = 2,
    /// First of Metropolis series (October 2017)
    Byzantium = 3,
    /// Second of Metropolis series (February 2019)
    Constantinople = 4,
    /// Constantinople bug fix (February 2019)
    Petersburg = 5,
    /// October 2019 fork
    Istanbul = 6,
    /// January 2020 fork - no opcode changes
    MuirGlacier = 7,
    /// December 2020 fork - no opcode changes
    Berlin = 8,
    /// August 2021 fork
    London = 9,
    /// December 2021 fork - no opcode changes
    ArrowGlacier = 10,
    /// June 2022 fork - no opcode changes
    GrayGlacier = 11,
    /// The Merge (September 2022) - no opcode changes
    Paris = 12,
    /// March 2023 fork
    Shanghai = 13,
    /// March 2024 fork
    Cancun = 14,
    /// Latest hard fork (default)
    #[default]
    Latest = 255,
}

impl HardFork {
    /// Returns the effective hard fork, resolving `Latest` to the actual latest fork.
    #[inline]
    pub const fn effective(self) -> Self {
        match self {
            Self::Latest => Self::Cancun,
            other => other,
        }
    }

    /// Returns true if `self` is at or after `other`.
    #[inline]
    pub const fn is_active(self, other: Self) -> bool {
        self.effective() as u8 >= other as u8
    }
}
