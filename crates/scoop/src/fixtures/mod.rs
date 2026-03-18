//! Fixtures 运行器（`scoop test`）。
//!
//! 设计目标：
//! - fixtures 是 Scoop 实现正确性的“底座”，必须可长期维护
//! - 每个 `.scoop` 文件可通过注释声明期望（pass/fail、错误包含等）
//!
//! 当前阶段仅实现 parse-level fixtures（调用 `scoopc::parser::parse_file`）。

mod expectations;

use std::path::{Path, PathBuf};

use miette::{miette, Context as _, Diagnostic as _, IntoDiagnostic as _, Result};

use expectations::{Expect, FixtureExpectation};

pub fn run_all(fixtures_root: &Path) -> Result<usize> {
    let mut files = Vec::new();
    collect_scoop_files(fixtures_root, &mut files)?;

    if files.is_empty() {
        return Err(miette!(
            "fixtures 目录下未发现任何 .scoop 文件：{}",
            fixtures_root.display()
        ));
    }

    let mut ok = 0usize;
    for file in files {
        run_one(&file).wrap_err_with(|| format!("fixture 失败：{}", file.display()))?;
        ok += 1;
    }

    Ok(ok)
}

fn run_one(path: &Path) -> Result<()> {
    let source = scoopc::source::SourceFile::load(path)?;
    let exp = FixtureExpectation::from_source(source.text());

    let parsed = scoopc::parser::parse_file(&source);

    match (exp.expect, parsed) {
        (Expect::Pass, Ok(_)) => Ok(()),
        (Expect::Pass, Err(e)) => Err(miette!("期望通过，但解析失败：{e}")),
        (Expect::Fail, Ok(_)) => Err(miette!("期望失败，但解析成功")),
        (Expect::Fail, Err(e)) => {
            if let Some(expected_code) = exp.error_code {
                let actual_code = e.code().map(|c| c.to_string());
                if actual_code.as_deref() != Some(expected_code) {
                    return Err(miette!(
                        "错误码不匹配：期望 {expected_code:?}，实际为：{actual_code:?}"
                    ));
                }
            }

            if let Some((line, col)) = exp.error_at {
                let (actual_line, actual_col) = primary_label_line_col(&source, &e)?;
                if (actual_line, actual_col) != (line, col) {
                    return Err(miette!(
                        "错误位置不匹配：期望 {line}:{col}，实际为：{actual_line}:{actual_col}"
                    ));
                }
            }

            if let Some(needle) = exp.error_contains {
                let msg = e.to_string();
                if !msg.contains(needle) {
                    return Err(miette!(
                        "错误信息不匹配：期望包含 {needle:?}，实际为：{msg}"
                    ));
                }
            }
            Ok(())
        }
    }
}

fn primary_label_line_col(
    source: &scoopc::source::SourceFile,
    diag: &dyn miette::Diagnostic,
) -> Result<(usize, usize)> {
    let mut first = None;
    let mut primary = None;

    if let Some(labels) = diag.labels() {
        for l in labels {
            first.get_or_insert(l.offset());
            if l.primary() {
                primary = Some(l.offset());
                break;
            }
        }
    }

    let offset = primary.or(first).ok_or_else(|| miette!("诊断未提供 labels/span，无法断言错误位置"))?;
    source.offset_to_line_col(offset)
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}
