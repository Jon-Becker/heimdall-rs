use clap::{AppSettings, Parser};
use derive_builder::Builder;
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Decode calldata into readable types",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    global_setting = AppSettings::DeriveDisplayOrder,
    override_usage = "heimdall decode <TARGET> [OPTIONS]"
)]
pub struct DecodeArgs {
    /// The target to decode, either a transaction hash or string of bytes.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target calldata.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Your OpenAI API key, used for explaining calldata.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub openai_api_key: String,

    /// Whether to explain the decoded calldata using OpenAI.
    #[clap(long)]
    pub explain: bool,

    /// When prompted, always select the default value.
    #[clap(long, short)]
    pub default: bool,

    /// Whether to truncate nonstandard sized calldata.
    #[clap(long, short)]
    pub truncate_calldata: bool,

    /// Whether to skip resolving selectors. Heimdall will attempt to guess types.
    #[clap(long = "skip-resolving")]
    pub skip_resolving: bool,
}

impl DecodeArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            openai_api_key: Some(String::new()),
            explain: Some(false),
            default: Some(true),
            truncate_calldata: Some(false),
            skip_resolving: Some(false),
        }
    }
}
