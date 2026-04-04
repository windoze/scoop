//! `scoop dump-mir` 子命令。
//!
//! 当前阶段（TODO T0708）：输出 MIR 的 Debug 视图，用于验证 if/when lowering 生成的 CFG 形态。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 读取输入文件并打印 MIR（Debug）。
pub fn run(input: PathBuf) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::new()?;
    let lowered = scoopc::mir::lower_for_dump(&session, &file).map_err(miette::Report::from)?;
    println!("{:#?}", lowered.file);
    Ok(())
}
