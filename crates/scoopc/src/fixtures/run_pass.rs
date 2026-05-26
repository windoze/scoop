//! run-pass fixtures（运行期）支持。
//!
//! 说明：
//! - 本模块落地“stdout/stderr golden 对比”与稳定诊断（T0106a/T0111a）。
//! - run-pass phase 的默认执行方式为：通过 `scoop run <fixture>` 作为子进程真正执行（T0106b2）。
//! - 对于少量“工具链可观测性”相关用例，可通过 `// RUN-MODE: dump-stackmaps` 切换为运行
//!   `scoop dump-stackmaps <bin>`（T1503a2）。
//! - 由于 `scoop run` 需要启用 `scoop` 的 `llvm` feature：若当前未启用，则仅校验 golden 文件可读并跳过执行。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use miette::Diagnostic;
use thiserror::Error;

use super::expectations::FixtureExpectation;

#[cfg(unix)]
use std::ffi::c_int;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

#[cfg(unix)]
const SIGKILL: c_int = 9;

#[cfg(unix)]
const ESRCH: i32 = 3;

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: c_int, sig: c_int) -> c_int;
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
#[error(
    "stdout 与 golden 不一致：{path}（fixture: {fixture}；expected: {expected_preview}；actual: {actual_preview}）"
)]
#[diagnostic(code(scoop::fixtures::run_stdout_mismatch))]
struct RunStdoutMismatch {
    path: String,
    fixture: String,
    expected_preview: String,
    actual_preview: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("stdout 未包含期望子串：{substring}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stdout_missing_substring))]
struct RunStdoutMissingSubstring {
    substring: String,
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
#[error("无法读取 stdin 输入文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stdin_read_failed))]
struct RunStdinReadFailed {
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
#[error("stderr 未包含期望子串：{substring}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_stderr_missing_substring))]
struct RunStderrMissingSubstring {
    substring: String,
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

#[derive(Debug, Error, Diagnostic)]
#[error("dump-stackmaps 输出缺少 records 字段（fixture: {fixture}；stdout: {stdout_preview}）")]
#[diagnostic(code(scoop::fixtures::run_stackmaps_missing_records))]
struct RunStackmapsMissingRecords {
    fixture: String,
    stdout_preview: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "dump-stackmaps records 数量不符合期望：期望 > {expected}，实际为 {actual}（fixture: {fixture}）"
)]
#[diagnostic(code(scoop::fixtures::run_stackmaps_records_not_gt))]
struct RunStackmapsRecordsNotGt {
    expected: u32,
    actual: u32,
    fixture: String,
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Error, Diagnostic)]
#[error("run-pass 命令超时：{timeout_ms}ms（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_exec_timeout))]
struct RunExecTimeout {
    timeout_ms: u64,
    fixture: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Error, Diagnostic)]
#[error("run-pass 命令被信号终止：{signal}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_exec_signaled))]
struct RunExecSignaled {
    signal: String,
    fixture: String,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Error, Diagnostic)]
#[error("run-pass 命令退出码不符合期望：期望 {expected}，实际为 {actual}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_exit_code_mismatch))]
struct RunExitCodeMismatch {
    expected: i32,
    actual: i32,
    fixture: String,
}

pub(crate) fn run_fixture(
    rel_fixture: &Path,
    fixture_path: &Path,
    opt_level: Option<scoopc::opt::OptLevel>,
    session_options: scoopc::session::SessionOptions,
    exp: &FixtureExpectation<'_>,
    run_pass_env: &super::RunPassEnvOverrides,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // `scoop run` 需要 LLVM 后端。为保持在未安装 LLVM 的环境仍可跑 `scoop test`（例如用
    // `--no-default-features` 构建），当未启用 LLVM 时这里仅校验 golden 文件可读并跳过执行。
    if !cfg!(feature = "llvm") {
        validate_golden_files_readable(fixture_path, exp)?;
        // 重要：即使在未启用 LLVM 的构建里，run-pass fixtures 仍应能回归 runner 的稳定诊断行为。
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
            if let Some(timeout_ms) = exp.timeout_ms {
                return Err(super::box_diagnostic(RunExecTimeout {
                    timeout_ms,
                    fixture: rel_fixture.display().to_string(),
                }));
            }

            if let Some(expected) = exp.expect_exit {
                // 说明：未启用 LLVM 时我们无法真正执行 `scoop run`，因此这里用一个稳定且可预测的
                // “空执行”模型来回归 runner 的诊断行为：假设实际退出码为 0。
                let actual = 0;
                if actual != expected {
                    return Err(super::box_diagnostic(RunExitCodeMismatch {
                        expected,
                        actual,
                        fixture: rel_fixture.display().to_string(),
                    }));
                }
            }

            assert_stdout_matches(fixture_path, exp, "")?;
            assert_stderr_matches(fixture_path, exp, "")?;
        }
        return Ok(());
    }

