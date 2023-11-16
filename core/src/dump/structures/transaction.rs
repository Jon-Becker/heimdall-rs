/// A single EVM transaction, which contains the hash, block number, and whether or not it has been
/// indexed by the dump process yet.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub indexed: bool,
    pub hash: String,
    pub block_number: u128,
}
