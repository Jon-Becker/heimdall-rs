mod abi;
mod constructor;
mod multicall;

// re-export
pub(crate) use abi::{infer_dynamic_tail, infer_scalar_type, offset_in_words, try_decode};
pub(crate) use constructor::*;
pub(crate) use multicall::*;
