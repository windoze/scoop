//! Fixtures 运行器（`scoop test`）。
//!
//! 设计目标：
//! - fixtures 是 Scoop 实现正确性的“底座”，必须可长期维护
//! - 每个 `.scoop` 文件可通过注释声明期望（pass/fail、错误包含等）
//!
//! 当前阶段仅实现 parse-level fixtures（调用 `scoopc::parser::parse_file`）。

mod expectations;

use std::path::{Path, PathBuf};

use miette::{miette, Context as _, IntoDiagnostic as _, Result};

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

