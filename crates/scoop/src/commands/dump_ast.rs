//! `scoop dump-ast` 子命令。
//!
//! 目前阶段还没有完整的 parser，因此该命令暂时只验证：
//! - 能读取源文件
//! - 能输出基础信息
//!
//! 后续会替换为真正的 AST/HIR dump。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

pub fn run(input: PathBuf) -> Result<()> {
    let input = input.canonicalize().into_diagnostic().wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    println!("path: {}", file.path().display());
    println!("bytes: {}", file.text().len());
    println!("lines: {}", file.text().lines().count());
    Ok(())
}
