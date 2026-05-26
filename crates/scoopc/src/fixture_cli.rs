//! Fixture runner CLI used by the `scoop` facade.

use std::collections::VecDeque;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use miette::{Context as _, IntoDiagnostic as _, Result, miette};

use crate::session::SessionOptions;

const FIXTURE_WORKER_ENV: &str = "SCOOP_FIXTURE_WORKER";
const FIXTURE_WORKER_OK_PREFIX: &str = "SCOOP_FIXTURE_OK=";
const DEFAULT_FIXTURE_PROCESSES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureCliOptions {
    pub fixtures: Option<PathBuf>,
    pub opt_level: Option<crate::opt::OptLevel>,
    pub gc_stress: bool,
    pub gc_move: bool,
    pub threads: Option<NonZeroU32>,
    pub exit_on_failure: bool,
    pub processes: NonZeroUsize,
    pub session_options: SessionOptions,
}

impl Default for FixtureCliOptions {
    fn default() -> Self {
        Self {
            fixtures: None,
            opt_level: None,
            gc_stress: false,
            gc_move: false,
            threads: None,
            exit_on_failure: false,
            processes: NonZeroUsize::new(DEFAULT_FIXTURE_PROCESSES).unwrap(),
            session_options: SessionOptions::new(),
        }
    }
}

