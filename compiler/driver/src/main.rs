//! Single-Cone artifact compiler CLI.

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
    /// Compile one Cone into a validated `.slib` artifact.
    Build {
        /// Cone directory, exact `Cone.toml`, or one `.scoop` file.
        input: PathBuf,
        /// Direct dependency artifact; may be repeated.
        #[arg(long = "direct-slib")]
        direct_slibs: Vec<PathBuf>,
        /// Transitive support dependency artifact; may be repeated.
        #[arg(long = "support-slib")]
        support_slibs: Vec<PathBuf>,
        /// Required destination for the validated current-Cone artifact.
        #[arg(long = "out-slib")]
        out_slib: PathBuf,
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
            input,
            direct_slibs,
            support_slibs,
            out_slib,
            emit,
        } => build(input, direct_slibs, support_slibs, out_slib, emit),
    }
}

fn build(
    input: PathBuf,
    direct_slibs: Vec<PathBuf>,
    support_slibs: Vec<PathBuf>,
    out_slib: PathBuf,
    emit: Option<Emit>,
) -> ExitCode {
    let emit = match emit {
        None => scoopc::StageDumpPolicy::None,
        Some(emit) => scoopc::StageDumpPolicy::Stage(match emit {
            Emit::Ast => scoopc::StageDumpKind::Ast,
            Emit::Hir => scoopc::StageDumpKind::Hir,
            Emit::Mir => scoopc::StageDumpKind::Mir,
            Emit::Lir => scoopc::StageDumpKind::Lir,
        }),
    };
    let request = match scoopc::normalize_direct_build_request(
        input,
        direct_slibs,
        support_slibs,
        out_slib,
        scoopc::DiagnosticOutputPolicy::Human,
        emit,
    ) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match request.build_and_publish(scoop_wire::DecodeLimits::default()) {
        Ok(success) => {
            if !success.warnings().is_empty() {
                eprintln!("{}", success.warnings().render_human());
            }
            if let Some(dump) = success.emitted_dump() {
                println!("{}", dump.text());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_build_arguments_are_accepted() {
        let cli = Cli::try_parse_from([
            "scoopc",
            "build",
            "Cone.toml",
            "--direct-slib",
            "direct.slib",
            "--support-slib",
            "support.slib",
            "--out-slib",
            "current.slib",
            "--emit",
            "lir",
        ])
        .unwrap();
        let Command::Build {
            input,
            direct_slibs,
            support_slibs,
            out_slib,
            emit,
        } = cli.command;
        assert_eq!(input, PathBuf::from("Cone.toml"));
        assert_eq!(direct_slibs, [PathBuf::from("direct.slib")]);
        assert_eq!(support_slibs, [PathBuf::from("support.slib")]);
        assert_eq!(out_slib, PathBuf::from("current.slib"));
        assert!(matches!(emit, Some(Emit::Lir)));
    }

    #[test]
    fn executable_runner_flags_are_rejected() {
        assert!(
            Cli::try_parse_from([
                "scoopc",
                "build",
                "main.scoop",
                "-o",
                "out",
                "--out-slib",
                "main.slib",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "scoopc",
                "build",
                "main.scoop",
                "-L",
                "native",
                "--out-slib",
                "main.slib",
            ])
            .is_err()
        );
    }
}
