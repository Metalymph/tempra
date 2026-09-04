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
    Diagnostics(DiagnosticsArgs),
    Symbols(SymbolsArgs),
    /// Inspect capability/effect facts from a fresh VM compilation
    Effects(EffectsArgs),
    Test(TestArgs),
    Repl,
    /// Emit TIR (Tempra Intermediate Representation) for a source file [experimental]
    EmitTir(EmitTirArgs),
}

#[derive(Parser)]
pub struct RunArgs {
    #[arg(long, help = "Use the experimental bytecode VM and cache")]
    pub experimental_vm: bool,
    #[arg(
        long,
        help = "Enable experimental Cranelift JIT (requires --experimental-vm)"
    )]
    pub experimental_jit: bool,
    pub path: PathBuf,
    pub args: Vec<String>,
}

#[derive(Parser)]
pub struct CheckArgs {
    #[arg(long, help = "Output diagnostics in JSON format")]
    pub json: bool,
    pub path: PathBuf,
}

#[derive(Parser)]
pub struct DiagnosticsArgs {
    #[arg(long, help = "Output diagnostics in JSON format")]
    pub json: bool,
    pub path: PathBuf,
}

#[derive(Parser)]
pub struct SymbolsArgs {
    #[arg(long, help = "Output symbols in JSON format")]
    pub json: bool,
    pub path: PathBuf,
}

#[derive(Parser)]
pub struct EffectsArgs {
    #[arg(long, help = "Output effect facts or diagnostics as one JSON document")]
    pub json: bool,
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
