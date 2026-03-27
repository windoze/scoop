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

    /// 解析/resolve 输入并打印 MIR（早期实现：Debug 输出）
    DumpMir {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 解析/resolve/typecheck 输入并打印 IR（单态化实例的 MIR 视图）
    DumpIr {
        /// 输入源文件路径
        input: PathBuf,
    },

    /// 构建可执行文件（默认仅做前端检查；启用 `--features llvm` 后会生成二进制）
    Build {
        /// 输入源文件路径（.scoop）或包目录（包含 Cone.toml）
        input: PathBuf,
        /// 输出文件路径
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// 输出 LLVM IR（`.ll`）
        #[arg(long, conflicts_with_all = ["emit_obj", "emit_asm"])]
        emit_llvm: bool,

        /// 输出 object 文件（`.o` / `.obj`）
        #[arg(long, conflicts_with_all = ["emit_llvm", "emit_asm"])]
        emit_obj: bool,

        /// 输出汇编（`.s` / `.asm`）
        #[arg(long, conflicts_with_all = ["emit_llvm", "emit_obj"])]
        emit_asm: bool,
    },

    /// 运行程序（先 build 后 exec；需要启用 `--features llvm`）
    Run {
        /// 输入源文件路径（.scoop）或包目录（包含 Cone.toml）
        input: PathBuf,
    },

    /// 打包 cone 包为 `.cone` 归档（v0：只写包）
    Package {
        /// 输入包目录（包含 `Cone.toml`）
        input: PathBuf,
        /// 输出 `.cone` 文件路径
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}
