mod app;
mod commands;

use app::{Cli, Commands};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run(args) => commands::run::run_command(args.path, args.args),
        Commands::Check(args) => commands::check::check_command(args.path),
        Commands::Test(args) => commands::test::test_command(args.path),
        Commands::Repl => commands::repl::repl_command(),
        Commands::EmitTir(args) => commands::emit_tir::emit_tir_command(args.path),
    };

    if let Err(err) = result {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}
