//! Compiler CLI: `scoopc`.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "scoopc", version, about = "The Scoop compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a `.scoop` source file into an executable.
    Build {
        /// The `.scoop` source file to compile.
        file: PathBuf,
        /// Directory for the object file and the linked executable.
        #[arg(short = 'o', default_value = "./target/scoopc-out")]
        out_dir: PathBuf,
        /// Print the text dump of one pipeline stage to stdout.
        #[arg(long, value_enum)]
        emit: Option<Emit>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Emit {
    Ast,
    Hir,
    Mir,
    Lir,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Build {
            file,
            out_dir,
            emit,
        } => match scoopc::compile_file(&file, &out_dir) {
            Ok(success) => {
                if let Some(emit) = emit {
                    let dump = match emit {
                        Emit::Ast => success.dumps.ast,
                        Emit::Hir => success.dumps.hir,
                        Emit::Mir => success.dumps.mir,
                        Emit::Lir => success.dumps.lir,
                    };
                    println!("{dump}");
                }
                ExitCode::SUCCESS
            }
            Err(diagnostics) => {
                // Map each diagnostic's file index back to its input;
                // diagnostics raised before loading finished (bad
                // sysroot, unreadable file) fall back to the user file.
                let inputs = scoopc::load_inputs(&file).unwrap_or_default();
                let name = file.display().to_string();
                let source = std::fs::read_to_string(&file).unwrap_or_default();
                eprintln!(
                    "{}",
                    scoopc::render_diagnostics(&diagnostics, &inputs, &name, &source)
                );
                ExitCode::FAILURE
            }
        },
    }
}
