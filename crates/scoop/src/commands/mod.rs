//! `scoop` CLI 子命令实现入口。
//!
//! 说明：
//! - 这里的职责是将 CLI 命令分发到具体子模块。
//! - 真实编译器逻辑在 `scoopc` crate 中实现；driver 只负责 I/O、调度与输出。

pub(crate) mod build;
mod dump_ast;
mod dump_hir;
mod dump_ir;
mod dump_mir;
mod dump_rtti;
mod package;
mod run;
pub(crate) mod temp;
mod test;

use tracing_subscriber::EnvFilter;

use crate::cli::{Args, Command};

/// 初始化日志系统。
///
/// 通过 `RUST_LOG=scoop=debug` 控制输出。
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // 约定：编译器/driver 日志走 stderr，避免污染 run-pass fixtures 的 stdout 断言。
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

pub fn dispatch(args: Args) -> Result<(), miette::Report> {
    match args.command {
        Command::Test { fixtures } => test::run(fixtures),
        Command::DumpAst { input } => dump_ast::run(input),
        Command::DumpHir { input } => dump_hir::run(input),
        Command::DumpMir { input } => dump_mir::run(input),
        Command::DumpIr { input } => dump_ir::run(input),
        Command::DumpRtti { input, type_name } => dump_rtti::run(input, type_name),
        Command::Build {
            input,
            output,
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

            build::run(input, output, build::BuildOptions { emit })
        }
        Command::Run { input } => run::run(input),
        Command::Package { input, output } => package::run(input, output),
    }
}