    let scoop_exe = super::current_scoop_exe_path().map_err(|e| {
        super::box_diagnostic(RunLocateScoopFailed {
            fixture: rel_fixture.display().to_string(),
            source: e,
        })
    })?;

    let mode = exp.run_mode.unwrap_or("run");
    match mode {
        // 默认模式：真正运行该 fixture 对应的 Scoop 程序。
        "run" => {
            let cmd = build_run_mode_command(
                scoop_exe,
                fixture_path,
                opt_level,
                session_options,
                exp,
                run_pass_env,
            );
            run_fixture_command(rel_fixture, fixture_path, exp, cmd)
        }
        // 工具链可观测性：构建该 fixture 的可执行文件，然后运行 `scoop dump-stackmaps <bin>`。
        "dump-stackmaps" => run_fixture_dump_stackmaps(
            rel_fixture,
            fixture_path,
            exp,
            run_pass_env,
            scoop_exe,
            session_options,
        ),
        other => Err(super::box_diagnostic(super::UnimplementedPhase {
            phase: format!("run-pass/{other}"),
            fixture: rel_fixture.display().to_string(),
        })),
    }
}

fn build_run_mode_command(
    scoop_exe: PathBuf,
    fixture_path: &Path,
    opt_level: Option<scoopc::opt::OptLevel>,
    session_options: scoopc::session::SessionOptions,
    exp: &FixtureExpectation<'_>,
    run_pass_env: &super::RunPassEnvOverrides,
) -> Command {
    let mut cmd = Command::new(scoop_exe);
    cmd.arg("run");
    if let Some(level) = opt_level {
        cmd.arg("--opt-level").arg(level.as_str());
    }
    cmd.arg(fixture_path);
    super::apply_session_options_to_command(&session_options, &mut cmd);
    run_pass_env.apply_to_command(&mut cmd);
    // 约定：run-pass fixtures 可通过 `// ARGS: ...` 向 `scoop run` 透传参数（最终作为程序 argv）。
    if !exp.args.is_empty() {
        cmd.args(&exp.args);
    }
    cmd
}

fn run_fixture_dump_stackmaps(
    rel_fixture: &Path,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
    run_pass_env: &super::RunPassEnvOverrides,
    scoop_exe: PathBuf,
    session_options: scoopc::session::SessionOptions,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // 说明：该模式不执行程序本身，只用于验证“编译产物包含可读 stackmaps”。
    let dir = make_temp_dir("scoop_fixture_dump_stackmaps").map_err(|e| {
        super::box_diagnostic(RunExecFailed {
            program: "make_temp_dir".to_string(),
            fixture: rel_fixture.display().to_string(),
            source: e,
        })
    })?;
    let exe_path = dir.join(default_exe_name());

    let result = (|| {
        let mut build_cmd = Command::new(&scoop_exe);
        build_cmd
            .arg("build")
            .arg(fixture_path)
            .arg("-o")
            .arg(&exe_path);
        super::apply_session_options_to_command(&session_options, &mut build_cmd);
        let build_output = build_cmd.output().map_err(|e| {
            super::box_diagnostic(RunExecFailed {
                program: "scoop build".to_string(),
                fixture: rel_fixture.display().to_string(),
                source: e,
            })
        })?;
        if !build_output.status.success() {
            return Err(super::diagnostic_from_subprocess_output(
                "scoop build",
                rel_fixture,
                build_output,
            ));
        }

        let mut cmd = build_dump_stackmaps_command(scoop_exe, &exe_path, session_options);
        run_pass_env.apply_to_command(&mut cmd);
        run_fixture_command(rel_fixture, fixture_path, exp, cmd)
    })();

    // 清理临时目录（尽力而为；不影响最终结果）。
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn make_temp_dir(prefix: &str) -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir();
    for attempt in 0..1000u32 {
        let candidate = base.join(format!(
            "{prefix}_{}_{}_{attempt}",
            std::process::id(),
            current_time_nanos()
        ));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to allocate temporary directory",
    ))
}

