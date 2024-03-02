use ethers::types::H256;

/// A storage slot, which contains the alias, value, and modifiers of a storage slot.
#[derive(Debug, Clone)]
pub struct StorageSlot {
    pub alias: Option<String>,
    pub value: H256,
    pub modifiers: Vec<(u128, String)>,
    pub decode_as_type_index: usize,
}
