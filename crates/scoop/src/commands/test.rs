//! `scoop test` 子命令。
//!
//! 早期阶段 fixtures runner 的目标是把框架搭起来：
//! - 能递归发现 `tests/fixtures/**/*.scoop`
//! - 能读取文件并做最小 smoke（目前只验证“能读”）
//!
//! 后续阶段会逐步扩展为：
//! - parse fixtures（AST snapshot / 错误恢复）
//! - typecheck fixtures（pass/fail）
//! - run-pass fixtures（stdout 对比）

use std::path::{Path, PathBuf};

use miette::{miette, Context as _, IntoDiagnostic as _, Result};

pub fn run(fixtures: Option<PathBuf>) -> Result<()> {
    let root = fixtures.unwrap_or_else(|| PathBuf::from("tests/fixtures"));
    let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "无法定位 fixtures 目录：{}（可用 --fixtures 指定）",
            root.display()
        )
    })?;

    let mut files = Vec::new();
    collect_scoop_files(&root, &mut files)?;

    if files.is_empty() {
        return Err(miette!(
            "fixtures 目录下未发现任何 .scoop 文件：{}",
            root.display()
        ));
    }

    let mut ok = 0usize;
    for file in files {
        scoopc::source::SourceFile::load(&file)
            .wrap_err_with(|| format!("读取 fixture 失败：{}", file.display()))?;
        ok += 1;
    }

    println!("fixtures: ok ({ok})");
    Ok(())
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
