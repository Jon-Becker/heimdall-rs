extern crate clipboard;
use clipboard::{ClipboardProvider, ClipboardContext};

pub fn copy_to_clipboard(text: &str) {
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    ctx.set_contents(text.to_string()).unwrap();
}