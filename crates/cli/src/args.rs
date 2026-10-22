use clap::{Parser, Subcommand};

use clap::{ArgAction, Args, ValueEnum};
use heimdall_cache::CacheArgs;
use heimdall_config::ConfigArgs;
use heimdall_core::{
    heimdall_cfg::CfgArgs, heimdall_decoder::DecodeArgs, heimdall_decompiler::DecompilerArgs,
    heimdall_disassembler::DisassemblerArgs, heimdall_dump::DumpArgs,
    heimdall_inspect::InspectArgs,
};
use heimdall_tracing::{
    tracing_subscriber::filter::Directive, FileWorkerGuard, HeimdallTracer, LayerInfo, LogFormat,
    Tracer,
};
use std::{
    fmt::{self, Display},
    str::FromStr,
};
use tracing::{level_filters::LevelFilter, Level};

#[derive(Debug, Parser)]
#[clap(name = "heimdall", author = "Jonathan Becker <jonathan@jbecker.dev>", version)]
pub(crate) struct Arguments {
    #[clap(subcommand)]
    pub sub: Subcommands,

    #[clap(flatten)]
    pub logs: LogArgs,
}

#[derive(Debug, Subcommand)]
#[clap(
    about = "Heimdall is an advanced Ethereum smart contract toolkit for forensic and heuristic analysis.",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki"
)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Subcommands {
    #[clap(name = "disassemble", about = "Disassemble EVM bytecode to assembly")]
    Disassemble(DisassemblerArgs),

    #[clap(name = "decompile", about = "Decompile EVM bytecode to Solidity")]
    Decompile(DecompilerArgs),

    #[clap(name = "cfg", about = "Generate a visual control flow graph for EVM bytecode")]
    Cfg(CfgArgs),

    #[clap(name = "decode", about = "Decode calldata into readable types")]
    Decode(DecodeArgs),

    #[clap(name = "config", about = "Display and edit the current configuration")]
    Config(ConfigArgs),

    #[clap(name = "cache", about = "Manage heimdall-rs' cached files")]
    Cache(CacheArgs),

    #[clap(name = "dump", about = "Dump the value of all storage slots accessed by a contract")]
    Dump(DumpArgs),

    #[clap(
        name = "inspect",
        about = "Detailed inspection of Ethereum transactions, including calldata & trace decoding, log visualization, and more"
    )]
    Inspect(InspectArgs),

    #[clap(
        name = "orcarouter",
        about = "Connect to OrcaRouter and inspect its live model catalog"
    )]
    Orcarouter(OrcaRouterArgs),
}

/// Arguments for the `orcarouter` command group.
#[derive(Debug, Args)]
pub(crate) struct OrcaRouterArgs {
    #[clap(subcommand)]
    pub action: OrcaRouterAction,
}

/// The OrcaRouter subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum OrcaRouterAction {
    /// Authorize this machine with an OrcaRouter account (OAuth 2.0 + PKCE) and store the key.
    Login(OrcaRouterLoginArgs),

    /// Show the configured OrcaRouter key and its provenance.
    Status,

    /// Remove the stored OrcaRouter key.
    Logout,

    /// List the models the live OrcaRouter catalog offers for a capability.
    Models(OrcaRouterModelsArgs),
}

/// Arguments for `heimdall orcarouter login`.
#[derive(Debug, Args)]
pub(crate) struct OrcaRouterLoginArgs {
    /// Print the authorization URL instead of opening a browser. Use this on headless machines.
    #[clap(long)]
    pub no_browser: bool,

    /// How long to wait for the browser callback, in seconds.
    #[clap(long, default_value = "300", hide_default_value = true)]
    pub timeout: u64,

    /// Override the OrcaRouter authentication origin.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub auth_base: String,

    /// Override the OrcaRouter inference origin.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub api_base: String,
}

/// Arguments for `heimdall orcarouter models`.
#[derive(Debug, Args)]
pub(crate) struct OrcaRouterModelsArgs {
    /// The capability to list models for.
    #[clap(long, value_enum, default_value_t = OrcaRouterCapability::Chat)]
    pub capability: OrcaRouterCapability,

    /// Print the catalog as JSON.
    #[clap(long)]
    pub json: bool,

    /// Override the OrcaRouter inference origin.
    #[clap(long, default_value = "", hide_default_value = true)]
    pub api_base: String,
}

