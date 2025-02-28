use clap::Parser;
use derive_builder::Builder;
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser, Builder)]
#[clap(
    about = "Dump the value of all storage slots accessed by a contract",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
    override_usage = "heimdall dump <TARGET> [OPTIONS]"
)]
/// Arguments for the dump operation
///
/// This struct contains all the configuration parameters needed to perform
/// a storage slot dump for a target contract.
pub struct DumpArgs {
    /// The target to find and dump the storage slots of.
    #[clap(required = true)]
    pub target: String,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short, default_value = "output", hide_default_value = true)]
    pub output: String,

    /// The RPC URL to use for fetching data.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, value_parser = parse_url_arg, default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// The number of threads to use when fetching data.
    #[clap(long, default_value = "4", hide_default_value = true)]
    pub threads: usize,

    /// The block number to start dumping from.
    #[clap(long, short, default_value = "0", hide_default_value = true, alias = "start_block")]
    pub from_block: u128,

    /// The block number to stop dumping at.
    #[clap(long, short, alias = "end_block")]
    pub to_block: Option<u128>,

    /// The name for the output file
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,
}

impl DumpArgsBuilder {
    /// Creates a new DumpArgsBuilder with default values
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            output: Some(String::new()),
            rpc_url: Some(String::new()),
            threads: Some(4),
            from_block: Some(0),
            to_block: Some(None),
            name: Some(String::new()),
        }
    }
}
