use clap::{AppSettings, Parser};

#[derive(Debug, Clone, Parser)]
#[clap(global_setting = AppSettings::DeriveDisplayOrder)]
pub struct TestArgs {
    #[clap(long, short, help_heading = "DISPLAY OPTIONS")]
    list: bool,
}
