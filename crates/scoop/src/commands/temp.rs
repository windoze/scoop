//! `scoop` 子命令共享的小工具函数。
//!
//! 当前仅包含临时目录创建（供 `build/run` 复用）。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use miette::{Context as _, IntoDiagnostic as _, Result};

/// 创建一个进程级唯一的临时目录。
///
/// 设计目标：
/// - 避免依赖额外 crate（例如 `tempfile`），保持运行期依赖最小；
/// - 目录名包含 PID + 时间戳，降低并发冲突概率；
/// - 失败时提供结构化上下文，便于定位问题。
pub fn make_temp_dir(prefix: &str) -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .into_diagnostic()
        .wrap_err("系统时间异常")?
        .as_nanos();

    let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir)
        .into_diagnostic()
        .wrap_err("无法创建临时目录")?;
    Ok(dir)
}
