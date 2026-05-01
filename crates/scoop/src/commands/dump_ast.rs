//! `scoop dump-ast` 子命令。
//!
//! 当前阶段：输出“早期 AST”（文件头 + 顶层声明 + `Block { stmts }` 的最小语句子集）。
//! 后续会逐步扩展为完整 AST/HIR dump。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    let ast = session.parse(&file).map_err(miette::Report::from)?;
    println!("{ast:#?}");
    Ok(())
}
