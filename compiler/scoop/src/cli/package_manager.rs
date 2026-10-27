use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Command, ExitStatus},
};

use clap::Args;
use scoop::BuildResult;
use scoop_process::CommandExt;

use super::commands::config;

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
