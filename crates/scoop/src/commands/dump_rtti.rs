//! `scoop dump-rtti` 子命令。
//!
//! 当前阶段（TODO T1507b）：输出 type descriptor 的稳定视图（JSON），用于调试/回归。

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 读取输入文件并打印 RTTI/type descriptor（v0：type_id + parent chain + trace bitmap/trace_fn）。
pub fn run(input: PathBuf, type_name: Option<String>) -> Result<()> {
    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let file = scoopc::source::SourceFile::load(&input)?;

    let session = scoopc::session::Session::new()?;
    let dump = scoopc::rtti::type_desc::dump_file_type_desc(&session, &file)
        .map_err(miette::Report::from)?;

    if let Some(query) = type_name {
        // 1) 先尝试 exact match（descriptor canonical name）。
        if let Some(found) = dump.types.iter().find(|t| t.name == query) {
            println!("{}", serde_json::to_string_pretty(found).into_diagnostic()?);
            return Ok(());
        }

        // 2) fallback：simple name（最后一段），要求唯一。
        let mut by_simple: std::collections::BTreeMap<
            &str,
            Vec<&scoopc::rtti::type_desc::TypeDesc>,
        > = std::collections::BTreeMap::new();
        for ty in &dump.types {
            let simple = ty.name.rsplit('.').next().unwrap_or(ty.name.as_str());
            by_simple.entry(simple).or_default().push(ty);
        }

        if let Some(cands) = by_simple.get(query.as_str()) {
            if cands.len() == 1 {
                println!(
                    "{}",
                    serde_json::to_string_pretty(cands[0]).into_diagnostic()?
                );
                return Ok(());
            }
            return Err(miette::miette!(
                "类型名不唯一：{query}（候选：{}）",
                cands
                    .iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        return Err(miette::miette!("未知类型：{query}"));
    }

    println!("{}", serde_json::to_string_pretty(&dump).into_diagnostic()?);
    Ok(())
}
