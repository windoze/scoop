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
#[cfg(test)]
mod parity;
mod run;
pub(crate) mod temp;
mod test;

use tracing_subscriber::EnvFilter;

use crate::cli::{Args, Command};
use scoopc::opt::OptLevel;
use scoopc::session::SessionOptions;

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
    let Args {
        effect_pipeline,
        command,
    } = args;
    let session_options = SessionOptions::new(effect_pipeline);

    match command {
        Command::New { project_name } => new::run(project_name),
        Command::Test {
            fixtures,
            opt_level,
            gc_stress,
            gc_move,
            threads,
        } => test::run(
            fixtures,
            test::TestOptions {
                opt_level: parse_opt_level_flag(opt_level)?,
                gc_stress,
                gc_move,
                threads,
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
            build::run(
                input,
                output,
                build::BuildOptions {
                    emit,
                    entry_package,
                    profile,
                    opt_level: parse_opt_level_flag(opt_level)?,
                    incremental,
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
        } => {
            let profile = build::BuildProfile::from_debug_release_flags(debug, release);
            let incremental = resolve_incremental_enabled(no_incremental);
            run::run(
                input,
                args,
                entry_package,
                profile,
                parse_opt_level_flag(opt_level)?,
                incremental,
                session_options,
            )
        }
        Command::Package { input, output } => package::run(input, output, session_options),
    }
}

fn parse_opt_level_flag(opt_level: Option<String>) -> Result<Option<OptLevel>, miette::Report> {
    let Some(value) = opt_level else {
        return Ok(None);
    };

    let parsed = OptLevel::parse(&value).map_err(miette::Report::from)?;
    Ok(Some(parsed))
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