/// The capabilities a command can select models for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OrcaRouterCapability {
    /// Text chat and agent completions.
    Chat,
    /// Vector embeddings.
    Embedding,
    /// Image generation.
    Image,
    /// Video generation.
    Video,
    /// Reranking.
    Rerank,
}

impl From<OrcaRouterCapability> for heimdall_common::resources::orcarouter::Capability {
    fn from(value: OrcaRouterCapability) -> Self {
        use heimdall_common::resources::orcarouter::Capability;
        match value {
            OrcaRouterCapability::Chat => Capability::Chat,
            OrcaRouterCapability::Embedding => Capability::Embedding,
            OrcaRouterCapability::Image => Capability::Image,
            OrcaRouterCapability::Video => Capability::Video,
            OrcaRouterCapability::Rerank => Capability::Rerank,
        }
    }
}

#[derive(Debug, Args)]
#[clap(next_help_heading = "LOGGING")]
pub(crate) struct LogArgs {
    /// The format to use for logs written to stdout.
    #[clap(long = "log.stdout.format", value_name = "FORMAT", global = true, default_value_t = LogFormat::Terminal)]
    pub log_stdout_format: LogFormat,

    /// The filter to use for logs written to stdout.
    #[clap(long = "log.stdout.filter", value_name = "FILTER", global = true, default_value = "")]
    pub log_stdout_filter: String,

    /// Write logs to journald.
    #[clap(long = "log.journald", global = true)]
    pub journald: bool,

    /// The filter to use for logs written to journald.
    #[clap(
        long = "log.journald.filter",
        value_name = "FILTER",
        global = true,
        default_value = "error"
    )]
    pub journald_filter: String,

    /// Sets whether or not the formatter emits ANSI terminal escape codes for colors and other
    /// text formatting.
    #[clap(
        long,
        value_name = "COLOR",
        global = true,
        default_value_t = ColorMode::Always
    )]
    pub color: ColorMode,

    /// The verbosity settings for the tracer.
    #[clap(flatten)]
    pub verbosity: Verbosity,
}

impl LogArgs {
    /// Creates a [LayerInfo] instance.
    fn layer(&self, format: LogFormat, filter: String, use_color: bool) -> LayerInfo {
        LayerInfo::new(
            format,
            self.verbosity.directive().to_string(),
            filter,
            if use_color { Some(self.color.to_string()) } else { None },
        )
    }

    /// Initializes tracing with the configured options from cli args.
    pub(crate) fn init_tracing(&self) -> eyre::Result<Option<FileWorkerGuard>> {
        let mut tracer = HeimdallTracer::new();

        let stdout = self.layer(self.log_stdout_format, self.log_stdout_filter.clone(), true);
        tracer = tracer.with_stdout(stdout);

        if self.journald {
            tracer = tracer.with_journald(self.journald_filter.clone());
        }

        let guard = tracer.init()?;
        Ok(guard)
    }
}

/// The color mode for the cli.
#[derive(Debug, Copy, Clone, ValueEnum, Eq, PartialEq)]
pub(crate) enum ColorMode {
    /// Colors on
    Always,
    /// Colors on
    Auto,
    /// Colors off
    Never,
}

impl Display for ColorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorMode::Always => write!(f, "always"),
            ColorMode::Auto => write!(f, "auto"),
            ColorMode::Never => write!(f, "never"),
        }
    }
}

impl FromStr for ColorMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "always" => Ok(ColorMode::Always),
            "auto" => Ok(ColorMode::Auto),
            "never" => Ok(ColorMode::Never),
            _ => Err(format!("Invalid color mode: {s}")),
        }
    }
}

#[derive(Debug, Copy, Clone, Args)]
#[clap(next_help_heading = "DISPLAY")]
pub(crate) struct Verbosity {
    /// Set the minimum log level.
    ///
    /// -v     Warnings & Errors
    /// -vv    Info
    /// -vvv   Debug
    /// -vvvv  Traces (warning: very verbose!)
    #[clap(short, long, action = ArgAction::Count, global = true, default_value_t = 1, verbatim_doc_comment, help_heading = "DISPLAY")]
    verbosity: u8,

    /// Silence all log output.
    #[clap(long, alias = "silent", short = 'q', global = true, help_heading = "DISPLAY")]
    quiet: bool,
}

impl Verbosity {
    /// Get the corresponding [Directive] for the given verbosity, or none if the verbosity
    /// corresponds to silent.
    pub(crate) fn directive(&self) -> Directive {
        if self.quiet {
            LevelFilter::OFF.into()
        } else {
            let level = match self.verbosity - 1 {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            };

            level.into()
        }
    }
}
