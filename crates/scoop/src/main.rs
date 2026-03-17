//! Scoop CLI (`scoop`)
//!
//! 目前阶段：提供工程骨架、子命令框架，以及最小的 fixtures runner。
//! 编译器实现位于 `scoopc` crate。

mod cli;
mod commands;
mod fixtures;

use clap::Parser as _;

fn main() -> miette::Result<()> {
    commands::init_tracing();

    let args = cli::Args::parse();
    commands::dispatch(args)
}
