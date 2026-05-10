use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tempra")]
#[command(about = "Tempra scripting language CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Run(RunArgs),
    Check(CheckArgs),
    Test(TestArgs),
    Repl,
    /// Emit TIR (Tempra Intermediate Representation) for a source file [experimental]
    EmitTir(EmitTirArgs),
}

#[derive(Parser)]
pub struct RunArgs {
    #[arg(long, help = "Use the experimental bytecode VM and cache")]
    pub experimental_vm: bool,
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

#[derive(Parser)]
pub struct EmitTirArgs {
    pub path: PathBuf,
}
