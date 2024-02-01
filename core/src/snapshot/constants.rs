use std::sync::Mutex;

use fancy_regex::Regex;
use lazy_static::lazy_static;

use crate::snapshot::structures::state::State;

lazy_static! {
    /// global state for the snapshot module
    pub static ref STATE: Mutex<State> = Mutex::new(State::new());

    /// constant about text
    pub static ref ABOUT_TEXT: Vec<String> = vec![
        format!("heimdall-rs v{}", env!("CARGO_PKG_VERSION")),
        "By Jonathan Becker <jonathan@jbecker.dev>".to_string(),
        "The snapshot module allows users to quickly generate an overview of a contract's bytecode, without the need for the contract's source code.".to_string(),
    ];

    /// constant help menu text
    pub static ref HELP_MENU_COMMANDS: Vec<String> = vec![
        ":q, :quit                              exit the program".to_string(),
        ":h, :help                              display this help menu".to_string(),
    ];

    /// constant help menu text
    pub static ref HELP_MENU_CONTROLS: Vec<String> = vec![
        "↑, Scroll Up                           move the cursor up".to_string(),
        "↓, Scroll Down                         move the cursor down".to_string(),
        "←, →                                   switch scrolling context between selector list and snapshot information".to_string(),
        "ESC                                    clear the search filter".to_string(),
    ];

    /// used to detect compiler size checks
    pub static ref VARIABLE_SIZE_CHECK_REGEX: Regex = Regex::new(r"!?\(?0(x01)? < [a-zA-Z0-9_\[\]]+\.length\)?").expect("failed to compile regex");
}
