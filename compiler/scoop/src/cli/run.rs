use std::ffi::OsString;
use std::path::Path;
use std::process::ExitStatus;

use scoop::{BuildFailure, BuildFailurePhase, BuildResult};

pub(super) fn program(
    executable: &Path,
    output: &Path,
    args: &[OsString],
    cwd: &Path,
) -> BuildResult<ExitStatus> {
    #[cfg(windows)]
    {
        scoop_process::run_program(executable, output, args, cwd).map_err(failure)
    }
    #[cfg(not(windows))]
    {
        use scoop_process::CommandExt;
        use std::process::{Command, Stdio};
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.arg0(output);
        }
        let mut child = command.scoop_spawn().map_err(failure)?;
        let status = child.wait().map_err(failure)?;
        drop(child);
        Ok(status)
    }
}

fn failure(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_PROGRAM_START", BuildFailurePhase::Run, error)
}
