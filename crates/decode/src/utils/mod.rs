mod abi;

use ethers::abi::{decode, Param, ParamType, Token};
use eyre::eyre;

use crate::error::Error;

// re-export
pub use abi::{try_decode, try_decode_dynamic_parameter};
