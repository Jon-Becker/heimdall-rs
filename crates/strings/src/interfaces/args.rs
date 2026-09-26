use std::num::NonZeroUsize;

use alloy::primitives::Address;
use clap::Parser;
use derive_builder::Builder;
use heimdall_common::ether::{bytecode::get_bytecode_from_target, rpc::get_code};
use heimdall_config::parse_url_arg;

use crate::Error;

/// Arguments for extracting printable ASCII strings from EVM bytecode.
#[derive(Debug, Clone, Parser, Builder)]
#[builder(default)]
#[clap(
    about = "Extract printable ASCII strings from bytecode",
    override_usage = "heimdall strings <TARGET> [OPTIONS]"
)]
pub struct StringsArgs {
    /// Hex bytecode, a file containing hex bytecode, or a contract address.
    pub target: String,

    /// The RPC provider to use for fetching contract bytecode (URL or MESC endpoint).
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Minimum number of consecutive printable ASCII characters.
    #[clap(long, short = 'n', default_value = "4")]
    pub min_length: NonZeroUsize,

    /// Scan all bytecode, including selectors, instead of filtering PUSH instruction data.
    #[clap(long)]
    pub full_scan: bool,
}

impl Default for StringsArgs {
    fn default() -> Self {
        Self {
            target: String::new(),
            rpc_url: String::new(),
            min_length: NonZeroUsize::new(4).expect("nonzero minimum length"),
            full_scan: false,
        }
    }
}

impl StringsArgs {
    /// Loads the target bytecode, propagating RPC errors for address targets.
    pub async fn get_bytecode(&self) -> Result<Vec<u8>, Error> {
        if let Ok(address) = self.target.parse::<Address>() {
            Ok(get_code(address, &self.rpc_url).await?)
        } else {
            Ok(get_bytecode_from_target(&self.target, "", "").await?)
        }
    }
}

impl StringsArgsBuilder {
    /// Creates a builder with PUSH-only scanning and a minimum length of four.
    pub fn new() -> Self {
        Self::default()
    }
}
