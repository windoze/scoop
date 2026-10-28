use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Args;
use scoop::BuildResult;
use scoop_manifest::DependencyCoordinateKey;
use scoop_process::CommandExt;
use scoop_protocol::package::{PackageMemberSelectionV1, PackageMembersV1};

use crate::cli::commands::{absolute, config};

#[derive(Args, Clone, Default)]
pub(in crate::cli) struct MemberOptions {
    /// Select all members of the workspace.
    #[arg(long, conflicts_with = "package")]
    pub workspace: bool,
    /// Select a workspace member; may be repeated.
    #[arg(short = 'p', long = "package", value_name = "GROUP:NAME")]
    pub package: Vec<DependencyCoordinateKey>,
}

impl MemberOptions {
    pub fn explicit(&self) -> bool {
        self.workspace || !self.package.is_empty()
    }

    pub fn selection(&self) -> PackageMemberSelectionV1 {
        if self.workspace {
            PackageMemberSelectionV1::All
        } else if self.package.is_empty() {
            PackageMemberSelectionV1::Default
        } else {
            PackageMemberSelectionV1::Packages {
                keys: self.package.clone(),
            }
        }
    }
}

pub(in crate::cli) fn members(
    cwd: &Path,
    input: Option<PathBuf>,
    options: &MemberOptions,
    executable: Option<PathBuf>,
) -> BuildResult<PackageMembersV1> {
    let executable = super::executable(cwd, executable)?;
    let mut command = Command::new(&executable);
    command.current_dir(cwd).args(["members", "--json"]);
    if let Some(input) = input {
        command.arg(absolute(cwd, input)?);
    }
    if options.workspace {
        command.arg("--workspace");
    }
    for package in &options.package {
        command.arg("--package").arg(package.to_string());
    }
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .scoop_spawn()
        .and_then(|child| child.wait_with_output())
        .map_err(|error| {
            config(format!(
                "cannot run package manager {}: {error}",
                executable.display()
            ))
        })?;
    super::check_output(&output, "member discovery")?;
    let result: PackageMembersV1 = serde_json::from_slice(&output.stdout).map_err(config)?;
    result.validate().map_err(config)?;
    Ok(result)
}
