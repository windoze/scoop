//! run-pass fixtures（运行期）支持。
//!
//! 说明：
//! - 本模块落地“stdout/stderr golden 对比”与稳定诊断（T0106a/T0111a）。
//! - run-pass phase 的默认执行方式为：通过 `scoop run <fixture>` 作为子进程真正执行（T0106b2）。
//! - 由于 `scoop run` 需要启用 `scoop` 的 `llvm` feature：若当前未启用，则仅校验 golden 文件可读并跳过执行。

use std::path::Path;
use std::process::Command;

use miette::Diagnostic;
use thiserror::Error;

use super::expectations::FixtureExpectation;

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

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 stderr golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stderr_read_failed))]
struct RunStderrReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("stderr 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stderr_mismatch))]
struct RunStderrMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法定位当前 scoop 可执行文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_locate_scoop_failed))]
struct RunLocateScoopFailed {
    fixture: String,
    #[source]
    source: std::io::Error,
}

// 说明：下面这组诊断与执行入口会在 fixtures runner 的 run-pass phase 中被调用；
// `run_fixture_command` 同时也用于单测（可注入外部命令）。
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

pub(crate) fn run_fixture(
    rel_fixture: &Path,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // `scoop run` 需要 feature-gated 的 LLVM 后端。为保持在未安装 LLVM 的环境仍可跑 `scoop test`，
    // 当未启用 `--features llvm` 时，这里仅校验 golden 文件可读并跳过执行。
    if !cfg!(feature = "llvm") {
        validate_golden_files_readable(fixture_path, exp)?;
        // 重要：CI 默认不启用 `scoop` 的 `llvm` feature，因此 run-pass fixtures 通常会被“跳过执行”。
        //
        // 但对于 `EXPECT: fail` 的 run-pass fixtures，我们仍希望在不依赖 LLVM 的情况下，能够回归：
        // - stdout/stderr mismatch 的稳定错误码；
        // - 同时断言 stdout/stderr 时，stderr mismatch 能被区分出来（见 T0111b）。
        //
        // 因此这里对“期望失败”的 case 做一个可预测的模拟：把实际 stdout/stderr 视为“空输出”，
        // 然后复用同一套 golden 对比逻辑生成诊断。
        //
        // 说明：这不会影响 `EXPECT: pass` 的 fixtures；它们依旧是“仅校验 golden 文件可读”。
        if matches!(exp.expect, super::expectations::Expect::Fail) {
            assert_stdout_matches(fixture_path, exp, "")?;
            assert_stderr_matches(fixture_path, exp, "")?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| {
        super::box_diagnostic(RunLocateScoopFailed {
            fixture: rel_fixture.display().to_string(),
            source: e,
        })
    })?;

    let mut cmd = Command::new(exe);
    cmd.arg("run").arg(fixture_path);
    run_fixture_command(rel_fixture, fixture_path, exp, cmd)
}

fn validate_golden_files_readable(
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    if let Some(golden_rel) = exp.run_stdout {
        let golden_path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(golden_rel);

        std::fs::read_to_string(&golden_path).map_err(|e| {
            super::box_diagnostic(RunStdoutReadFailed {
                path: golden_path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;
    }

    if let Some(golden_rel) = exp.run_stderr {
        let golden_path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(golden_rel);

        std::fs::read_to_string(&golden_path).map_err(|e| {
            super::box_diagnostic(RunStderrReadFailed {
                path: golden_path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;
    }

    Ok(())
}

/// 执行一个外部命令来“运行”该 run-pass fixture，并断言 stdout 与 golden 一致。
///
/// 说明：
/// - 该函数是 run-pass phase 的“真实执行接口”（T0106b1）；
/// - `run_fixture` 默认通过 `scoop run <fixture>` 接入该能力；
/// - 当前阶段只做 stdout/stderr 捕获 + golden 比对（不做超时/退出码断言）。
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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_stdout_matches(fixture_path, exp, &stdout)?;
    assert_stderr_matches(fixture_path, exp, &stderr)?;
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

/// 断言 stderr 与 golden 文件一致（按 `RUN-STDERR` 指令）。
///
/// - 若 fixture 未提供 `RUN-STDERR`，则不做断言直接通过（保留“仅验证能跑”的用例空间）。
/// - 比对时会做换行归一化（`\r\n` → `\n`），避免跨平台差异。
pub(crate) fn assert_stderr_matches(
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
    actual_stderr: &str,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let Some(golden_rel) = exp.run_stderr else {
        return Ok(());
    };

    let golden_path = fixture_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(golden_rel);

    let expected = std::fs::read_to_string(&golden_path).map_err(|e| {
        super::box_diagnostic(RunStderrReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;

    let expected = super::normalize_newlines(&expected);
    let actual = super::normalize_newlines(actual_stderr);

    if expected != actual {
        return Err(super::box_diagnostic(RunStderrMismatch {
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
    fn stderr_golden_matches_ok() {
        let dir = make_temp_dir("stderr_golden_matches_ok");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("err.txt");

        std::fs::write(&fixture_path, "// RUN-STDERR: err.txt\nfun main() {}\n").unwrap();
        std::fs::write(&golden_path, "hello\r\nworld\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDERR: err.txt\n");
        assert_stderr_matches(&fixture_path, &exp, "hello\nworld\n").unwrap();

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

    #[test]
    fn stderr_golden_mismatch_has_stable_code() {
        let dir = make_temp_dir("stderr_golden_mismatch_has_stable_code");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("err.txt");

        std::fs::write(&fixture_path, "// RUN-STDERR: err.txt\nfun main() {}\n").unwrap();
        std::fs::write(&golden_path, "expected\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDERR: err.txt\n");
        let err = assert_stderr_matches(&fixture_path, &exp, "actual\n").unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_stderr_mismatch"
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
    fn run_fixture_command_captures_stderr_and_compares_golden() {
        let dir = make_temp_dir("run_fixture_command_captures_stderr_and_compares_golden");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("err.txt");

        std::fs::write(&fixture_path, "// RUN-STDERR: err.txt\nfun main() {}\n").unwrap();
        std::fs::write(&golden_path, "hello\r\nworld\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDERR: err.txt\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("printf 'hello\\nworld\\n' 1>&2");
            cmd
        };

        run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_both_streams_stderr_mismatch_is_distinguishable() {
        let dir = make_temp_dir("run_fixture_command_both_streams_stderr_mismatch");
        let fixture_path = dir.join("hello.scoop");
        let stdout_golden_path = dir.join("out.txt");
        let stderr_golden_path = dir.join("err.txt");

        std::fs::write(
            &fixture_path,
            "// RUN-STDOUT: out.txt\n// RUN-STDERR: err.txt\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(&stdout_golden_path, "ok\n").unwrap();
        std::fs::write(&stderr_golden_path, "expected\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n// RUN-STDERR: err.txt\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("printf 'ok\\n'; printf 'actual\\n' 1>&2");
            cmd
        };

        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_stderr_mismatch"
        );

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
