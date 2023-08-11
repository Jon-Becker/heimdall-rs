use std::sync::Mutex;

use fancy_regex::Regex;
use lazy_static::lazy_static;

use crate::snapshot::structures::state::State;

lazy_static! {
    pub static ref STATE: Mutex<State> = Mutex::new(State::new());

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

    // used to detect compiler size checks
    pub static ref VARIABLE_SIZE_CHECK_REGEX: Regex = Regex::new(r"!?\(?0(x01)? < [a-zA-Z0-9_\[\]]+\.length\)?").unwrap();
}
