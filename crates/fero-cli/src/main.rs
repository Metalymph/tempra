mod app;
mod commands;

use app::{Cli, Commands};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run(args) => {
            commands::run::run_command(args.path)
        }
    };

    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}
