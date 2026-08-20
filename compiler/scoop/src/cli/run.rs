use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_process::CommandExt;

pub(super) fn program(
    stable: &Path,
    copy: tempfile::TempPath,
    args: &[OsString],
    cwd: &Path,
) -> BuildResult<ExitStatus> {
    let mut command = Command::new(&copy);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.arg0(stable);
    }
    let mut child = command.scoop_spawn().map_err(failure)?;
    let status = child.wait().map_err(failure)?;
    drop(child);
    drop(copy);
    Ok(status)
}

fn failure(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_PROGRAM_START", BuildFailurePhase::Run, error)
}
