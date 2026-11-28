mod abi;
mod constructor;
mod multicall;

// re-export
pub(crate) use abi::{
    alternative_raw_inputs, can_start_dynamic_tail, full_word_row_inputs, infer_dynamic_tail,
    infer_scalar_type, infer_tail_ending_at, offset_in_words, try_decode,
};
pub(crate) use constructor::*;
pub(crate) use multicall::*;
