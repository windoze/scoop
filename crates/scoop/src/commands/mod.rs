//! `scoop` CLI 子命令实现入口。
//!
//! 说明：
//! - 这里的职责是将 CLI 命令分发到具体子模块。
//! - 真实编译器逻辑在 `scoopc` crate 中实现；driver 只负责 I/O、调度与输出。

mod dump_ast;
mod test;

use tracing_subscriber::EnvFilter;

use crate::cli::{Args, Command};

/// 初始化日志系统。
///
/// 通过 `RUST_LOG=scoop=debug` 控制输出。
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

pub fn dispatch(args: Args) -> Result<(), miette::Report> {
    match args.command {
        Command::Test { fixtures } => test::run(fixtures),
        Command::DumpAst { input } => dump_ast::run(input),
        Command::Build { .. } => Err(miette::miette!(
            "子命令 `build` 尚未实现；当前仅提供工程骨架。"
        )),
        Command::Run { .. } => Err(miette::miette!(
            "子命令 `run` 尚未实现；当前仅提供工程骨架。"
        )),
    }
}

