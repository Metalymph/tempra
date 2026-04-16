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
    Check(CheckArgs),
    Test(TestArgs),
}

#[derive(Parser)]
pub struct RunArgs {
    pub path: PathBuf,
    pub args: Vec<String>,
}

#[derive(Parser)]
pub struct CheckArgs {
    pub path: PathBuf,
}

#[derive(Parser)]
pub struct TestArgs {
    pub path: PathBuf,
}
