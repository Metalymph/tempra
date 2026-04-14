use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "fero")]
#[command(about = "Fero scripting language CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Run(RunArgs),
}

#[derive(Parser)]
pub struct RunArgs {
    pub path: PathBuf,
}
