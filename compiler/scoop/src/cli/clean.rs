use std::path::PathBuf;

use clap::Args;
use scoop::{BuildFailure, BuildFailurePhase, BuildResult};

use super::args::MessageFormat;
use super::commands::{absolute, config};
use super::paths::build_root;
use super::presentation::Reporter;

#[derive(Args)]
pub(super) struct CleanArgs {
    /// Cone directory, Cone.toml, or one .scoop file; otherwise discover local workspace.
    root_input: Option<PathBuf>,
    /// Remove this entire target directory, resolved relative to the current directory.
    #[arg(long)]
    target_dir: Option<PathBuf>,
    #[arg(long, default_value = "human")]
    pub message_format: MessageFormat,
}

pub(super) fn execute(
    args: CleanArgs,
    package_manager: Option<PathBuf>,
    reporter: &Reporter,
) -> BuildResult<()> {
    let cwd = std::env::current_dir().map_err(config)?;
    let directory = if let Some(path) = args.target_dir {
        absolute(&cwd, path)?
    } else {
        let single_file = args
            .root_input
            .as_ref()
            .filter(|path| path.extension().is_some_and(|ext| ext == "scoop"))
            .map(|path| build_root(&cwd, Some(path.clone())))
            .transpose()?
            .is_some_and(|root| root.manifest_root().is_none());
        if single_file {
            cwd.join("target")
        } else {
            super::package_manager::members(
                &cwd,
                args.root_input,
                &Default::default(),
                package_manager,
            )?
            .root
            .to_path_buf()
            .map_err(config)?
            .join("target")
        }
    };
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
