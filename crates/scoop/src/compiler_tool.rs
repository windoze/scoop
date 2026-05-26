//! Helpers for invoking the compiler binary from the `scoop` facade.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use thiserror::Error;

/// Environment variable used to pin the compiler binary path during tests or custom installs.
pub(crate) const COMPILER_BIN_ENV_VAR: &str = "SCOOP_SCOOPC_BIN";

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(code(scoop::driver::compiler_binary_not_found))]
#[error("无法定位 scoopc 可执行文件：环境变量 `{env_var}` 与候选路径 {tried:?} 都不可用")]
pub(crate) struct CompilerBinaryNotFound {
    env_var: String,
    tried: Vec<PathBuf>,
}

/// Locate the compiler subprocess next to the current facade binary.
pub(crate) fn locate_compiler_bin() -> std::result::Result<PathBuf, CompilerBinaryNotFound> {
    let mut tried = Vec::new();
    if let Ok(raw) = std::env::var(COMPILER_BIN_ENV_VAR) {
        let path = PathBuf::from(raw);
        tried.push(path.clone());
        if path.is_file() {
            return Ok(path);
        }
    }

    let exe_name = if cfg!(windows) {
        "scoopc.exe"
    } else {
        "scoopc"
    };
    if let Ok(current) = std::env::current_exe()
        && let Some(parent) = current.parent()
    {
        let sibling = parent.join(exe_name);
        tried.push(sibling.clone());
        if sibling.is_file() {
            return Ok(sibling);
        }
        if parent.file_name().and_then(|s| s.to_str()) == Some("deps")
            && let Some(grandparent) = parent.parent()
        {
            let candidate = grandparent.join(exe_name);
            tried.push(candidate.clone());
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(CompilerBinaryNotFound {
        env_var: COMPILER_BIN_ENV_VAR.to_string(),
        tried,
    })
}

/// Build a command targeting the compiler subprocess.
pub(crate) fn command() -> Result<Command> {
    let compiler = locate_compiler_bin().map_err(miette::Report::new)?;
    Ok(Command::new(compiler))
}

/// Run a compiler command and fail with captured output on non-zero exit.
pub(crate) fn run_capture(mut cmd: Command, label: &str) -> Result<Output> {
    let output = cmd
        .output()
        .into_diagnostic()
        .wrap_err_with(|| format!("无法启动 compiler 子进程：{label}"))?;
    if output.status.success() {
        return Ok(output);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(miette::miette!(
        "compiler 子进程失败：{label}, status={}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        stdout,
        stderr
    ))
}

/// Append a path argument pair to a command.
pub(crate) fn arg_path(cmd: &mut Command, name: &str, path: &Path) {
    cmd.arg(name).arg(path);
}
