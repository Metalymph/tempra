use anyhow::Result;
use clap::{Parser, Subcommand};

mod app;

/// Fero — Write scripts, ship binaries.
#[derive(Parser)]
#[command(name = "fero", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a Fero script via the interpreter.
    Run {
        /// Path to the `.fero` source file.
        file: String,
    },
    /// Type-check a source file without executing it.
    Check {
        /// Path to the `.fero` source file.
        file: String,
    },
    /// Compile a source file to a binary or Wasm module.
    Build {
        /// Path to the `.fero` source file.
        file: String,
        /// Output target: `native` or `wasm32-wasi`.
        #[arg(long, default_value = "native")]
        target: String,
        /// Build mode: `fast` (performance) or `light` (size).
        #[arg(long, default_value = "fast")]
        mode: String,
    },
    /// Build and install a Fero tool locally.
    Install {
        /// Path to the `.fero` source file.
        file: String,
    },
    /// Run tests.
    Test,
    /// Format Fero source code.
    Fmt,
    /// Start an interactive REPL session.
    Repl,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let app = app::App::new();

    match cli.command {
        Command::Run { file } => app.run(&file),
        Command::Check { file } => app.check(&file),
        Command::Build { file, target, mode } => app.build(&file, &target, &mode),
        Command::Install { file } => app.install(&file),
        Command::Test => app.test(),
        Command::Fmt => app.fmt(),
        Command::Repl => app.repl(),
    }
}