fn current_time_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn build_dump_stackmaps_command(
    scoop_exe: PathBuf,
    exe_path: &Path,
    _session_options: scoopc::session::SessionOptions,
) -> Command {
    let mut cmd = Command::new(scoop_exe);
    // GC-FIX Phase E2：`dump-stackmaps` 作为 GC 调试主工具时应默认可校验 roots slot 契约。
    cmd.arg("dump-stackmaps")
        .arg("--verify-roots")
        .arg(exe_path);
    cmd
}

/// 与 `scoop run` 保持一致的默认可执行文件名（用于临时产物）。
fn default_exe_name() -> String {
    let ext = std::env::consts::EXE_EXTENSION;
    if ext.is_empty() {
        "a.out".to_string()
    } else {
        format!("a.{ext}")
    }
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

    if let Some(stdin_rel) = exp.run_stdin {
        let stdin_path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(stdin_rel);

        std::fs::read(&stdin_path).map_err(|e| {
            super::box_diagnostic(RunStdinReadFailed {
                path: stdin_path.display().to_string(),
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
/// - 当前阶段支持 stdout/stderr 捕获 + golden 比对，并可选启用退出码断言与超时控制（T0112）。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn run_fixture_command(
    rel_fixture: &Path,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
    mut cmd: Command,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    for (key, value) in &exp.env {
        cmd.env(key, value);
    }

    // 若 fixture 指定了 stdin 文件，则在执行前读取并原样写入子进程 stdin（随后关闭）。
    let stdin_bytes = if let Some(stdin_rel) = exp.run_stdin {
        let stdin_path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(stdin_rel);

        Some(std::fs::read(&stdin_path).map_err(|e| {
            super::box_diagnostic(RunStdinReadFailed {
                path: stdin_path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?)
    } else {
        None
    };

    let output = run_command_collect_output(rel_fixture, exp, &mut cmd, stdin_bytes.as_deref());
    cleanup_single_file_virtual_cone(fixture_path);
    let output = output?;
    assert_exit_status_matches(rel_fixture, exp, output.status)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_stackmaps_records_matches(rel_fixture, exp, &stdout)?;
    assert_stdout_matches(fixture_path, exp, &stdout)?;
    assert_stderr_matches(fixture_path, exp, &stderr)?;
    Ok(())
}

fn cleanup_single_file_virtual_cone(fixture_path: &Path) {
    if !fixture_path.is_file() {
        return;
    }
    let parent = fixture_path.parent().unwrap_or_else(|| Path::new("."));
    let root = parent
        .join("build")
        .join("debug")
        .join("virtual")
        .join(format!("{}@0.0.0", fixture_virtual_cone_name(fixture_path)));
    if root.exists() {
        let _ = std::fs::remove_dir_all(root);
    }
}

fn fixture_virtual_cone_name(input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("virtual-cone");
    let mut out = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "virtual-cone".to_string()
    } else {
        out
    }
}

fn assert_stackmaps_records_matches(
    rel_fixture: &Path,
    exp: &FixtureExpectation<'_>,
    actual_stdout: &str,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // 仅当 fixture 显式要求检查 records 数量时才启用该断言，避免影响普通 run-pass fixtures。
    let Some(min) = exp.run_stackmaps_records_gt else {
        return Ok(());
    };

    let actual = parse_records_from_dump_stackmaps_stdout(actual_stdout).ok_or_else(|| {
        super::box_diagnostic(RunStackmapsMissingRecords {
            fixture: rel_fixture.display().to_string(),
            stdout_preview: preview_text(actual_stdout, 200),
        })
    })?;

    if actual <= min {
        return Err(super::box_diagnostic(RunStackmapsRecordsNotGt {
            expected: min,
            actual,
            fixture: rel_fixture.display().to_string(),
        }));
    }

    Ok(())
}

fn parse_records_from_dump_stackmaps_stdout(stdout: &str) -> Option<u32> {
    // 输出格式由 `scoop dump-stackmaps` 约定：
    // - 包含一行 `records: <n>`
    for line in stdout.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("records:") {
            return rest.trim().parse::<u32>().ok();
        }
    }
    None
}

fn preview_text(s: &str, max_chars: usize) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "<empty>".to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    if s.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// 运行外部命令后收集到的输出（stdout/stderr + 退出状态）。
#[derive(Debug)]
struct CommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// 执行 `cmd` 并捕获 stdout/stderr，可选启用超时。
///
/// - 该函数不负责“退出码是否符合期望”的判定（由 `assert_exit_status_matches` 完成）。
/// - 若发生超时，会尽力 kill 子进程组并回收（wait）后返回 `RunExecTimeout`。
fn run_command_collect_output(
    rel_fixture: &Path,
    exp: &FixtureExpectation<'_>,
    cmd: &mut Command,
    stdin_bytes: Option<&[u8]>,
) -> std::result::Result<CommandOutput, Box<dyn miette::Diagnostic>> {
    let program = cmd.get_program().to_string_lossy().to_string();

    if stdin_bytes.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    if exp.timeout_ms.is_some() {
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| {
        super::box_diagnostic(RunExecFailed {
            program,
            fixture: rel_fixture.display().to_string(),
            source: e,
        })
    })?;

    let child_stdout = child.stdout.take().ok_or_else(|| {
        super::box_diagnostic(RunExecFailed {
            program: cmd.get_program().to_string_lossy().to_string(),
            fixture: rel_fixture.display().to_string(),
            source: std::io::Error::other("stdout 未被配置为 piped"),
        })
    })?;
    let child_stderr = child.stderr.take().ok_or_else(|| {
        super::box_diagnostic(RunExecFailed {
            program: cmd.get_program().to_string_lossy().to_string(),
            fixture: rel_fixture.display().to_string(),
            source: std::io::Error::other("stderr 未被配置为 piped"),
        })
    })?;

    if let Some(stdin_bytes) = stdin_bytes {
        let mut child_stdin = child.stdin.take().ok_or_else(|| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: std::io::Error::other("stdin 未被配置为 piped"),
            })
        })?;
        child_stdin.write_all(stdin_bytes).map_err(|e| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: e,
            })
        })?;
        // 关闭 stdin：确保被测程序能够观察到 EOF，并避免死等更多输入。
        drop(child_stdin);
    }

    let stdout_thread = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut stdout = child_stdout;
        stdout.read_to_end(&mut buf)?;
        Ok(buf)
    });
    let stderr_thread = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut stderr = child_stderr;
        stderr.read_to_end(&mut buf)?;
        Ok(buf)
    });

    let timeout = exp.timeout_ms.map(Duration::from_millis);
    let (status, timed_out) =
        wait_child_with_optional_timeout(&mut child, timeout).map_err(|e| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: e,
            })
        })?;

    let stdout = stdout_thread
        .join()
        .map_err(|_| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: std::io::Error::other("stdout 捕获线程 panic"),
            })
        })?
        .map_err(|e| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: e,
            })
        })?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: std::io::Error::other("stderr 捕获线程 panic"),
            })
        })?
        .map_err(|e| {
            super::box_diagnostic(RunExecFailed {
                program: cmd.get_program().to_string_lossy().to_string(),
                fixture: rel_fixture.display().to_string(),
                source: e,
            })
        })?;

    if timed_out {
        return Err(super::box_diagnostic(RunExecTimeout {
            timeout_ms: exp.timeout_ms.unwrap_or(0),
            fixture: rel_fixture.display().to_string(),
        }));
    }

    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

