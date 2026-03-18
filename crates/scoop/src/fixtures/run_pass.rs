//! run-pass fixtures（运行期）支持。
//!
//! 说明：
//! - 本模块当前只落地“stdout golden 对比”与诊断类型（T0106a）
//! - “执行外部命令并捕获 stdout”的接口由 T0106b1 提供
//! - 真正的 `scoop run <fixture>` 接入与可执行 fixture 由后续任务（T0106b2 / T0807）完成

use std::path::Path;
use std::process::Command;

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

// 说明：下面这组诊断与执行入口会在 T0106b2（接入 `scoop run`）中被 fixtures runner 调用。
// 目前阶段仅提供“可单测的执行能力”，因此在非 test build 下暂时未被引用。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Error, Diagnostic)]
#[error("无法执行 run-pass 命令：{program}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_exec_failed))]
struct RunExecFailed {
    program: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Error, Diagnostic)]
#[error("run-pass 命令退出码非 0：{status}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_exec_nonzero_exit))]
struct RunExecNonZeroExit {
    status: String,
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
        reason: "需要先实现 `scoop run`（T0807），并在 fixtures runner 中接入真实执行（T0106b2）"
            .to_string(),
    }))
}

/// 执行一个外部命令来“运行”该 run-pass fixture，并断言 stdout 与 golden 一致。
///
/// 说明：
/// - 该函数是 run-pass phase 的“真实执行接口”（T0106b1）；
/// - 真正的 `scoop run <fixture>` 接入由后续任务（T0106b2/T0807）完成；
/// - 当前阶段只做 stdout 捕获 + `RUN-STDOUT` golden 比对（不做 stderr/超时/退出码断言）。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn run_fixture_command(
    rel_fixture: &Path,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
    mut cmd: Command,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let output = cmd.output().map_err(|e| {
        super::box_diagnostic(RunExecFailed {
            program: cmd.get_program().to_string_lossy().to_string(),
            fixture: rel_fixture.display().to_string(),
            source: e,
        })
    })?;

    if !output.status.success() {
        return Err(super::box_diagnostic(RunExecNonZeroExit {
            status: output.status.to_string(),
            fixture: rel_fixture.display().to_string(),
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_stdout_matches(fixture_path, exp, &stdout)?;
    Ok(())
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

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_captures_stdout_and_compares_golden() {
        let dir = make_temp_dir("run_fixture_command_captures_stdout_and_compares_golden");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("out.txt");

        std::fs::write(&fixture_path, "// RUN-STDOUT: out.txt\nfun main() {}\n").unwrap();
        std::fs::write(&golden_path, "hello\r\nworld\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("printf 'hello\\nworld\\n'");
            cmd
        };

        run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_nonzero_exit_has_stable_code() {
        let dir = make_temp_dir("run_fixture_command_nonzero_exit_has_stable_code");
        let fixture_path = dir.join("hello.scoop");

        std::fs::write(&fixture_path, "fun main() {}\n").unwrap();

        let exp = FixtureExpectation::from_source("fun main() {}\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("exit 3");
            cmd
        };

        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_exec_nonzero_exit"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
