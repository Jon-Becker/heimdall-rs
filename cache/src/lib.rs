#[allow(deprecated)]
use std::{env::home_dir};
use clap::{AppSettings, Parser};
use serde::{Deserialize, Serialize};
use heimdall_common::{
    io::{
        file::{read_file, write_file},
        logging::*,
    }
};


#[derive(Debug, Clone, Parser)]
#[clap(about = "Manage heimdall-rs' cached objects",
       after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki",
       global_setting = AppSettings::DeriveDisplayOrder, 
       override_usage = "heimdall cache <SUBCOMMAND>")]
pub struct CacheArgs {
    #[clap(subcommand)]
    pub sub: Subcommands,
}


#[derive(Debug, Clone, Parser)]
#[clap(
    about = "Manage heimdall-rs' cached objects",
    after_help = "For more information, read the wiki: https://jbecker.dev/r/heimdall-rs/wiki"
)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommands {
    #[clap(name = "clean", about = "Removes all cached objects in ~/.bifrost/cache")]
    Clean(NoArguments),

    #[clap(name = "ls", about = "Lists all cached objects in ~/.bifrost/cache")]
    Ls(NoArguments),
}

#[derive(Debug, Clone, Parser)]
pub struct NoArguments {}


pub fn cache(args: CacheArgs) {
    let (logger, _) = Logger::new("");
    
    println!("{:#?}", args)
}