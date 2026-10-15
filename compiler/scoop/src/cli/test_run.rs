use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_process::CommandExt;

use super::{presentation::Reporter, test_selection::SelectedTests};
use model::{Output, RunResult, Status};

mod events;
pub(super) mod model;

pub(super) fn program(
    executable: &Path,
    output: &Path,
    selected: &SelectedTests,
    nocapture: bool,
    cwd: &Path,
    reporter: &Reporter,
    target: &str,
) -> BuildResult<Option<ExitStatus>> {
    if selected.empty_reason.is_some() {
        let mut result = RunResult::empty(0);
        result.complete = true;
        reporter
            .test_report(target, selected, &result, None)
            .map_err(failure)?;
        return Ok(None);
    }
    let run = tempfile::Builder::new()
        .prefix(".test-run-")
        .tempdir_in(output.parent().expect("an executable has a parent"))
        .map_err(failure)?;
    let mut command = Command::new(executable);
    command
        .args(&selected.arguments)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .env("SCOOP_TEST_RUN_DIR", run.path())
        .env_remove("SCOOP_TEST_NOCAPTURE");
    if nocapture {
        command
            .env("SCOOP_TEST_NOCAPTURE", "1")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
    } else {
        command
            .stdout(File::create(run.path().join("target.stdout")).map_err(failure)?)
            .stderr(File::create(run.path().join("target.stderr")).map_err(failure)?);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.arg0(output);
    }
    let mut child = command.scoop_spawn().map_err(failure)?;
    let status = child.wait().map_err(failure)?;
    drop(child);
    let expected = selected
        .arguments
        .iter()
        .skip(1)
        .map(|key| key.to_str().expect("generated case keys are ASCII"))
        .collect::<Vec<_>>();
    let mut result = match File::open(run.path().join("events.jsonl")) {
        Ok(file) => events::read(BufReader::new(file), &expected),
        Err(error) => {
            let mut result = RunResult::empty(expected.len());
            result.target_failure = Some(format!("cannot read test events: {error}"));
            result
        }
    };
    // Exit 1 is the generated entry's ordinary assertion/exception failure status.
    // All other nonzero exits, and exit 1 after passing cases, are target failures.
    let ordinary_failure = status.code() == Some(1)
        && result.complete
        && result
            .cases
            .iter()
            .any(|case| case.status == Status::Failed);
    if !status.success() && !ordinary_failure && result.target_failure.is_none() {
        result.target_failure = Some(format!("test process failed: {status}"));
    }
    if !nocapture {
        result.output = Some(read_output(run.path(), "target").map_err(failure)?);
        for (index, case) in result.cases.iter_mut().enumerate() {
            if case.status == Status::Failed {
                case.output = Some(read_output(run.path(), &index.to_string()).map_err(failure)?);
            }
        }
    }
    reporter
        .test_report(target, selected, &result, Some(status))
        .map_err(failure)?;
    if status.success() && result.failed() {
        return Err(failure("test run failed; see the test report"));
    }
    Ok(Some(status))
}

fn read_output(directory: &Path, prefix: &str) -> io::Result<Output> {
    fn read(path: &Path) -> io::Result<Vec<u8>> {
        match fs::read(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            result => result,
        }
    }
    Ok(Output {
        stdout: read(&directory.join(format!("{prefix}.stdout")))?,
        stderr: read(&directory.join(format!("{prefix}.stderr")))?,
    })
}

fn failure(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_TEST_RUN", BuildFailurePhase::Run, error)
}
