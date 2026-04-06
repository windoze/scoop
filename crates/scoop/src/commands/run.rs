//! `scoop run` 子命令。
//!
//! T0807：实现“build + exec”最小链路：
//! - 复用 `scoop build` 的前端检查与（feature-gated）后端/链接产物；
//! - 产物写到临时目录并执行；
//! - stdout/stderr 透传给当前进程；
//! - 子进程退出码透传为 `scoop run` 的退出码（便于后续 run-pass fixtures 断言）。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 执行 `scoop run <input>`。
///
/// 说明：
/// - 当未启用 LLVM 后端时（例如用 `--no-default-features` 构建），当前阶段无法产出可执行文件；
///   但仍会先做前端检查，然后给出明确报错（提示启用 LLVM）。
/// - 若被运行程序退出码非 0，会直接以相同退出码退出当前进程（不打印额外诊断）。
pub fn run(input: PathBuf, args: Vec<String>, entry_package: Option<String>) -> Result<()> {
    let dir = super::temp::make_temp_dir("scoop_run")?;
    let exe = dir.join(default_exe_name());

    let result = run_for_exit_code(input, &exe, args, entry_package);

    // 清理临时目录（尽力而为；不影响最终结果）。
    let _ = std::fs::remove_dir_all(&dir);

    match result? {
        0 => Ok(()),
        code => std::process::exit(code),
    }
}

fn run_for_exit_code(
    input: PathBuf,
    exe: &Path,
    args: Vec<String>,
    entry_package: Option<String>,
) -> Result<i32> {
    // 复用 build 的“前端检查 +（可选）生成二进制”逻辑。
    super::build::run(
        input,
        Some(exe.to_path_buf()),
        super::build::BuildOptions {
            emit: super::build::BuildEmit::Executable,
            entry_package,
        },
    )?;

    if !cfg!(feature = "llvm") {
        return Err(miette::miette!(
            "子命令 `run` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- run <file>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
        ));
    }

    if !exe.is_file() {
        return Err(miette::miette!(
            "构建未生成可执行文件：{}（这通常表示 LLVM/clang 工具链不可用）",
            exe.display()
        ));
    }

    let status = Command::new(exe)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .into_diagnostic()
        .wrap_err("运行可执行文件失败")?;

    Ok(status.code().unwrap_or(1))
}

fn default_exe_name() -> String {
    let ext = std::env::consts::EXE_EXTENSION;
    if ext.is_empty() {
        "a.out".to_string()
    } else {
        format!("a.{ext}")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    #[cfg(not(feature = "llvm"))]
    #[test]
    fn run_requires_llvm_feature() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        let err = super::run_for_exit_code(input, &exe, Vec::new(), None).unwrap_err();
        assert!(
            err.to_string().contains("需要启用 LLVM"),
            "应提示开启 llvm feature，实际：{err}"
        );
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn run_builds_and_executes_minimal_main() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        let code = super::run_for_exit_code(input, &exe, Vec::new(), None).unwrap();
        assert_eq!(code, 0, "最小 main 应返回 0");
    }
}
