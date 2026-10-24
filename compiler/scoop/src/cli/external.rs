use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_process::CommandExt;

use super::commands::{absolute, config};

pub(super) fn execute(args: &[OsString], sysroot: Option<PathBuf>) -> BuildResult<ExitStatus> {
    let (name, args) = args
        .split_first()
        .expect("Clap external subcommands include their name");
    if Path::new(name).file_name() != Some(name.as_os_str()) {
        return Err(config("external command name must be a single file name"));
    }
    let cwd = std::env::current_dir().map_err(config)?;
    let sysroot = absolute(
        &cwd,
        scoop_toolchain::configured_sysroot_root(sysroot).map_err(config)?,
    )?;
    let mut filename = OsString::from("scoop-");
    filename.push(name);
    let executable = sysroot.join("commands").join(filename);
    Command::new(&executable)
        .args(args)
        .scoop_spawn()
        .and_then(|mut child| child.wait())
        .map_err(|error| {
            BuildFailure::tool(
                "SCOOP_COMMAND_START",
                BuildFailurePhase::Run,
                format!(
                    "cannot run external command {}: {error}",
                    executable.display()
                ),
            )
        })
}
