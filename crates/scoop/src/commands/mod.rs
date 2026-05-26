//! `scoop` CLI 子命令实现入口。
//!
//! 说明：
//! - 这里的职责是将 CLI 命令分发到具体子模块。
//! - 真实编译器逻辑在 `scoopc` crate 中实现；driver 只负责 I/O、调度与输出。

pub(crate) mod build;
mod dump_ast;
pub(crate) mod dump_effect_facts;
pub(crate) mod dump_effect_lowered;
mod dump_hir;
mod dump_ir;
mod dump_mir;
mod dump_rtti;
mod dump_stackmaps;
mod new;
mod package;
mod run;
pub(crate) mod temp;
mod test;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use tracing_subscriber::EnvFilter;

use crate::cli::{Args, Command};
use scoop_project_model::OptLevel;

use build::concurrency::{self, BuildJobsError};

/// 初始化日志系统。
///
/// 通过 `RUST_LOG=scoop=debug` 控制输出。
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // 约定：编译器/driver 日志走 stderr，避免污染 run-pass fixtures 的 stdout 断言。
    // 显式检测 stderr 是否为终端，决定是否输出 ANSI 转义码：当 stderr 被 pipe（如 fixture
    // runner 捕获输出时）禁用 ANSI，避免 `RUN-STDERR-CONTAINS` 子串匹配因隐藏的转义码而失败。
    let use_ansi = std::io::IsTerminal::is_terminal(&std::io::stderr());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(use_ansi)
        .init();
}

pub fn dispatch(args: Args) -> Result<(), miette::Report> {
    let Args { command } = args;
    let session_options = FacadeSessionOptions::new();

    match command {
        Command::New { project_name } => new::run(project_name),
        Command::Test {
            fixture_path,
            fixtures,
            exit_on_failure,
            processes,
            opt_level,
            gc_stress,
            gc_move,
            threads,
        } => test::run(
            fixture_path.or(fixtures),
            test::TestOptions {
                opt_level: parse_opt_level_flag(opt_level)?,
                gc_stress,
                gc_move,
                threads,
                exit_on_failure,
                processes,
                session_options,
            },
        ),
        Command::DumpAst { input } => dump_ast::run(input, session_options),
        Command::DumpEffectFacts { input } => dump_effect_facts::run(input, session_options),
        Command::DumpEffectLowered { input } => dump_effect_lowered::run(input, session_options),
        Command::DumpHir { input } => dump_hir::run(input, session_options),
        Command::DumpMir { input } => dump_mir::run(input, session_options),
        Command::DumpIr { input } => dump_ir::run(input, session_options),
        Command::DumpRtti { input, type_name } => dump_rtti::run(input, type_name, session_options),
        Command::DumpStackmaps {
            input,
            verify_roots,
            dump_records,
        } => dump_stackmaps::run(input, verify_roots, dump_records),
        Command::Build {
            input,
            output,
            entry_package,
            debug,
            release,
            opt_level,
            no_incremental,
            emit_llvm,
            emit_obj,
            emit_asm,
            jobs,
            sysroot_dep,
        } => {
            let emit = if emit_llvm {
                build::BuildEmit::LlvmIr
            } else if emit_obj {
                build::BuildEmit::Obj
            } else if emit_asm {
                build::BuildEmit::Asm
            } else {
                build::BuildEmit::Executable
            };

            let profile = build::BuildProfile::from_debug_release_flags(debug, release);
            let incremental = resolve_incremental_enabled(no_incremental);
            let jobs = resolve_build_jobs(jobs)?;
            let session_options =
                session_options_with_cli_sysroot_deps(session_options, sysroot_dep)?;
            build::run(
                input,
                output,
                build::BuildOptions {
                    emit,
                    entry_package,
                    profile,
                    opt_level: parse_opt_level_flag(opt_level)?,
                    incremental,
                    jobs,
                    session_options,
                },
            )
        }
        Command::Run {
            input,
            entry_package,
            debug,
            release,
            opt_level,
            no_incremental,
            args,
            jobs,
            sysroot_dep,
        } => {
            let profile = build::BuildProfile::from_debug_release_flags(debug, release);
            let incremental = resolve_incremental_enabled(no_incremental);
            let jobs = resolve_build_jobs(jobs)?;
            let session_options =
                session_options_with_cli_sysroot_deps(session_options, sysroot_dep)?;
            run::run(
                input,
                args,
                entry_package,
                profile,
                parse_opt_level_flag(opt_level)?,
                incremental,
                jobs,
                session_options,
            )
        }
        Command::Package { input, output } => package::run(input, output, session_options),
    }
}