#[cfg(unix)]
fn kill_child_process_tree(child: &mut std::process::Child) -> std::io::Result<()> {
    let pgid: c_int = child
        .id()
        .try_into()
        .map_err(|_| std::io::Error::other("子进程 pid 超出 Unix process-group 支持范围"))?;

    // SAFETY: timeout path 会把 child 放进自己的 process group；这里向 `-pgid`
    // 发送 `SIGKILL` 以确保 `scoop run` 及其后代一并退出，不再持有继承的 pipe。
    let kill_result = unsafe { libc_kill(-pgid, SIGKILL) };
    if kill_result == 0 {
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(ESRCH) {
        return Ok(());
    }

    let _ = child.kill();
    Err(err)
}

#[cfg(not(unix))]
fn kill_child_process_tree(child: &mut std::process::Child) -> std::io::Result<()> {
    child.kill()
}

/// 等待子进程结束；若提供 `timeout`，则在超过时限后 kill 整个子进程树并返回 `(status, true)`。
fn wait_child_with_optional_timeout(
    child: &mut std::process::Child,
    timeout: Option<Duration>,
) -> std::io::Result<(ExitStatus, bool)> {
    let Some(timeout) = timeout else {
        return child.wait().map(|status| (status, false));
    };

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }

        if start.elapsed() >= timeout {
            kill_child_process_tree(child)?;
            let status = child.wait()?;
            return Ok((status, true));
        }

        let elapsed = start.elapsed();
        let remaining = if elapsed >= timeout {
            Duration::from_millis(0)
        } else {
            timeout - elapsed
        };
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

/// 对 `ExitStatus` 做“期望退出码/信号终止/非零退出”三类稳定诊断归因。
///
/// 约定：
/// - 若 `EXPECT-EXIT` 存在，则优先按“退出码断言”处理（允许非零退出）。
/// - 若 `EXPECT-EXIT` 不存在，则非零退出视为“程序失败”（`run_exec_nonzero_exit`）。
fn assert_exit_status_matches(
    rel_fixture: &Path,
    exp: &FixtureExpectation<'_>,
    status: ExitStatus,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    if let Some(expected) = exp.expect_exit {
        let Some(actual) = status.code() else {
            return Err(super::box_diagnostic(RunExecSignaled {
                signal: exit_status_signal_string(status),
                fixture: rel_fixture.display().to_string(),
            }));
        };

        if actual != expected {
            return Err(super::box_diagnostic(RunExitCodeMismatch {
                expected,
                actual,
                fixture: rel_fixture.display().to_string(),
            }));
        }

        return Ok(());
    }

    if status.success() {
        return Ok(());
    }

    if status.code().is_none() {
        return Err(super::box_diagnostic(RunExecSignaled {
            signal: exit_status_signal_string(status),
            fixture: rel_fixture.display().to_string(),
        }));
    }

    Err(super::box_diagnostic(RunExecNonZeroExit {
        status: status.to_string(),
        fixture: rel_fixture.display().to_string(),
    }))
}

/// 将“信号终止”转换为稳定字符串（目前使用信号编号）。
fn exit_status_signal_string(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        if let Some(signal) = status.signal() {
            return signal.to_string();
        }
    }

    let _ = status;
    "unknown".to_string()
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
    let actual = super::normalize_newlines(actual_stdout);

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

        let expected = super::normalize_newlines(&expected);

        if expected != actual {
            return Err(super::box_diagnostic(RunStdoutMismatch {
                path: golden_path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                expected_preview: preview_text(&expected, 200),
                actual_preview: preview_text(&actual, 200),
            }));
        }
    }

    if let Some(needle) = exp.run_stdout_contains {
        let needle = super::normalize_newlines(needle);
        if !actual.contains(&needle) {
            return Err(super::box_diagnostic(RunStdoutMissingSubstring {
                substring: needle,
                fixture: fixture_path.display().to_string(),
            }));
        }
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
    let actual = super::normalize_newlines(actual_stderr);

    if let Some(golden_rel) = exp.run_stderr {
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

        if expected != actual {
            return Err(super::box_diagnostic(RunStderrMismatch {
                path: golden_path.display().to_string(),
                fixture: fixture_path.display().to_string(),
            }));
        }
    }

    if let Some(needle) = exp.run_stderr_contains {
        let needle = super::normalize_newlines(needle);
        if !actual.contains(&needle) {
            return Err(super::box_diagnostic(RunStderrMissingSubstring {
                substring: needle,
                fixture: fixture_path.display().to_string(),
            }));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("scoop_{prefix}_{}_{}", std::process::id(), nanos));
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
    fn stderr_contains_matches_ok() {
        let dir = make_temp_dir("stderr_contains_matches_ok");
        let fixture_path = dir.join("hello.scoop");

        std::fs::write(
            &fixture_path,
            "// RUN-STDERR-CONTAINS: boxed oversized variant\nfun main() {}\n",
        )
        .unwrap();

        let exp =
            FixtureExpectation::from_source("// RUN-STDERR-CONTAINS: boxed oversized variant\n");
        assert_stderr_matches(&fixture_path, &exp, "warn: boxed oversized variant\n").unwrap();

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
    fn run_fixture_command_writes_stdin_from_file() {
        let dir = make_temp_dir("run_fixture_command_writes_stdin_from_file");
        let fixture_path = dir.join("hello.scoop");
        let stdin_path = dir.join("in.txt");
        let golden_path = dir.join("out.txt");

        std::fs::write(
            &fixture_path,
            "// RUN-STDIN: in.txt\n// RUN-STDOUT: out.txt\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(&stdin_path, "hello\n").unwrap();
        std::fs::write(&golden_path, "hello\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDIN: in.txt\n// RUN-STDOUT: out.txt\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("cat");
            cmd
        };

        run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_sets_env_vars() {
        let dir = make_temp_dir("run_fixture_command_sets_env_vars");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("out.txt");

        std::fs::write(
            &fixture_path,
            "// RUN-STDOUT: out.txt\n// ENV: FOO=bar\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(&golden_path, "bar\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n// ENV: FOO=bar\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("printf '%s\\n' \"$FOO\"");
            cmd
        };

        run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_pass_env_overrides_are_applied_and_can_be_overridden_by_fixture_env() {
        let dir = make_temp_dir("run_pass_env_overrides_are_applied_and_can_be_overridden");
        let fixture_path = dir.join("hello.scoop");
        let golden_path = dir.join("out.txt");

        std::fs::write(
            &fixture_path,
            "// RUN-STDOUT: out.txt\n// ENV: FOO=fixture\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(&golden_path, "fixture\n").unwrap();

        let exp = FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n// ENV: FOO=fixture\n");
        let mut env = crate::fixtures::RunPassEnvOverrides::new();
        env.set("FOO", "global");

        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("printf '%s\\n' \"$FOO\"");
            cmd
        };
        let mut cmd = cmd;
        env.apply_to_command(&mut cmd);

        run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn run_pass_single_pipeline_omits_selector() {
        let exp = FixtureExpectation::from_source("// ARGS: --program-arg\n");
        let cmd = build_run_mode_command(
            PathBuf::from("scoop"),
            Path::new("tests/fixtures/run-pass/minimal_main.scoop"),
            None,
            scoopc::session::SessionOptions::new(),
            &exp,
            &crate::fixtures::RunPassEnvOverrides::new(),
        );

        let args = command_args(&cmd);
        assert_eq!(args.first().map(String::as_str), Some("run"));
        assert!(!args.iter().any(|arg| arg == "--effect-pipeline"));
        assert!(args.iter().any(|arg| arg == "--program-arg"));
    }

    #[test]
    fn run_pass_single_pipeline_propagates_sysroot_overlay_env() {
        let exp = FixtureExpectation::from_source("// ARGS: --program-arg\n");
        let overlay = PathBuf::from("/tmp/sysroot-overlay");
        let cmd = build_run_mode_command(
            PathBuf::from("scoop"),
            Path::new("tests/fixtures/run-pass/minimal_main.scoop"),
            None,
            scoopc::session::SessionOptions::new().with_sysroot_overlay(overlay.clone()),
            &exp,
            &crate::fixtures::RunPassEnvOverrides::new(),
        );

        let propagated = cmd
            .get_envs()
            .find_map(|(key, value)| {
                if key == scoopc::sysroot::SYSROOT_OVERLAY_ENV {
                    value.map(|value| value.to_os_string())
                } else {
                    None
                }
            })
            .unwrap();

        assert_eq!(propagated, overlay.into_os_string());
    }

    #[test]
    fn run_pass_single_pipeline_dump_stackmaps_command_omits_selector() {
        let cmd = build_dump_stackmaps_command(
            PathBuf::from("scoop"),
            Path::new("/tmp/a.out"),
            scoopc::session::SessionOptions::new(),
        );

        let args = command_args(&cmd);
        assert_eq!(args[0..2], ["dump-stackmaps", "--verify-roots"]);
        assert!(!args.iter().any(|arg| arg == "--effect-pipeline"));
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

        let exp =
            FixtureExpectation::from_source("// RUN-STDOUT: out.txt\n// RUN-STDERR: err.txt\n");
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

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_expect_exit_allows_nonzero_exit() {
        let dir = make_temp_dir("run_fixture_command_expect_exit_allows_nonzero_exit");
        let fixture_path = dir.join("hello.scoop");

        std::fs::write(&fixture_path, "// EXPECT-EXIT: 3\nfun main() {}\n").unwrap();

        let exp = FixtureExpectation::from_source("// EXPECT-EXIT: 3\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("exit 3");
            cmd
        };

        run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap();

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_exit_code_mismatch_has_stable_code() {
        let dir = make_temp_dir("run_fixture_command_exit_code_mismatch_has_stable_code");
        let fixture_path = dir.join("hello.scoop");

        std::fs::write(&fixture_path, "// EXPECT-EXIT: 0\nfun main() {}\n").unwrap();

        let exp = FixtureExpectation::from_source("// EXPECT-EXIT: 0\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("exit 3");
            cmd
        };

        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_exit_code_mismatch"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_timeout_has_stable_code() {
        let dir = make_temp_dir("run_fixture_command_timeout_has_stable_code");
        let fixture_path = dir.join("hello.scoop");

        std::fs::write(&fixture_path, "// TIMEOUT: 10\nfun main() {}\n").unwrap();

        let exp = FixtureExpectation::from_source("// TIMEOUT: 10\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("sleep 5");
            cmd
        };

        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_exec_timeout"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_timeout_removes_single_file_virtual_cone() {
        let dir = make_temp_dir("run_fixture_command_timeout_removes_single_file_virtual_cone");
        let fixture_path = dir.join("hello world!.scoop");
        let virtual_root = dir
            .join("build")
            .join("debug")
            .join("virtual")
            .join("hello_world_@0.0.0");

        std::fs::write(&fixture_path, "// TIMEOUT: 10\nfun main() {}\n").unwrap();
        std::fs::create_dir_all(virtual_root.join("src")).unwrap();
        std::fs::write(virtual_root.join("Cone.toml"), "[cone]\n").unwrap();

        let exp = FixtureExpectation::from_source("// TIMEOUT: 10\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("sleep 5");
            cmd
        };

        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_exec_timeout"
        );
        assert!(!virtual_root.exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_timeout_kills_descendants() {
        let dir = make_temp_dir("run_fixture_command_timeout_kills_descendants");
        let fixture_path = dir.join("hello.scoop");
        let pid_path = dir.join("descendant.pid");

        std::fs::write(&fixture_path, "// TIMEOUT: 200\nfun main() {}\n").unwrap();

        let exp = FixtureExpectation::from_source("// TIMEOUT: 200\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(format!(
                "sleep 2 & echo $! > \"{}\"; wait",
                pid_path.display()
            ));
            cmd
        };

        let start = Instant::now();
        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_exec_timeout"
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "timeout cleanup should not wait for descendant sleep: {:?}",
            start.elapsed()
        );

        let descendant_pid = wait_for_descendant_pid(&pid_path);
        wait_for_process_exit(descendant_pid);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn run_fixture_command_signaled_has_stable_code() {
        let dir = make_temp_dir("run_fixture_command_signaled_has_stable_code");
        let fixture_path = dir.join("hello.scoop");

        std::fs::write(&fixture_path, "fun main() {}\n").unwrap();

        let exp = FixtureExpectation::from_source("fun main() {}\n");
        let cmd = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("kill -9 $$");
            cmd
        };

        let err = run_fixture_command(&fixture_path, &fixture_path, &exp, cmd).unwrap_err();
        assert_eq!(
            err.code().unwrap().to_string(),
            "scoop::fixtures::run_exec_signaled"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    fn wait_for_descendant_pid(pid_path: &Path) -> c_int {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if let Ok(pid) = std::fs::read_to_string(pid_path)
                && let Ok(pid) = pid.trim().parse::<c_int>()
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "descendant pid file was not created in time"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn wait_for_process_exit(pid: c_int) {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if !process_exists(pid) {
                return;
            }
            if Instant::now() >= deadline {
                // SAFETY: best-effort cleanup for a descendant created by this test.
                let _ = unsafe { libc_kill(pid, SIGKILL) };
                panic!("descendant process {pid} was not terminated by timeout cleanup");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn process_exists(pid: c_int) -> bool {
        // SAFETY: `kill(pid, 0)` is the standard POSIX liveness probe and does not deliver a signal.
        let probe = unsafe { libc_kill(pid, 0) };
        if probe == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(ESRCH)
    }
}
