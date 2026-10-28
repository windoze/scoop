use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Command, ExitStatus},
};

use clap::Args;
use scoop::BuildResult;
use scoop_process::CommandExt;

use super::commands::config;

mod prepare;
pub(super) use prepare::{PackageOptions, PrepareInput, prepare};
mod members;
pub(super) use members::{MemberOptions, members};

fn executable(cwd: &std::path::Path, explicit: Option<PathBuf>) -> BuildResult<PathBuf> {
    match explicit {
        Some(path) => super::commands::absolute(cwd, path),
        None => std::env::current_exe()
            .map_err(config)
            .map(|path| path.with_file_name(format!("scoop-pm{}", std::env::consts::EXE_SUFFIX))),
    }
}

fn check_output(output: &std::process::Output, action: &str) -> BuildResult<()> {
    if output.status.success() {
        return Ok(());
    }
    if let Ok(record) =
        serde_json::from_slice::<scoop_protocol::package::PackageDiagnosticV1>(&output.stderr)
    {
        record.validate().map_err(config)?;
        let mut failure = config(record.diagnostic.message());
        failure.diagnostics.push(record.diagnostic);
        return Err(failure);
    }
    Err(config(format!(
        "package {action} failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[derive(Args)]
#[command(disable_help_flag = true, disable_version_flag = true)]
pub(super) struct PmArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}

pub(super) fn execute(args: PmArgs, executable: Option<PathBuf>) -> BuildResult<ExitStatus> {
    let executable = match executable {
        Some(path) => path,
        None => std::env::current_exe()
            .map_err(config)?
            .with_file_name(format!("scoop-pm{}", std::env::consts::EXE_SUFFIX)),
    };
    Command::new(&executable)
        .args(args.args)
        .scoop_spawn()
        .and_then(|mut child| child.wait())
        .map_err(|error| {
            config(format!(
                "cannot run package manager {}: {error}",
                executable.display()
            ))
        })
}
