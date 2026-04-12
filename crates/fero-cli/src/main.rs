use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use fero_driver::{BuildRequest, Driver, DriverCommand};
use fero_package::{BuildMode, BuildTarget};
use fero_repl::Repl;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "fero", about = "Fero compiler and toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        path: PathBuf,
    },
    Check {
        path: PathBuf,
    },
    Build {
        path: PathBuf,
        #[arg(long, value_enum, default_value = "native")]
        target: TargetArg,
        #[arg(long, value_enum, default_value = "fast")]
        mode: ModeArg,
    },
    Install {
        path: PathBuf,
        #[arg(long, value_enum, default_value = "native")]
        target: TargetArg,
        #[arg(long, value_enum, default_value = "fast")]
        mode: ModeArg,
    },
    Test {
        path: Option<PathBuf>,
    },
    Fmt {
        path: Option<PathBuf>,
    },
    Repl,
}

#[derive(Clone, Copy, ValueEnum)]
enum TargetArg {
    Native,
    Wasm32Wasi,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Fast,
    Light,
}

impl From<TargetArg> for BuildTarget {
    fn from(value: TargetArg) -> Self {
        match value {
            TargetArg::Native => BuildTarget::Native,
            TargetArg::Wasm32Wasi => BuildTarget::Wasm32Wasi,
        }
    }
}

impl From<ModeArg> for BuildMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Fast => BuildMode::Fast,
            ModeArg::Light => BuildMode::Light,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let driver = Driver;
    match cli.command {
        Command::Run { path } => print_outcome(driver.execute(DriverCommand::Run, &path)?),
        Command::Check { path } => print_outcome(driver.execute(DriverCommand::Check, &path)?),
        Command::Build { path, target, mode } => print_outcome(driver.execute(
            DriverCommand::Build(BuildRequest {
                target: target.into(),
                mode: mode.into(),
            }),
            &path,
        )?),
        Command::Install { path, target, mode } => print_outcome(driver.execute(
            DriverCommand::Install(BuildRequest {
                target: target.into(),
                mode: mode.into(),
            }),
            &path,
        )?),
        Command::Test { path } => {
            let path = path.unwrap_or_else(|| PathBuf::from("tests"));
            print_outcome(driver.execute(DriverCommand::Test, &path)?);
        }
        Command::Fmt { path } => {
            let path = path.unwrap_or_else(|| PathBuf::from("."));
            print_outcome(driver.execute(DriverCommand::Fmt, &path)?);
        }
        Command::Repl => println!("{}", Repl.banner()),
    }
    Ok(())
}

fn print_outcome(outcome: fero_driver::DriverOutcome) {
    println!("{}", outcome.summary);
}