/// 把 CLI `--jobs N` 与环境变量 `SCOOP_BUILD_JOBS` 解析成最终的 [`NonZeroUsize`]。
///
/// 优先级 CLI > env > [`concurrency::DEFAULT_BUILD_JOBS`]；env 解析失败时返回结构化诊断。
fn resolve_build_jobs(cli_jobs: Option<NonZeroUsize>) -> Result<NonZeroUsize, miette::Report> {
    concurrency::resolve_build_jobs(cli_jobs).map_err(|err| match err {
        BuildJobsError::InvalidEnvValue { value } => miette::miette!(
            "环境变量 `{}` 的值 `{value}` 无效：必须是正整数（>=1）",
            concurrency::BUILD_JOBS_ENV_VAR
        ),
        BuildJobsError::EnvNotUnicode => miette::miette!(
            "环境变量 `{}` 不是有效的 UTF-8 字符串",
            concurrency::BUILD_JOBS_ENV_VAR
        ),
    })
}

fn parse_opt_level_flag(opt_level: Option<String>) -> Result<Option<OptLevel>, miette::Report> {
    let Some(value) = opt_level else {
        return Ok(None);
    };

    let parsed = OptLevel::parse(&value).map_err(miette::Report::from)?;
    Ok(Some(parsed))
}

pub(crate) const SYSROOT_DEPENDENCIES_ENV: &str = "SCOOP_SYSROOT_DEPS";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct FacadeSessionOptions {
    sysroot_overlay: Option<PathBuf>,
    extra_sysroot_dependencies: Vec<String>,
}

impl FacadeSessionOptions {
    pub(crate) const fn new() -> Self {
        Self {
            sysroot_overlay: None,
            extra_sysroot_dependencies: Vec::new(),
        }
    }

    pub(crate) fn with_extra_sysroot_dependencies<I, S>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extra_sysroot_dependencies
            .extend(dependencies.into_iter().map(Into::into));
        self.extra_sysroot_dependencies.sort();
        self.extra_sysroot_dependencies.dedup();
        self
    }

    pub(crate) fn with_env_fallback(mut self) -> Self {
        if self.sysroot_overlay.is_none() {
            self.sysroot_overlay = std::env::var_os(scoop_project_model::SYSROOT_OVERLAY_ENV)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from);
        }
        if self.extra_sysroot_dependencies.is_empty()
            && let Some(value) = std::env::var_os(SYSROOT_DEPENDENCIES_ENV)
            && let Some(value) = value.to_str()
        {
            self = self.with_extra_sysroot_dependencies(parse_sysroot_dependency_env(value));
        }
        self
    }

    pub(crate) fn sysroot_overlay(&self) -> Option<&Path> {
        self.sysroot_overlay.as_deref()
    }

    pub(crate) fn extra_sysroot_dependencies(&self) -> &[String] {
        &self.extra_sysroot_dependencies
    }

    pub(crate) fn apply_to_command(&self, cmd: &mut std::process::Command) {
        if let Some(overlay_root) = self.sysroot_overlay() {
            cmd.env(scoop_project_model::SYSROOT_OVERLAY_ENV, overlay_root);
        }
        if !self.extra_sysroot_dependencies().is_empty() {
            cmd.env(
                SYSROOT_DEPENDENCIES_ENV,
                self.extra_sysroot_dependencies().join(","),
            );
        }
    }
}

fn parse_sysroot_dependency_env(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
}

fn session_options_with_cli_sysroot_deps(
    session_options: FacadeSessionOptions,
    deps: Vec<String>,
) -> Result<FacadeSessionOptions, miette::Report> {
    if deps.is_empty() {
        return Ok(session_options);
    }
    let mut seen = std::collections::BTreeSet::new();
    for dep in &deps {
        let trimmed = dep.trim();
        if trimmed.is_empty() {
            return Err(miette::miette!("`--sysroot-dep` 不能是空字符串"));
        }
        if !seen.insert(trimmed.to_string()) {
            return Err(miette::miette!("重复的 `--sysroot-dep`：{trimmed}"));
        }
    }
    Ok(session_options.with_extra_sysroot_dependencies(seen))
}

fn resolve_incremental_enabled(no_incremental: bool) -> bool {
    if no_incremental {
        return false;
    }

    // 未启用 LLVM 后端时，build 会退化为“仅前端检查”，不应跳过（否则会让错误被缓存掩盖）。
    if !cfg!(feature = "llvm") {
        return false;
    }

    match std::env::var("SCOOP_INCREMENTAL") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "0" | "false" | "no" | "off")
        }
        Err(_) => true,
    }
}

pub(crate) fn run_compiler_passthrough<I, S>(
    args: I,
    session_options: &FacadeSessionOptions,
    label: &str,
) -> Result<(), miette::Report>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = crate::compiler_tool::command()?;
    cmd.args(args);
    session_options.apply_to_command(&mut cmd);
    let output = crate::compiler_tool::run_capture(cmd, label)?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    Ok(())
}
