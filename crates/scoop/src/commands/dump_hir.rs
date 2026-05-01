//! `scoop dump-hir` 子命令。
//!
//! 当前阶段（TODO T0701）：输出 HIR 的 Debug 视图，用于后续 HIR/MIR/LLVM 迭代调试。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};
use scoopc::session::SessionOptions;

/// 读取输入文件并打印 HIR（Debug）。
pub fn run(input: PathBuf, session_options: SessionOptions) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::with_options(session_options)?;
    let lowered = scoopc::hir::lower_for_dump(&session, &file).map_err(miette::Report::from)?;
    println!("{:#?}", lowered.file);
    Ok(())
}
