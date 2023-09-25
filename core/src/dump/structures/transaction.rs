#[derive(Debug, Clone)]
pub struct Transaction {
    pub indexed: bool,
    pub hash: String,
    pub block_number: u128,
}
