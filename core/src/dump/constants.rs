use std::sync::Mutex;

use lazy_static::lazy_static;

use crate::dump::structures::dump_state::DumpState;

lazy_static! {
    pub static ref DUMP_STATE: Mutex<DumpState> = Mutex::new(DumpState::new());
    pub static ref DECODE_AS_TYPES: Vec<String> = vec![
        "bytes32".to_string(),
        "bool".to_string(),
        "address".to_string(),
        "string".to_string(),
        "uint256".to_string()
    ];

    pub static ref ABOUT_TEXT: Vec<String> = vec![
        format!("heimdall-rs v{}", env!("CARGO_PKG_VERSION")),
        "By Jonathan Becker <jonathan@jbecker.dev>".to_string(),
        "The storage dump module will fetch all storage slots and values accessed by any EVM contract.".to_string(),
    ];

    pub static ref HELP_MENU_COMMANDS: Vec<String> = vec![
        ":q, :quit                              exit the program".to_string(),
        ":h, :help                              display this help menu".to_string(),
        ":f, :find      <VALUE>                 search for a storage slot by slot or value".to_string(),
        ":e, :export    <FILENAME>              export the current storage dump to a file, preserving decoded values".to_string(),
        ":s, :seek      <DIRECTION> <AMOUNT>    move the cusor up or down by a specified amount".to_string(),
    ];

    pub static ref HELP_MENU_CONTROLS: Vec<String> = vec![
        "↑, Scroll Up                           move the cursor up one slot".to_string(),
        "↓, Scroll Down                         move the cursor down one slot".to_string(),
        "←, →                                   change the decoding type of the selected slot".to_string(),
        "CTRL + ↑, CTRL + ↓                     move the cursor up or down by 10 slots".to_string(),
        "ESC                                    clear the search filter".to_string(),
    ];
}