#[derive(Debug)]
struct WorkerOutput {
    checks: usize,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct WorkerFailure {
    message: String,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct WorkerResult {
    target: crate::fixtures::PlannedFixtureTarget,
    result: std::result::Result<WorkerOutput, WorkerFailure>,
}

pub fn run(options: FixtureCliOptions) -> Result<()> {
    let root = options
        .fixtures
        .clone()
        .unwrap_or_else(|| PathBuf::from("tests/fixtures"));
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

    if is_worker_process() {
        let ok = crate::fixtures::run_all(
            &root,
            options.opt_level,
            options.session_options,
            &run_pass_env,
        )?;
        println!("{FIXTURE_WORKER_OK_PREFIX}{ok}");
        return Ok(());
    }

    let targets = crate::fixtures::plan_targets(&root)?;
    let total_targets = targets.len();
    let compiler_exe = crate::fixtures::current_scoopc_exe_path()
        .into_diagnostic()
        .wrap_err("无法定位当前 scoopc 可执行文件")?;
    let max_workers = options.processes.get().min(total_targets);
    let mut pending: VecDeque<_> = targets.into();
    let (tx, rx) = mpsc::channel();
    let mut active = 0usize;
    let mut completed = 0usize;
    let mut passed_targets = 0usize;
    let mut failed_targets = 0usize;
    let mut passed_checks = 0usize;
    let mut stopped_early = false;

    while active > 0 || !pending.is_empty() {
        while active < max_workers
            && !pending.is_empty()
            && !(options.exit_on_failure && failed_targets > 0)
        {
            let target = pending.pop_front().expect("pending should not be empty");
            if let Err(failure) = spawn_worker(&tx, &compiler_exe, target, options.clone()) {
                completed += 1;
                failed_targets += 1;
                print_worker_failure(completed, total_targets, &failure.target, &failure.failure);
                if options.exit_on_failure {
                    pending.clear();
                    stopped_early = true;
                }
                continue;
            }
            active += 1;
        }

        if active == 0 {
            break;
        }

        let worker = rx
            .recv()
            .into_diagnostic()
            .wrap_err("fixture worker 通道提前关闭")?;
        active -= 1;
        completed += 1;

        match worker.result {
            Ok(output) => {
                emit_stdout(&output.stdout);
                emit_stderr(&output.stderr);
                passed_targets += 1;
                passed_checks += output.checks;
                println!(
                    "[{completed}/{total_targets}] PASS {} ({})",
                    worker.target.display.display(),
                    format_check_count(output.checks),
                );
            }
            Err(failure) => {
                failed_targets += 1;
                print_worker_failure(completed, total_targets, &worker.target, &failure);
                if options.exit_on_failure {
                    pending.clear();
                    stopped_early = true;
                }
            }
        }
    }

    if failed_targets == 0 {
        println!("fixtures: ok ({passed_checks})");
        return Ok(());
    }

    let mut summary = format!(
        "fixtures: failed ({failed_targets}/{total_targets} targets failed, {passed_targets} targets passed, {} passed)",
        format_check_count(passed_checks),
    );
    if stopped_early {
        summary.push_str(", stopped scheduling new targets after the first failure");
    }
    println!("{summary}");
    Err(miette!(
        "fixtures 失败：{failed_targets} 个 target 执行失败"
    ))
}

fn is_worker_process() -> bool {
    std::env::var_os(FIXTURE_WORKER_ENV).is_some()
}

fn spawn_worker(
    tx: &mpsc::Sender<WorkerResult>,
    compiler_exe: &Path,
    target: crate::fixtures::PlannedFixtureTarget,
    options: FixtureCliOptions,
) -> std::result::Result<(), SpawnWorkerFailure> {
    let child = match build_worker_command(compiler_exe, &target.path, options).spawn() {
        Ok(child) => child,
        Err(source) => {
            return Err(SpawnWorkerFailure {
                target,
                failure: WorkerFailure {
                    message: format!("无法启动 fixture worker：{source}"),
                    stdout: String::new(),
                    stderr: String::new(),
                },
            });
        }
    };

    let tx = tx.clone();
    thread::spawn(move || {
        let result = match child.wait_with_output() {
            Ok(output) => interpret_worker_output(target, output),
            Err(source) => WorkerResult {
                target,
                result: Err(WorkerFailure {
                    message: format!("等待 fixture worker 结束失败：{source}"),
                    stdout: String::new(),
                    stderr: String::new(),
                }),
            },
        };
        let _ = tx.send(result);
    });

    Ok(())
}

fn build_worker_command(
    compiler_exe: &Path,
    target_path: &Path,
    options: FixtureCliOptions,
) -> Command {
    let mut cmd = Command::new(compiler_exe);
    cmd.arg("test-fixtures")
        .arg("--fixtures")
        .arg(target_path)
        .arg("--processes")
        .arg("1")
        .env(FIXTURE_WORKER_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::fixtures::apply_session_options_to_command(&options.session_options, &mut cmd);
    if let Some(scoop_bin) = std::env::var_os(crate::fixtures::FIXTURE_SCOOP_BIN_ENV) {
        cmd.env(crate::fixtures::FIXTURE_SCOOP_BIN_ENV, scoop_bin);
    }
    if let Some(level) = options.opt_level {
        cmd.arg("--opt-level").arg(level.as_str());
    }
    if options.gc_stress {
        cmd.arg("--gc-stress");
    }
    if options.gc_move {
        cmd.arg("--gc-move");
    }
    if let Some(threads) = options.threads {
        cmd.arg("--threads").arg(threads.get().to_string());
    }
    cmd
}

fn interpret_worker_output(
    target: crate::fixtures::PlannedFixtureTarget,
    output: std::process::Output,
) -> WorkerResult {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return WorkerResult {
            target,
            result: Err(WorkerFailure {
                message: format!("fixture worker 退出状态非 0：{}", output.status),
                stdout,
                stderr,
            }),
        };
    }
    let Some((checks, passthrough_stdout)) = parse_worker_success_count(&stdout) else {
        return WorkerResult {
            target,
            result: Err(WorkerFailure {
                message: "fixture worker 未返回成功计数".to_string(),
                stdout,
                stderr,
            }),
        };
    };
    WorkerResult {
        target,
        result: Ok(WorkerOutput {
            checks,
            stdout: passthrough_stdout,
            stderr,
        }),
    }
}

fn parse_worker_success_count(stdout: &str) -> Option<(usize, String)> {
    let mut checks = None;
    let mut passthrough = Vec::new();
    for line in stdout.lines() {
        if checks.is_none()
            && let Some(rest) = line.strip_prefix(FIXTURE_WORKER_OK_PREFIX)
            && let Ok(parsed) = rest.trim().parse::<usize>()
        {
            checks = Some(parsed);
            continue;
        }
        passthrough.push(line);
    }
    let checks = checks?;
    let mut passthrough_stdout = passthrough.join("\n");
    if !passthrough_stdout.is_empty() && stdout.ends_with('\n') {
        passthrough_stdout.push('\n');
    }
    Some((checks, passthrough_stdout))
}

fn emit_stdout(stdout: &str) {
    if !stdout.is_empty() {
        print!("{stdout}");
        if !stdout.ends_with('\n') {
            println!();
        }
    }
}

fn emit_stderr(stderr: &str) {
    if !stderr.is_empty() {
        eprint!("{stderr}");
        if !stderr.ends_with('\n') {
            eprintln!();
        }
    }
}

fn print_worker_failure(
    completed: usize,
    total_targets: usize,
    target: &crate::fixtures::PlannedFixtureTarget,
    failure: &WorkerFailure,
) {
    println!(
        "[{completed}/{total_targets}] FAIL {} ({})",
        target.display.display(),
        failure.message,
    );
    emit_stdout(&failure.stdout);
    emit_stderr(&failure.stderr);
}

fn format_check_count(checks: usize) -> String {
    if checks == 1 {
        "1 check".to_string()
    } else {
        format!("{checks} checks")
    }
}

#[derive(Debug)]
struct SpawnWorkerFailure {
    target: crate::fixtures::PlannedFixtureTarget,
    failure: WorkerFailure,
}

#[cfg(test)]
mod tests {
    use super::parse_worker_success_count;

    #[test]
    fn parse_worker_success_count_extracts_count_and_passthrough_stdout() {
        let parsed = parse_worker_success_count("note\nSCOOP_FIXTURE_OK=3\nextra\n").unwrap();
        assert_eq!(parsed.0, 3);
        assert_eq!(parsed.1, "note\nextra\n");
    }
}
