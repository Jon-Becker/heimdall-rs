pub mod emit;
pub mod parser;
pub mod passes;
pub mod tokenizer;
pub mod types;

#[cfg(test)]
mod tests;

pub use emit::SolidityEmitter;
pub use parser::Parser;
pub use tokenizer::{Token, Tokenizer};
pub use types::{BinOp, Block, Expr, Function, Stmt, UnOp};

use crate::Error;
use heimdall_vm::ext::exec::VMTrace;

pub fn decompile_trace(trace: &VMTrace) -> Result<String, Error> {
    let tokens = Tokenizer::tokenize(trace)?;
    let mut ir = Parser::parse(tokens)?;

    // Run optimization passes
    ir = passes::run_all_passes(ir)?;

    // Emit Solidity
    let emitter = SolidityEmitter::new();
    Ok(emitter.emit(&ir))
}