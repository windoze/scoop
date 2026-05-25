//! `scoop run` 子命令。
//!
//! T0807：实现“build + exec”最小链路：
//! - 复用 `scoop build` 的前端检查与（feature-gated）后端/链接产物；
//! - 产物写到临时目录并执行；
//! - stdout/stderr 透传给当前进程；
//! - 子进程退出码透传为 `scoop run` 的退出码（便于后续 run-pass fixtures 断言）。
//!
//! T1123：cone 项目目录下的 `scoop run`：
//! - 省略 input 时默认使用当前目录（向上发现 `Cone.toml`）；
//! - profile 复用 `scoop build --debug/--release`；
//! - 产物落到 `build/<profile>/bin/<project-name>`，并直接运行该产物。

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 执行 `scoop run <input>`。
///
/// 说明：
/// - 当未启用 LLVM 后端时（例如用 `--no-default-features` 构建），当前阶段无法产出可执行文件；
///   但仍会先做前端检查，然后给出明确报错（提示启用 LLVM）。
/// - 若被运行程序退出码非 0，会直接以相同退出码退出当前进程（不打印额外诊断）。
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: Option<PathBuf>,
    args: Vec<String>,
    entry_package: Option<String>,
    profile: super::build::BuildProfile,
    opt_level: Option<scoopc::opt::OptLevel>,
    incremental: bool,
    jobs: NonZeroUsize,
    session_options: scoopc::session::SessionOptions,
) -> Result<()> {
    match run_for_exit_code(
        input,
        args,
        entry_package,
        profile,
        opt_level,
        incremental,
        jobs,
        session_options,
    )? {
        0 => Ok(()),
        code => std::process::exit(code),
    }
}

#[derive(Debug)]
enum RunInput {
    File(PathBuf),
    ConeRoot(PathBuf),
}

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn resolve_run_input(input: Option<PathBuf>) -> Result<RunInput> {
    match input {
        Some(path) => {
            let path = path
                .canonicalize()
                .into_diagnostic()
                .wrap_err("无法定位输入路径")?;

            if path.is_file() {
                return Ok(RunInput::File(path));
            }

            if path.is_dir() {
                let root = scoopc::cone::discover_cone_root(&path).ok_or_else(|| {
                    miette::miette!("目录不是 cone 项目（找不到 Cone.toml）：{}", path.display())
                })?;

                let root = root
                    .canonicalize()
                    .into_diagnostic()
                    .wrap_err("无法定位 cone root")?;
                return Ok(RunInput::ConeRoot(root));
            }

            Err(miette::miette!(
                "输入既不是文件也不是目录：{}",
                path.display()
            ))
        }
        None => {
            let cwd = std::env::current_dir()
                .into_diagnostic()
                .wrap_err("无法获取当前目录")?;

            let root = scoopc::cone::discover_cone_root(&cwd).ok_or_else(|| {
                miette::miette!(
                    "未指定输入：当前目录不是 cone 项目（找不到 Cone.toml）：{}",
                    cwd.display()
                )
            })?;

            let root = root
                .canonicalize()
                .into_diagnostic()
                .wrap_err("无法定位 cone root")?;
            Ok(RunInput::ConeRoot(root))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_for_exit_code(
    input: Option<PathBuf>,
    args: Vec<String>,
    entry_package: Option<String>,
    profile: super::build::BuildProfile,
    opt_level: Option<scoopc::opt::OptLevel>,
    incremental: bool,
    jobs: NonZeroUsize,
    session_options: scoopc::session::SessionOptions,
) -> Result<i32> {
    let input = resolve_run_input(input)?;
    match input {
        RunInput::File(input) => {
            let dir = super::temp::make_temp_dir("scoop_run")?;
            let _guard = TempDirGuard(dir.clone());
            let exe = dir.join(default_exe_name());

            // 复用 build 的“前端检查 +（可选）生成二进制”逻辑。
            super::build::run(
                input,
                Some(exe.to_path_buf()),
                super::build::BuildOptions {
                    emit: super::build::BuildEmit::Executable,
                    entry_package,
                    profile,
                    opt_level,
                    incremental,
                    jobs,
                    session_options,
                },
            )?;

            if !cfg!(feature = "llvm") {
                return Err(miette::miette!(
                    "子命令 `run` 需要启用 LLVM 后端（feature: llvm）：若你用了 `--no-default-features`，去掉它或加上 `--features llvm`"
                ));
            }

            if !exe.is_file() {
                return Err(miette::miette!(
                    "构建未生成可执行文件：{}（这通常表示 LLVM/clang 工具链不可用）",
                    exe.display()
                ));
            }

            run_executable(&exe, args)
        }
        RunInput::ConeRoot(cone_root) => {
            let manifest = scoopc::cone::load_cone_manifest_from_dir(&cone_root)?;
            let exe = super::build::layout::cone_exe_path(
                &cone_root,
                None,
                profile.as_str(),
                &manifest.cone.name,
            );

            // T1123 v0：允许 always rebuild（与 CONE-IMPROVEMENTS.md §4.1 对齐）。
            super::build::run(
                cone_root.clone(),
                Some(exe.to_path_buf()),
                super::build::BuildOptions {
                    emit: super::build::BuildEmit::Executable,
                    entry_package,
                    profile,
                    opt_level,
                    incremental,
                    jobs,
                    session_options,
                },
            )?;

            if !cfg!(feature = "llvm") {
                return Err(miette::miette!(
                    "子命令 `run` 需要启用 LLVM 后端（feature: llvm）：若你用了 `--no-default-features`，去掉它或加上 `--features llvm`"
                ));
            }

            if !exe.is_file() {
                return Err(miette::miette!(
                    "构建未生成可执行文件：{}（这通常表示 LLVM/clang 工具链不可用）",
                    exe.display()
                ));
            }

            run_executable(&exe, args)
        }
    }
}

fn run_executable(exe: &Path, args: Vec<String>) -> Result<i32> {
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

    use std::num::NonZeroUsize;

    fn default_jobs_for_test() -> NonZeroUsize {
        super::super::build::concurrency::default_build_jobs()
    }

    #[cfg(not(feature = "llvm"))]
    #[test]
    fn run_requires_llvm_feature() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        let err = super::run_for_exit_code(
            Some(input),
            Vec::new(),
            None,
            super::super::build::BuildProfile::Debug,
            None,
            true,
            default_jobs_for_test(),
            scoopc::session::SessionOptions::new(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("需要启用 LLVM"),
            "应提示开启 llvm feature，实际：{err}"
        );
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn run_builds_and_executes_minimal_main() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        let code = super::run_for_exit_code(
            Some(input),
            Vec::new(),
            None,
            super::super::build::BuildProfile::Debug,
            None,
            true,
            default_jobs_for_test(),
            scoopc::session::SessionOptions::new(),
        )
        .unwrap();
        assert_eq!(code, 0, "最小 main 应返回 0");
    }
}
