#[derive(Debug, Clone)]
pub struct DumpRow {
    pub last_modified: String,
    pub alias: String,
    pub slot: String,
    pub decoded_type: String,
    pub value: String,
}
