//! `scoop dump-rtti` 子命令。
//!
//! 当前阶段（TODO T1206）：输出 RTTI 的稳定视图（JSON），用于调试/回归。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 读取输入文件并打印 RTTI（v0：type id + struct 字段布局）。
pub fn run(input: PathBuf, type_name: Option<String>) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::new()?;
    if let Some(name) = type_name {
        let rtti =
            scoopc::rtti::dump_type_rtti(&session, &file, &name).map_err(miette::Report::from)?;
        println!("{}", serde_json::to_string_pretty(&rtti).into_diagnostic()?);
        return Ok(());
    }

    let dump = scoopc::rtti::dump_file_rtti(&session, &file).map_err(miette::Report::from)?;
    println!("{}", serde_json::to_string_pretty(&dump).into_diagnostic()?);
    Ok(())
}
