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
    let filename = scoop_toolchain::host_executable_name(filename);
    let mut executable = sysroot.join("commands").join(&filename);
    if !executable.try_exists().map_err(config)?
        && let Some(shared) = scoop_toolchain::shared_commands_root()
    {
        let shared = absolute(&cwd, shared)?.join(filename);
        if shared.try_exists().map_err(config)? {
            executable = shared;
        }
    }
    Command::new(&executable)
        .args(args)
        .env("SCOOP_DRIVER", std::env::current_exe().map_err(config)?)
        .env("SCOOP_SYSROOT", &sysroot)
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn external_command_receives_the_selected_driver_and_sysroot() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("selected root");
        std::fs::create_dir_all(root.join("commands")).unwrap();
        let command = root.join("commands/scoop-probe");
        std::fs::write(
            &command,
            b"#!/bin/sh\nprintf '%s\\n' \"$SCOOP_DRIVER\" \"$SCOOP_SYSROOT\" > \"$1\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
        let output = temporary.path().join("environment");
        let result = execute(
            &["probe".into(), output.as_os_str().to_owned()],
            Some(root.clone()),
        )
        .unwrap();
        assert!(result.success());
        assert_eq!(
            std::fs::read_to_string(output).unwrap(),
            format!(
                "{}\n{}\n",
                std::env::current_exe().unwrap().display(),
                root.display()
            )
        );
    }
}
