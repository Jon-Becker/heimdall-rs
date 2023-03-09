use std::sync::Mutex;

use lazy_static::lazy_static;

use crate::dump::DumpState;


lazy_static! {
    pub static ref DUMP_STATE: Mutex<DumpState> = Mutex::new(DumpState::new());
    pub static ref DECODE_AS_TYPES: Vec<String> = vec![
        "bytes32".to_string(),
        "bool".to_string(),
        "address".to_string(),
        "string".to_string(),
        "uint256".to_string()
    ];
}