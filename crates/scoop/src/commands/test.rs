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

use std::num::NonZeroU32;
use std::path::PathBuf;

use miette::{Context as _, IntoDiagnostic as _, Result};

#[derive(Debug, Clone, Copy, Default)]
pub struct TestOptions {
    pub opt_level: Option<scoopc::opt::OptLevel>,
    pub gc_stress: bool,
    pub gc_move: bool,
    pub threads: Option<NonZeroU32>,
}

pub fn run(fixtures: Option<PathBuf>, options: TestOptions) -> Result<()> {
    let root = fixtures.unwrap_or_else(|| PathBuf::from("tests/fixtures"));
    let root = root.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "无法定位 fixtures 路径：{}（可用 --fixtures 指定目录或单个 fixture）",
            root.display()
        )
    })?;

    let mut run_pass_env = crate::fixtures::RunPassEnvOverrides::new();
    if options.gc_stress {
        run_pass_env.set("SCOOP_GC_STRESS", "1");
    }
    if options.gc_move {
        run_pass_env.set("SCOOP_GC_MOVE", "1");
    }
    if let Some(threads) = options.threads {
        let v = threads.get().to_string();
        run_pass_env.set("SCOOP_GC_IMMIX_PARALLEL_MARK", v.clone());
        run_pass_env.set("SCOOP_GC_IMMIX_PARALLEL_SWEEP", v);
    }

    let ok = crate::fixtures::run_all(&root, options.opt_level, &run_pass_env)?;
    println!("fixtures: ok ({ok})");
    Ok(())
}
