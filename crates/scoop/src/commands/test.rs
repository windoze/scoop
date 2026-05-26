//! `scoop test` facade.

use std::num::{NonZeroU32, NonZeroUsize};
use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

const DEFAULT_FIXTURE_PROCESSES: usize = 5;
const FIXTURE_SCOOP_BIN_ENV: &str = "SCOOP_FIXTURE_SCOOP_BIN";

#[derive(Debug, Clone)]
pub struct TestOptions {
    pub opt_level: Option<scoop_project_model::OptLevel>,
    pub gc_stress: bool,
    pub gc_move: bool,
    pub threads: Option<NonZeroU32>,
    pub exit_on_failure: bool,
    pub processes: NonZeroUsize,
    pub session_options: super::FacadeSessionOptions,
}

impl Default for TestOptions {
    fn default() -> Self {
        Self {
            opt_level: None,
            gc_stress: false,
            gc_move: false,
            threads: None,
            exit_on_failure: false,
            processes: NonZeroUsize::new(DEFAULT_FIXTURE_PROCESSES).unwrap(),
            session_options: super::FacadeSessionOptions::new(),
        }
    }
}

pub fn run(fixtures: Option<PathBuf>, options: TestOptions) -> Result<()> {
    let mut cmd = crate::compiler_tool::command()?;
    cmd.arg("test-fixtures");
    if let Some(fixtures) = fixtures {
        cmd.arg("--fixtures").arg(fixtures);
    }
    cmd.arg("--processes")
        .arg(options.processes.get().to_string());
    if options.exit_on_failure {
        cmd.arg("--exit-on-failure");
    }
    if let Some(level) = options.opt_level {
        cmd.arg("--opt-level").arg(level.as_str());
    }
    if options.gc_stress {
        cmd.arg("--gc-stress");
    }
    if options.gc_move {
        cmd.arg("--gc-move");
    }
    if let Some(threads) = options.threads {
        cmd.arg("--threads").arg(threads.get().to_string());
    }
    options.session_options.apply_to_command(&mut cmd);
    if let Ok(current) = current_scoop_exe_path() {
        cmd.env(FIXTURE_SCOOP_BIN_ENV, current);
    }

    let status = cmd
        .status()
        .into_diagnostic()
        .wrap_err("无法启动 fixture compiler 子进程")?;
    if status.success() {
        Ok(())
    } else {
        Err(miette::miette!("fixtures 失败：{status}"))
    }
}

fn current_scoop_exe_path() -> std::io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    if current.is_file() {
        return Ok(current);
    }
    if let Some(stripped) = strip_deleted_exe_suffix(&current)
        && stripped.is_file()
    {
        return Ok(stripped);
    }
    if let Some(argv0) = std::env::args_os().next() {
        let argv0 = PathBuf::from(argv0);
        if argv0.is_file() {
            return Ok(argv0);
        }
        if argv0.is_relative() {
            let cwd_candidate = std::env::current_dir()?.join(&argv0);
            if cwd_candidate.is_file() {
                return Ok(cwd_candidate);
            }
        }
    }
    Ok(current)
}

fn strip_deleted_exe_suffix(path: &std::path::Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_str()?;
    let stripped = file_name.strip_suffix(" (deleted)")?;
    Some(path.with_file_name(stripped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn default_fixture_processes_is_stable() {
        assert_eq!(
            TestOptions::default().processes.get(),
            DEFAULT_FIXTURE_PROCESSES
        );
    }

    #[test]
    fn wrapper_env_key_is_stable() {
        let key = OsString::from(FIXTURE_SCOOP_BIN_ENV);
        assert_eq!(key.to_string_lossy(), "SCOOP_FIXTURE_SCOOP_BIN");
    }
}
