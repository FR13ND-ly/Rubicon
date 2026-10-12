use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "rb", version, about = "Run Rubicon applications")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, value_name = "FILE")]
    pub config: PathBuf,

    #[arg(long, value_enum, default_value = "rubicon")]
    pub backend: Backend,

    #[arg(long, help = "Show launch configuration without starting the server")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Backend {
    Rubicon,
    Node,
}
