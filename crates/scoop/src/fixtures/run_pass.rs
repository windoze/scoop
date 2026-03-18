//! run-pass fixtures（运行期）支持。
//!
//! 说明：
//! - 本模块当前只落地“stdout golden 对比”与诊断类型（T0106a）
//! - 真正的“编译 + 运行”由后续任务（T0106b / T0807）在 driver 侧接入

use std::path::Path;

use miette::Diagnostic;
use thiserror::Error;

use super::expectations::FixtureExpectation;

#[derive(Debug, Error, Diagnostic)]
#[error("run-pass fixtures 尚未启用（fixture: {fixture}）：{reason}")]
#[diagnostic(code(scoop::fixtures::run_pass_unimplemented))]
struct RunPassUnimplemented {
    fixture: String,
    reason: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 stdout golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stdout_read_failed))]
struct RunStdoutReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("stdout 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stdout_mismatch))]
struct RunStdoutMismatch {
    path: String,
    fixture: String,
}

/// run-pass phase 的占位实现。
///
/// 该 phase 依赖 `scoop run`（T0807）与 build/link/codegen pipeline，
/// 当前阶段先返回稳定诊断，便于先写 fixtures/runner 逻辑再补齐后端。
pub(crate) fn run_fixture_unimplemented(
    rel_fixture: &Path,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // 先验证 stdout golden 文件可读（若提供）。
    //
    // 这样即使 run-pass 的“真实执行”尚未接入，也能尽早发现 fixture 本身的路径错误。
    if let Some(golden_rel) = exp.run_stdout {
        let golden_path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(golden_rel);

        let expected = std::fs::read_to_string(&golden_path).map_err(|e| {
            super::box_diagnostic(RunStdoutReadFailed {
                path: golden_path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;

        // 用“golden 自身作为 stdout”跑一遍对比逻辑，确保：
        // - 换行归一化逻辑正确
        // - 诊断类型在非 test build 下也会被编译进来（避免 dead_code 警告）
        assert_stdout_matches(fixture_path, exp, &expected)?;
    }

    Err(super::box_diagnostic(RunPassUnimplemented {
        fixture: rel_fixture.display().to_string(),
        reason: "需要先实现 `scoop run`（T0807），并在 fixtures runner 中接入真实执行（T0106b）"
            .to_string(),
    }))
}

/// 断言 stdout 与 golden 文件一致（按 `RUN-STDOUT` 指令）。
///
/// - 若 fixture 未提供 `RUN-STDOUT`，则不做断言直接通过（保留“仅验证能跑”的用例空间）。
/// - 比对时会做换行归一化（`\r\n` → `\n`），避免跨平台差异。
pub(crate) fn assert_stdout_matches(
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
    actual_stdout: &str,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let Some(golden_rel) = exp.run_stdout else {
        return Ok(());
    };

    let golden_path = fixture_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(golden_rel);

    let expected = std::fs::read_to_string(&golden_path).map_err(|e| {
        super::box_diagnostic(RunStdoutReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;

    let expected = super::normalize_newlines(&expected);
    let actual = super::normalize_newlines(actual_stdout);

    if expected != actual {
        return Err(super::box_diagnostic(RunStdoutMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "scoop_{prefix}_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stdout_golden_matches_ok() {
        let dir = make_temp_dir("stdout_golden_matches_ok");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("out.txt");

        std::fs::write(&fixture_path, "// RUN-STDOUT: out.txt\nfun main() {}\n").unwrap();
        std::fs::write(&golden_path, "hello\r\nworld\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n");
        assert_stdout_matches(&fixture_path, &exp, "hello\nworld\n").unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stdout_golden_mismatch_has_stable_code() {
        let dir = make_temp_dir("stdout_golden_mismatch_has_stable_code");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("out.txt");

        std::fs::write(&fixture_path, "// RUN-STDOUT: out.txt\nfun main() {}\n").unwrap();
        std::fs::write(&golden_path, "expected\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n");
        let err = assert_stdout_matches(&fixture_path, &exp, "actual\n").unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_stdout_mismatch"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
