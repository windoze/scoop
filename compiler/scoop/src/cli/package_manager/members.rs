use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use scoop::BuildResult;
use scoop_process::CommandExt;
use scoop_protocol::package::PackageMembersV1;

use crate::cli::commands::{absolute, config};

pub(in crate::cli) use scoop::options::MemberOptions;

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
