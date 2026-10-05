//! Single-Cone artifact compiler CLI.

use std::path::PathBuf;
use std::process::ExitCode;
use std::{io, io::Write};

use clap::{Parser, Subcommand, ValueEnum};

mod child_protocol;

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
        /// Target triple; defaults to the supported host.
        #[arg(long)]
        target: Option<String>,
        /// Core sysroot; defaults to SCOOP_SYSROOT and the development layout.
        #[arg(long)]
        sysroot: Option<PathBuf>,
        /// Linux C compiler driver used for generated native bridges.
        #[arg(long)]
        cc: Option<PathBuf>,
        /// Target C development sysroot, separate from the Scoop sysroot.
        #[arg(long)]
        native_sysroot: Option<PathBuf>,
        /// Print pipeline stage dumps from this compilation to stdout.
        #[arg(long, value_enum)]
        emit: Option<Emit>,
    },
    /// Report the exact machine protocol understood by this compiler.
    #[command(name = "__machine-capability", hide = true)]
    MachineCapability,
    /// Compile exactly one request from the versioned machine transport.
    #[command(name = "__child-protocol", hide = true)]
    ChildProtocol { version: u32 },
}

#[derive(Clone, Copy, ValueEnum)]
enum Emit {
    Ast,
    Hir,
    Mir,
    Lir,
    All,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(error) = scoop_process::initialize() {
        eprintln!("cannot initialize process signal handling: {error}");
        return ExitCode::FAILURE;
    }
    let result = match cli.command {
        Command::Build {
            input,
            direct_slibs,
            support_slibs,
            out_slib,
            emit,
            target,
            sysroot,
            cc,
            native_sysroot,
        } => build(
            input,
            direct_slibs,
            support_slibs,
            out_slib,
            emit,
            scoopc::DirectBuildOptions {
                target,
                sysroot,
                c_toolchain: scoop_toolchain::CToolchainOptions {
                    compiler: cc,
                    native_sysroot,
                },
            },
        ),
        Command::MachineCapability => write_machine_capability(),
        Command::ChildProtocol { version } => child_protocol::run(version),
    };
    scoop_process::finish_interruption();
    result
}

fn write_machine_capability() -> ExitCode {
    let capability = match scoop_toolchain::paired_compiler_machine_capability() {
        Ok(capability) => capability,
        Err(error) => {
            eprintln!("error: cannot construct scoopc machine capability: {error}");
            return ExitCode::FAILURE;
        }
    };
    let frame = match scoop_protocol::encode_machine_capability_frame(&capability) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!("error: cannot encode scoopc machine capability: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut stdout = io::stdout().lock();
    if let Err(error) = stdout.write_all(&frame).and_then(|()| stdout.flush()) {
        eprintln!("error: cannot write scoopc machine capability: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn build(
    input: PathBuf,
    direct_slibs: Vec<PathBuf>,
    support_slibs: Vec<PathBuf>,
    out_slib: PathBuf,
    emit: Option<Emit>,
    options: scoopc::DirectBuildOptions,
) -> ExitCode {
    let emit = match emit {
        None => scoopc::StageDumpPolicy::None,
        Some(emit) => scoopc::StageDumpPolicy::Stages(match emit {
            Emit::Ast => scoop_protocol::StageDumpSet::one(scoopc::StageDumpKind::Ast),
            Emit::Hir => scoop_protocol::StageDumpSet::one(scoopc::StageDumpKind::Hir),
            Emit::Mir => scoop_protocol::StageDumpSet::one(scoopc::StageDumpKind::Mir),
            Emit::Lir => scoop_protocol::StageDumpSet::one(scoopc::StageDumpKind::Lir),
            Emit::All => scoop_protocol::StageDumpSet::all(),
        }),
    };
    let request = match scoopc::normalize_direct_build_request(
        input,
        direct_slibs,
        support_slibs,
        out_slib,
        scoopc::DiagnosticOutputPolicy::Human,
        emit,
        options,
    ) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    match request.build_and_publish() {
        Ok(success) => {
            if !success.warnings().is_empty() {
                eprintln!("{}", success.warnings().render_human());
            }
            for dump in success.emitted_dumps() {
                println!("{}", dump.text());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if let Some(warnings) = error.warnings().filter(|warnings| !warnings.is_empty()) {
                eprintln!("{}", warnings.render_human());
            }
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
            "all",
            "--target",
            "arm64-apple-darwin",
            "--sysroot",
            "custom-sysroot",
        ])
        .unwrap();
        let Command::Build {
            input,
            direct_slibs,
            support_slibs,
            out_slib,
            emit,
            target,
            sysroot,
            ..
        } = cli.command
        else {
            panic!("expected build command");
        };
        assert_eq!(input, PathBuf::from("Cone.toml"));
        assert_eq!(direct_slibs, [PathBuf::from("direct.slib")]);
        assert_eq!(support_slibs, [PathBuf::from("support.slib")]);
        assert_eq!(out_slib, PathBuf::from("current.slib"));
        assert!(matches!(emit, Some(Emit::All)));
        assert_eq!(target.as_deref(), Some("arm64-apple-darwin"));
        assert_eq!(sysroot, Some(PathBuf::from("custom-sysroot")));
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

    #[test]
    fn machine_capability_is_a_unique_hidden_command() {
        let cli = Cli::try_parse_from(["scoopc", "__machine-capability"]).unwrap();
        assert!(matches!(cli.command, Command::MachineCapability));
        assert!(Cli::try_parse_from(["scoopc", "__machine-capability", "unexpected"]).is_err());
    }

    #[test]
    fn child_protocol_is_a_unique_versioned_hidden_command() {
        let cli = Cli::try_parse_from(["scoopc", "__child-protocol", "1"]).unwrap();
        assert!(matches!(cli.command, Command::ChildProtocol { version: 1 }));
        assert!(Cli::try_parse_from(["scoopc", "__child-protocol"]).is_err());
        assert!(Cli::try_parse_from(["scoopc", "__child-protocol", "1", "extra"]).is_err());
    }
}
