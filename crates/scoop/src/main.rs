//! Scoop CLI (`scoop`)
//!
//! 目前阶段：提供工程骨架、子命令框架，以及编译/运行入口。
//! 编译器实现位于 `scoopc` crate。

mod cli;
mod commands;
mod compiler_tool;

use clap::Parser as _;

fn main() -> miette::Result<()> {
    commands::init_tracing();

    let args = cli::Args::parse();
    commands::dispatch(args)
}
