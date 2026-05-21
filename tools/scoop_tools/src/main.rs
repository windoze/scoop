//! Scoop 仓库内置工具（开发期）。
//!
//! 目标：
//! - 把 “规范/fixtures/实现” 三者联动起来，避免文档漂移
//! - 提供可在 CI 强制执行的一致性检查（check mode）

mod dependency_gate;
mod fixtures_matrix;
mod safepoint_baseline;
mod spec_fixtures;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use miette::{Context as _, Result};

#[derive(Debug, Parser)]
#[command(name = "scoop-tools", version, about = "Scoop repository tools")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 从 `SCOOP_FULL_SPEC.md` 抽取 doctest fixtures，并写入/检查 `tests/fixtures/spec_doctest`
    SpecFixtures {
        /// 运行模式：`sync`（写文件）或 `check`（仅检查一致性）
        #[arg(value_parser = ["sync", "check"])]
        mode: String,

        /// 在 `check` 模式下自动写回不一致文件（只改动受影响文件）
        #[arg(long)]
        fix: bool,

        /// 规范文件路径（默认：`SCOOP_FULL_SPEC.md`）
        #[arg(long, default_value = "SCOOP_FULL_SPEC.md")]
        spec: PathBuf,

        /// fixtures 根目录（默认：`tests/fixtures`）
        #[arg(long, default_value = "tests/fixtures")]
        fixtures_root: PathBuf,
    },

    /// 覆盖矩阵检查：按 spec 章节或 stdlib 领域统计 fixtures 的覆盖缺口（仅报告，不强制失败）
    FixturesMatrix {
        /// 运行模式：`check`（spec 章节覆盖）或 `stdlib`（stdlib 领域覆盖）
        #[arg(value_parser = ["check", "stdlib"])]
        mode: String,

        /// 规范文件路径（默认：`SCOOP_FULL_SPEC.md`；仅 `check` 模式使用）
        #[arg(long, default_value = "SCOOP_FULL_SPEC.md")]
        spec: PathBuf,

        /// fixtures 根目录（默认：`tests/fixtures`）
        #[arg(long, default_value = "tests/fixtures")]
        fixtures_root: PathBuf,
    },

    /// 生成当前 safepoint / gc-live roots 基线报告（自动构建内置 workload）
    SafepointBaseline,

    /// 检查 pipeline 基础 crate 没有反向依赖 facade/stage/fact crate
    DependencyGate,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::SpecFixtures {
            mode,
            fix,
            spec,
            fixtures_root,
        } => {
            let mode = match mode.as_str() {
                "sync" => spec_fixtures::Mode::Sync,
                "check" => spec_fixtures::Mode::Check,
                other => return Err(miette::miette!("未知 mode：{other}")),
            };

            let report = spec_fixtures::run(mode, fix, &spec, &fixtures_root)
                .wrap_err("spec fixtures 处理失败")?;

            if report.is_empty() {
                eprintln!("spec fixtures: no blocks found (ok)");
            } else {
                eprintln!("spec fixtures: ok ({})", report.len());
            }
        }

        Command::FixturesMatrix {
            mode,
            spec,
            fixtures_root,
        } => match mode.as_str() {
            "check" => {
                let report = fixtures_matrix::run_check(&spec, &fixtures_root)
                    .wrap_err("fixtures matrix 检查失败")?;
                eprintln!("{}", report.render());
            }
            "stdlib" => {
                let report = fixtures_matrix::run_stdlib_check(&fixtures_root)
                    .wrap_err("stdlib coverage 检查失败")?;
                eprintln!("{}", report.render());
            }
            other => return Err(miette::miette!("未知 mode：{other}")),
        },

        Command::SafepointBaseline => {
            let report = safepoint_baseline::run().wrap_err("safepoint baseline 生成失败")?;
            eprintln!("{report}");
        }

        Command::DependencyGate => {
            let report =
                dependency_gate::run().wrap_err("pipeline 基础/fact crate 依赖门禁失败")?;
            eprintln!("{}", report.render());
        }
    }

    Ok(())
}
