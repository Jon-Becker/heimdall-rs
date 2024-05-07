//! clap [Args](clap::Args) for logging configuration.
// Mostly taken from [reth](https://github.com/paradigmxyz/reth)

use clap::{ArgAction, Args, ValueEnum};
use heimdall_tracing::{
    tracing_subscriber::filter::Directive, FileWorkerGuard, HeimdallTracer, LayerInfo, LogFormat,
    Tracer,
};
use std::{
    fmt::{self, Display},
    str::FromStr,
};
use tracing::{level_filters::LevelFilter, Level};

/// The log configuration.
#[derive(Debug, Args)]
#[clap(next_help_heading = "LOGGING")]
pub struct LogArgs {
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
    pub fn init_tracing(&self) -> eyre::Result<Option<FileWorkerGuard>> {
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
pub enum ColorMode {
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
            _ => Err(format!("Invalid color mode: {}", s)),
        }
    }
}

/// The verbosity settings for the cli.
#[derive(Debug, Copy, Clone, Args)]
#[clap(next_help_heading = "DISPLAY")]
pub struct Verbosity {
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
    pub fn directive(&self) -> Directive {
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
