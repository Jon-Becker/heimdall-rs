use clap::{AppSettings, Parser};
use heimdall_config::parse_url_arg;

#[derive(Debug, Clone, Parser)]
#[clap(about = "Disassembles EVM bytecode to assembly",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder,
       override_usage = "heimdall disassemble <TARGET> [OPTIONS]")]
pub struct DisassemblerArgs {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    #[clap(required = true)]
    pub target: String,

    /// The RPC provider to use for fetching target bytecode.
    /// This can be an explicit URL or a reference to a MESC endpoint.
    #[clap(long, short, parse(try_from_str = parse_url_arg), default_value = "", hide_default_value = true)]
    pub rpc_url: String,

    /// Whether to use base-10 for the program counter.
    #[clap(long = "decimal-counter", short = 'd')]
    pub decimal_counter: bool,

    /// Name of the output file.
    #[clap(long, short, default_value = "", hide_default_value = true)]
    pub name: String,

    /// The output directory to write the output to or 'print' to print to the console
    #[clap(long = "output", short = 'o', default_value = "output", hide_default_value = true)]
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct DisassemblerArgsBuilder {
    /// The target to disassemble, either a file, bytecode, contract address, or ENS name.
    target: Option<String>,

    /// The RPC provider to use for fetching target bytecode.
    rpc_url: Option<String>,

    /// Whether to use base-10 for the program counter.
    decimal_counter: Option<bool>,

    /// Name of the output file.
    name: Option<String>,

    /// The output directory to write the output to or 'print' to print to the console
    output: Option<String>,
}

impl Default for DisassemblerArgsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DisassemblerArgsBuilder {
    pub fn new() -> Self {
        Self {
            target: Some(String::new()),
            rpc_url: Some(String::new()),
            decimal_counter: Some(false),
            name: Some(String::new()),
            output: Some(String::new()),
        }
    }

    pub fn target(&mut self, target: String) -> &mut Self {
        self.target = Some(target);
        self
    }

    pub fn rpc_url(&mut self, rpc_url: String) -> &mut Self {
        self.rpc_url = Some(rpc_url);
        self
    }

    pub fn decimal_counter(&mut self, decimal_counter: bool) -> &mut Self {
        self.decimal_counter = Some(decimal_counter);
        self
    }

    pub fn name(&mut self, name: String) -> &mut Self {
        self.name = Some(name);
        self
    }

    pub fn output(&mut self, output: String) -> &mut Self {
        self.output = Some(output);
        self
    }

    pub fn build(&self) -> eyre::Result<DisassemblerArgs> {
        Ok(DisassemblerArgs {
            target: self.target.clone().ok_or_else(|| eyre::eyre!("target is required"))?,
            rpc_url: self.rpc_url.clone().ok_or_else(|| eyre::eyre!("rpc_url is required"))?,
            decimal_counter: self
                .decimal_counter
                .ok_or_else(|| eyre::eyre!("decimal_counter is required"))?,
            name: self.name.clone().ok_or_else(|| eyre::eyre!("name is required"))?,
            output: self.output.clone().ok_or_else(|| eyre::eyre!("output is required"))?,
        })
    }
}
