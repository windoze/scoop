use std::path::PathBuf;

use clap::Args;
use scoop::{BuildFailure, BuildFailurePhase, BuildResult};

use super::args::MessageFormat;
use super::commands::config;
use super::paths::{build_root, target_root};
use super::presentation::Reporter;

#[derive(Args)]
pub(super) struct CleanArgs {
    /// Cone directory, Cone.toml, or one .scoop file; defaults to ./Cone.toml.
    root_input: Option<PathBuf>,
    /// Remove this entire target directory, resolved relative to the current directory.
    #[arg(long)]
    target_dir: Option<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

pub(super) fn execute(args: CleanArgs, reporter: &Reporter) -> BuildResult<()> {
    let cwd = std::env::current_dir().map_err(config)?;
    let root = build_root(&cwd, args.root_input)?;
    let directory = target_root(&cwd, &root, args.target_dir)?;
    let removed = match std::fs::remove_dir_all(&directory) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(BuildFailure::tool(
                "SCOOP_CLEAN_IO",
                BuildFailurePhase::Publication,
                error,
            )
            .at_host(&directory));
        }
    };
    reporter.cleaned(&directory, removed).map_err(config)
}
