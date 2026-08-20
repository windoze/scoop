//! Compiler CLI: `scoopc`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "scoopc", version, about = "The Scoop compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a `.scoop` source file.
    Build { file: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { file } => match scoopc::compile_file(&file) {
            Ok(()) => ExitCode::SUCCESS,
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    eprintln!("{}", diagnostic.message);
                }
                ExitCode::FAILURE
            }
        },
    }
}
