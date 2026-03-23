//! 命令行参数定义。
//!
//! 本模块只负责“解析参数 → 结构化配置”，不做具体业务逻辑。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "scoop", version, about = "Scoop compiler + tooling")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 运行 fixtures（当前阶段仅做最小 smoke）
    Test {
        /// fixtures 根目录（默认：`tests/fixtures`）
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },

    /// 解析输入并打印 AST（当前阶段输出为占位信息）
    DumpAst {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve 输入并打印 HIR（早期实现：Debug 输出）
    DumpHir {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 构建可执行文件（未实现）
    Build {
        /// 输入源文件路径
        input: PathBuf,
        /// 输出文件路径（未实现）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// 运行程序（未实现）
    Run {
        /// 输入源文件路径
        input: PathBuf,
    },
}
