//! `scoop test` 子命令。
//!
//! 早期阶段 fixtures runner 的目标是把框架搭起来：
//! - 能递归发现 `tests/fixtures/**/*.scoop`
//! - 能按文件头注释指令执行（pass/fail）
//!
//! 后续阶段会逐步扩展为：
//! - parse fixtures（AST snapshot / 错误恢复）
//! - typecheck fixtures（pass/fail）
//! - run-pass fixtures（stdout 对比）

use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

pub fn run(fixtures: Option<PathBuf>) -> Result<()> {
    let root = fixtures.unwrap_or_else(|| PathBuf::from("tests/fixtures"));
    let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "无法定位 fixtures 目录：{}（可用 --fixtures 指定）",
            root.display()
        )
    })?;

    let ok = crate::fixtures::run_all(&root)?;
    println!("fixtures: ok ({ok})");
    Ok(())
}
