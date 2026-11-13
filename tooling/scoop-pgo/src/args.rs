use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use scoop::options::{BuildOptions, DumpScope, Emit};

use crate::Result;

#[derive(Parser)]
#[command(name = "scoop-pgo", version = scoop_toolchain::VERSION, about = "Build and run instrumented Scoop programs, then build with profile feedback")]
pub struct Cli {
    /// Paired main driver; otherwise SCOOP_DRIVER or the command installation root.
    #[arg(long, global = true)]
    pub scoop: Option<PathBuf>,
    /// Scoop installation root; otherwise SCOOP_SYSROOT or the command installation root.
    #[arg(long, global = true)]
    pub sysroot: Option<PathBuf>,
    /// Paired LLVM merge tool, for a development toolchain outside an installation.
    #[arg(long, global = true)]
    pub llvm_profdata: Option<PathBuf>,
    #[arg(long, global = true)]
    pub package_manager: Option<PathBuf>,
    #[arg(long, global = true)]
    pub enable_gc_stress: bool,
    #[arg(long, global = true)]
    pub enable_startup_stats: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Build with counters and collect a training run; append to the selected data directory.
    Instrument(Instrument),
    /// Build a release binary using the saved training configuration and matching data.
    Optimize(Common),
}

#[derive(Args)]
pub struct Common {
    #[command(flatten)]
    pub build: BuildOptions,
    /// Training data directory; independent of --target-dir, relative to this invocation's cwd.
    #[arg(long, default_value = "target/pgo")]
    pub data_dir: PathBuf,
    /// Binary destination; defaults to the data directory's phase output.
    #[arg(short = 'o')]
    pub output: Option<PathBuf>,
    #[arg(long, requires = "dump_dir")]
    pub emit: Option<Emit>,
    #[arg(long, requires = "emit")]
    pub dump_dir: Option<PathBuf>,
    #[arg(long, requires = "emit")]
    pub dump_scope: Option<DumpScope>,
}

#[derive(Args)]
pub struct Instrument {
    #[command(flatten)]
    pub common: Common,
    /// Save a build and collection directory for an externally launched process.
    #[arg(long)]
    pub no_run: bool,
    #[arg(last = true, allow_hyphen_values = true)]
    pub program_args: Vec<OsString>,
}

pub struct Parsed {
    pub cli: Cli,
    pub provided: BTreeSet<String>,
}

pub fn parse() -> Result<Parsed> {
    let matches = Cli::command().get_matches();
    let (_, command) = matches.subcommand().expect("Clap requires a command");
    let provided = command
        .ids()
        .filter(|id| {
            command.value_source(id.as_str()) == Some(clap::parser::ValueSource::CommandLine)
        })
        .map(|id| id.as_str().to_owned())
        .collect();
    Ok(Parsed {
        cli: Cli::from_arg_matches(&matches)?,
        provided,
    })
}

pub fn overlay(
    saved: &BuildOptions,
    current: BuildOptions,
    provided: &BTreeSet<String>,
    cwd: &Path,
) -> Result<BuildOptions> {
    let current = serde_json::to_value(current.normalized(cwd)?)?;
    let mut result = serde_json::to_value(saved)?;
    for (name, value) in current
        .as_object()
        .expect("build options serialize as an object")
    {
        if provided.contains(name) {
            result[name] = value.clone();
        }
    }
    Ok(serde_json::from_value(result)?)
}

pub fn validate_fixed_modes(common: &Common, provided: &BTreeSet<String>) -> Result<()> {
    for name in ["profile", "release", "profile_use", "profile_use_required"] {
        if provided.contains(name) {
            return Err(format!(
                "--{} is managed by scoop-pgo; choose instrument or optimize",
                name.replace('_', "-")
            )
            .into());
        }
    }
    if common
        .build
        .root_input
        .as_ref()
        .is_some_and(|root| root.as_os_str().is_empty())
    {
        return Err("root input must not be empty".into());
    }
    if common.data_dir.as_os_str().is_empty()
        || common
            .output
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
    {
        return Err("data directory and binary output must not be empty".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common(args: &[&str]) -> (Common, BTreeSet<String>) {
        let matches = Cli::command().try_get_matches_from(args).unwrap();
        let sub = matches.subcommand().unwrap().1;
        let provided = sub
            .ids()
            .filter(|id| {
                sub.value_source(id.as_str()) == Some(clap::parser::ValueSource::CommandLine)
            })
            .map(|id| id.to_string())
            .collect();
        let cli = Cli::from_arg_matches(&matches).unwrap();
        let common = match cli.command {
            Command::Instrument(args) => args.common,
            Command::Optimize(args) => args,
        };
        (common, provided)
    }

    #[test]
    fn explicit_overrides_preserve_saved_values_and_use_the_new_cwd_for_paths() {
        let previous = tempfile::tempdir().unwrap();
        let current = tempfile::tempdir().unwrap();
        let (first, _) = common(&[
            "scoop-pgo",
            "instrument",
            "project",
            "--export-ir",
            "full",
            "--jobs",
            "2",
            "--cc",
            "tools/cc",
            "--lockfile",
            "saved.lock",
        ]);
        let saved = first.build.normalized(previous.path()).unwrap();
        let (second, provided) = common(&[
            "scoop-pgo",
            "optimize",
            "--jobs",
            "7",
            "--library-path",
            "new libs",
        ]);
        let combined = overlay(&saved, second.build, &provided, current.path()).unwrap();
        assert_eq!(combined.root_input, Some(previous.path().join("project")));
        assert_eq!(combined.native.cc, Some(previous.path().join("tools/cc")));
        assert_eq!(
            combined.packages.lockfile,
            Some(previous.path().join("saved.lock"))
        );
        assert_eq!(combined.export_ir, scoop_protocol::IrTemplateExport::Full);
        assert_eq!(combined.jobs.unwrap().get(), 7);
        assert_eq!(combined.library_path, [current.path().join("new libs")]);
    }

    #[test]
    fn phase_overrides_are_rejected_and_program_arguments_remain_opaque() {
        let (args, provided) = common(&["scoop-pgo", "instrument", "--release"]);
        assert!(validate_fixed_modes(&args, &provided).is_err());
        let cli = Cli::try_parse_from([
            "scoop-pgo",
            "instrument",
            "app",
            "--",
            "",
            "--release",
            "two words",
        ])
        .unwrap();
        let Command::Instrument(args) = cli.command else {
            panic!("instrument expected")
        };
        assert_eq!(args.program_args, ["", "--release", "two words"]);
    }
}
