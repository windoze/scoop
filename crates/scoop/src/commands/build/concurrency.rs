//! Per-cone 多进程并发编译的 CLI 参数、抽象边界与本地默认实现（P10-T05/P10-T06）。
//!
//! - P10-T05 发布的 trait surface：`ConcurrencyStrategy` / `SubprocessConeCompiler`。
//! - P10-T06 在 [`LocalProcessConeCompiler`] 上落地真实子进程派发：通过
//!   `scoopc build-single-cone` 子命令 fork+exec，把每个 cone 的 frontend artifact
//!   产出 + cache-hit 短路从 driver 调度器侧解耦出来。
//!
//! 设计基线：
//! - `ConcurrencyStrategy`：把"如何决定并发数"与"如何调度"解耦。当前只实现
//!   `FixedJobsStrategy`，未来可以挂接按物理 CPU 数 / 内存 / 远端 worker 池等
//!   策略。
//! - `SubprocessConeCompiler`：把"实际跑一个 cone 的子进程"与"调度 driver"解耦，
//!   方便 driver 单元测试用 fake 实现替换 fork+exec。

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::Command;

use thiserror::Error;

/// per-cone 多进程并发编译的默认子进程数。
///
/// 这是 CLI `--jobs N` / 环境变量 `SCOOP_BUILD_JOBS` 都未提供时的回退值。
/// 选 4 是出于"在主流开发机上不至于占满资源、又能开始铺并发执行体"的折中；
/// 后续策略（按物理 CPU 数 / 远端资源池）应通过 [`ConcurrencyStrategy`]
/// 的新实现挂接，而不是改动该常量。
pub const DEFAULT_BUILD_JOBS: usize = 4;

/// per-cone 多进程并发编译的环境变量名。
///
/// CLI `--jobs N` 优先级高于该环境变量；环境变量优先级高于 [`DEFAULT_BUILD_JOBS`]。
/// 解析失败（非数字 / 0 / 负值）时由 [`resolve_build_jobs`] 返回结构化错误。
pub const BUILD_JOBS_ENV_VAR: &str = "SCOOP_BUILD_JOBS";

/// 解析 `--jobs N` / `SCOOP_BUILD_JOBS` / 默认值 三者之一作为最终并发数。
///
/// 优先级：CLI > env > [`DEFAULT_BUILD_JOBS`]。
/// 0 / 负值 / 非数字会返回 [`BuildJobsError`]。
pub fn resolve_build_jobs(cli_jobs: Option<NonZeroUsize>) -> Result<NonZeroUsize, BuildJobsError> {
    if let Some(jobs) = cli_jobs {
        return Ok(jobs);
    }

    match std::env::var(BUILD_JOBS_ENV_VAR) {
        Ok(raw) => parse_jobs_env_value(&raw),
        Err(std::env::VarError::NotPresent) => Ok(default_build_jobs()),
        Err(std::env::VarError::NotUnicode(_)) => Err(BuildJobsError::EnvNotUnicode),
    }
}

/// `DEFAULT_BUILD_JOBS` 对应的 [`NonZeroUsize`]（编译期保证非零）。
pub fn default_build_jobs() -> NonZeroUsize {
    NonZeroUsize::new(DEFAULT_BUILD_JOBS).expect("DEFAULT_BUILD_JOBS must be non-zero")
}

fn parse_jobs_env_value(raw: &str) -> Result<NonZeroUsize, BuildJobsError> {
    let trimmed = raw.trim();
    let parsed: usize = trimmed
        .parse()
        .map_err(|_| BuildJobsError::InvalidEnvValue {
            value: raw.to_string(),
        })?;
    NonZeroUsize::new(parsed).ok_or(BuildJobsError::InvalidEnvValue {
        value: raw.to_string(),
    })
}

/// 解析 `--jobs N` / `SCOOP_BUILD_JOBS` 时的错误。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BuildJobsError {
    #[error(
        "环境变量 `{}` 的值 `{value}` 无效：必须是正整数（>=1）",
        BUILD_JOBS_ENV_VAR
    )]
    InvalidEnvValue { value: String },
    #[error("环境变量 `{}` 不是有效的 UTF-8 字符串", BUILD_JOBS_ENV_VAR)]
    EnvNotUnicode,
}

/// "如何决定并发编译子进程数" trait。
///
/// 本任务（P10-T05）只提供 [`FixedJobsStrategy`] 一个实现。trait 形态保留是为了
/// 后续按物理 CPU 数 / 内存 / 远端 worker 池等策略挂接，而不必改 driver 调用点。
///
/// driver 不应假设并发数为常量——同一个 build 内允许策略动态调整，但本任务的
/// 默认实现是 fixed value。
pub trait ConcurrencyStrategy: std::fmt::Debug + Send + Sync {
    /// 调度 driver 在本次 build 中允许同时跑的子进程编译任务数。
    ///
    /// driver 必须把返回值视为"硬上限"：实际并发数可以小于此值，但不允许大于。
    fn max_concurrent_jobs(&self) -> NonZeroUsize;
}

/// 固定并发数策略：所有 build 步骤都使用同一个 `jobs` 上限。
#[derive(Debug, Clone, Copy)]
pub struct FixedJobsStrategy {
    jobs: NonZeroUsize,
}

impl FixedJobsStrategy {
    pub fn new(jobs: NonZeroUsize) -> Self {
        Self { jobs }
    }
}

impl ConcurrencyStrategy for FixedJobsStrategy {
    fn max_concurrent_jobs(&self) -> NonZeroUsize {
        self.jobs
    }
}

/// 跑单个 cone 的子进程编译执行体抽象。
///
/// driver 把 cone DAG 的拓扑遍历和并发调度自己处理；每一步要把"实际跑 cone"
/// 的过程交给本 trait 的实现，由后者负责 fork+exec / RPC / 任意分布式编译路径。
///
/// 输入：
/// - `cone_id`：本次要编译的 cone 在 driver 视角的稳定标识（StableConeKey 字符串化形式）；
/// - `cone_root`：本 cone 的源根目录，subprocess 会用它加载 `Cone.toml` + sources；
/// - `upstream_artifact_dirs`：本 cone 可读取的所有上游 cone artifact 目录；
/// - `inputs_fingerprint`：本 cone 的输入 fingerprint（与 P10-T04 fingerprint chain 一致）；
/// - `output_artifact_dir`：本 cone artifact 输出目录。
///
/// 输出：
/// - `output_artifact_dir` 中应包含完整的 [`crate::commands::build::layout`] 兼容布局；
/// - 同时要返回 `outputs.fingerprint` 让 driver 把它写到 build.json，供下游 cone
///   作为 inputs 之一。
pub trait SubprocessConeCompiler: std::fmt::Debug + Send + Sync {
    /// 编译单个 cone 并把 artifact 写到 `request.output_artifact_dir`。
    fn compile_cone(
        &self,
        request: ConeCompileRequest,
    ) -> Result<ConeCompileResponse, SubprocessConeCompileError>;
}

/// 调度 driver 派发给 [`SubprocessConeCompiler::compile_cone`] 的请求载荷。
#[derive(Debug, Clone)]
pub struct ConeCompileRequest {
    /// 待编译 cone 的稳定标识（StableConeKey 字符串化形式）。
    pub cone_id: String,
    /// cone 的源根目录（包含 `Cone.toml`）。
    pub cone_root: PathBuf,
    /// 本 cone 可读取的所有上游 cone artifact 目录。
    pub upstream_artifact_dirs: Vec<PathBuf>,
    /// 本 cone 输入 fingerprint（与 P10-T04 fingerprint chain 一致）。
    pub inputs_fingerprint: Vec<u8>,
    /// 本 cone 的 artifact 输出目录。driver 应在此目录下找到 manifest / facts / objs。
    pub output_artifact_dir: PathBuf,
}

/// [`SubprocessConeCompiler::compile_cone`] 完成后返回给调度 driver 的应答。
#[derive(Debug, Clone)]
pub struct ConeCompileResponse {
    /// 实际写出的 cone artifact 目录（应等于 [`ConeCompileRequest::output_artifact_dir`]）。
    pub output_artifact_dir: PathBuf,
    /// 本 cone 的 outputs fingerprint（写入 build.json 前的最终值）。
    pub outputs_fingerprint: Vec<u8>,
}

/// 决定 [`LocalProcessConeCompiler`] 在搜索 scoopc 二进制时优先级最高的环境变量。
///
/// 设置该变量可显式锁定要派发的 scoopc 路径（通常用于测试或自定义安装位置）；
/// 未设置时回退到 `current_exe()` 旁的 `scoopc` / `scoopc.exe`。
pub const SCOOPC_BIN_ENV_VAR: &str = "SCOOP_SCOOPC_BIN";

/// [`SubprocessConeCompiler::compile_cone`] 可能返回的稳定错误形态。
#[derive(Debug, Error)]
pub enum SubprocessConeCompileError {
    #[error(
        "无法定位 scoopc 可执行文件（cone_id={cone_id}）：环境变量 `{env_var}` 与 `{tried:?}` 都不可用"
    )]
    BinaryNotFound {
        cone_id: String,
        env_var: String,
        tried: Vec<PathBuf>,
    },
    #[error("无法启动 scoopc 子进程：cone_id={cone_id}, scoopc={}", scoopc.display())]
    SpawnFailure {
        cone_id: String,
        scoopc: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("scoopc 子进程异常退出：cone_id={cone_id}, status={status}\n--- stderr ---\n{stderr}")]
    ExitNonZero {
        cone_id: String,
        status: std::process::ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error(
        "scoopc 子进程退出 0 但未写入 outputs.fingerprint：cone_id={cone_id}, dir={}",
        dir.display()
    )]
    ArtifactMissing {
        cone_id: String,
        dir: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// 本机进程内的 [`SubprocessConeCompiler`] 实现：通过 `scoopc build-single-cone`
/// 把每个 cone 派发到独立子进程。
///
/// scoopc 二进制定位顺序：
/// 1. 环境变量 [`SCOOPC_BIN_ENV_VAR`] 指定的路径；
/// 2. 当前 `scoop` 可执行文件所在目录的 `scoopc` / `scoopc.exe`；
/// 3. 若当前可执行文件位于 `target/<profile>/deps/`（例如 `cargo test` 跑出来的
///    test binary），再向上一层尝试 `target/<profile>/scoopc`。
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalProcessConeCompiler;

impl LocalProcessConeCompiler {
    pub fn new() -> Self {
        Self
    }
}

impl SubprocessConeCompiler for LocalProcessConeCompiler {
    fn compile_cone(
        &self,
        request: ConeCompileRequest,
    ) -> Result<ConeCompileResponse, SubprocessConeCompileError> {
        let scoopc = locate_scoopc_bin(&request.cone_id)?;

        let mut cmd = Command::new(&scoopc);
        cmd.arg("build-single-cone");
        cmd.arg("--cone-root").arg(&request.cone_root);
        cmd.arg("--out").arg(&request.output_artifact_dir);
        cmd.arg("--inputs-fingerprint")
            .arg(hex_lower(&request.inputs_fingerprint));
        cmd.arg("--cone-id").arg(&request.cone_id);
        for upstream in &request.upstream_artifact_dirs {
            cmd.arg("--upstream-artifact").arg(upstream);
        }

        let output = cmd
            .output()
            .map_err(|err| SubprocessConeCompileError::SpawnFailure {
                cone_id: request.cone_id.clone(),
                scoopc: scoopc.clone(),
                source: err,
            })?;

        forward_subprocess_stderr_with_prefix(&request.cone_id, &output.stderr);

        if !output.status.success() {
            return Err(SubprocessConeCompileError::ExitNonZero {
                cone_id: request.cone_id,
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let outputs_fp_path = request
            .output_artifact_dir
            .join(scoopc::cone::CONE_ARTIFACT_OUTPUTS_FINGERPRINT_FILE_NAME);
        let outputs_fingerprint = std::fs::read(&outputs_fp_path).map_err(|err| {
            SubprocessConeCompileError::ArtifactMissing {
                cone_id: request.cone_id.clone(),
                dir: request.output_artifact_dir.clone(),
                source: err,
            }
        })?;

        Ok(ConeCompileResponse {
            output_artifact_dir: request.output_artifact_dir,
            outputs_fingerprint,
        })
    }
}

pub(crate) fn locate_scoopc_bin(cone_id: &str) -> Result<PathBuf, SubprocessConeCompileError> {
    let mut tried: Vec<PathBuf> = Vec::new();

    if let Ok(raw) = std::env::var(SCOOPC_BIN_ENV_VAR) {
        let path = PathBuf::from(&raw);
        tried.push(path.clone());
        if path.is_file() {
            return Ok(path);
        }
    }

    let exe_name = if cfg!(windows) {
        "scoopc.exe"
    } else {
        "scoopc"
    };

    if let Ok(current) = std::env::current_exe()
        && let Some(parent) = current.parent()
    {
        let sibling = parent.join(exe_name);
        tried.push(sibling.clone());
        if sibling.is_file() {
            return Ok(sibling);
        }
        // `cargo test` 把 test binary 丢到 `target/<profile>/deps/`；scoopc 在上一层。
        if parent.file_name().and_then(|s| s.to_str()) == Some("deps")
            && let Some(grandparent) = parent.parent()
        {
            let candidate = grandparent.join(exe_name);
            tried.push(candidate.clone());
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(SubprocessConeCompileError::BinaryNotFound {
        cone_id: cone_id.to_string(),
        env_var: SCOOPC_BIN_ENV_VAR.to_string(),
        tried,
    })
}

fn forward_subprocess_stderr_with_prefix(cone_id: &str, stderr: &[u8]) {
    if stderr.is_empty() {
        return;
    }
    let text = String::from_utf8_lossy(stderr);
    let prefix = format!("[{cone_id}] ");
    for line in text.lines() {
        eprintln!("{prefix}{line}");
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nz(n: usize) -> NonZeroUsize {
        NonZeroUsize::new(n).expect("test value must be non-zero")
    }

    fn with_env_var<F: FnOnce() -> R, R>(key: &str, value: Option<&str>, f: F) -> R {
        // 注意：单元测试可能并发跑；这里用一个 Mutex 串行 BUILD_JOBS_ENV_VAR 改动。
        use std::sync::Mutex;
        static GUARD: Mutex<()> = Mutex::new(());
        let _lock = GUARD.lock().unwrap_or_else(|e| e.into_inner());

        let prev = std::env::var(key).ok();
        // SAFETY: env mutation is serialized by the GUARD mutex above.
        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        let result = f();
        match prev {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        result
    }

    #[test]
    fn fixed_jobs_strategy_reports_constructor_value() {
        let strat = FixedJobsStrategy::new(nz(7));
        assert_eq!(strat.max_concurrent_jobs().get(), 7);
    }

    #[test]
    fn default_build_jobs_matches_constant() {
        assert_eq!(default_build_jobs().get(), DEFAULT_BUILD_JOBS);
    }

    #[test]
    fn resolve_build_jobs_uses_cli_when_provided() {
        with_env_var(BUILD_JOBS_ENV_VAR, Some("11"), || {
            let jobs = resolve_build_jobs(Some(nz(3))).unwrap();
            assert_eq!(jobs.get(), 3, "CLI 应当覆盖 env");
        });
    }

    #[test]
    fn resolve_build_jobs_falls_back_to_env_when_cli_absent() {
        with_env_var(BUILD_JOBS_ENV_VAR, Some("11"), || {
            let jobs = resolve_build_jobs(None).unwrap();
            assert_eq!(jobs.get(), 11);
        });
    }

    #[test]
    fn resolve_build_jobs_falls_back_to_default_when_neither_present() {
        with_env_var(BUILD_JOBS_ENV_VAR, None, || {
            let jobs = resolve_build_jobs(None).unwrap();
            assert_eq!(jobs.get(), DEFAULT_BUILD_JOBS);
        });
    }

    #[test]
    fn resolve_build_jobs_rejects_zero_env() {
        with_env_var(BUILD_JOBS_ENV_VAR, Some("0"), || {
            let err = resolve_build_jobs(None).unwrap_err();
            assert_eq!(
                err,
                BuildJobsError::InvalidEnvValue {
                    value: "0".to_string()
                }
            );
        });
    }

    #[test]
    fn resolve_build_jobs_rejects_non_numeric_env() {
        with_env_var(BUILD_JOBS_ENV_VAR, Some("abc"), || {
            let err = resolve_build_jobs(None).unwrap_err();
            assert_eq!(
                err,
                BuildJobsError::InvalidEnvValue {
                    value: "abc".to_string()
                }
            );
        });
    }

    #[test]
    fn resolve_build_jobs_rejects_negative_env() {
        with_env_var(BUILD_JOBS_ENV_VAR, Some("-2"), || {
            let err = resolve_build_jobs(None).unwrap_err();
            assert_eq!(
                err,
                BuildJobsError::InvalidEnvValue {
                    value: "-2".to_string()
                }
            );
        });
    }

    #[test]
    fn local_process_cone_compiler_failure_carries_cone_id_through_subprocess_error() {
        // 不论本地 toolchain 的 `target/<profile>/scoopc` 是否已经构建出来，给
        // `LocalProcessConeCompiler` 喂一个不可能成功跑通的 cone（不存在的 cone-root +
        // 不存在的输出目录），都应该返回一个携带 `cone_id="demo@0.0.0"` 的结构化错误，
        // 而不是 panic 或丢失 cone 标识——driver 调度器的失败诊断依赖这个字段。
        with_env_var(
            SCOOPC_BIN_ENV_VAR,
            Some("/definitely/not/a/real/scoopc"),
            || {
                let compiler = LocalProcessConeCompiler::new();
                let request = ConeCompileRequest {
                    cone_id: "demo@0.0.0".to_string(),
                    cone_root: PathBuf::from("/nonexistent/cone"),
                    upstream_artifact_dirs: Vec::new(),
                    inputs_fingerprint: Vec::new(),
                    output_artifact_dir: PathBuf::from("/nonexistent/out"),
                };
                let err = compiler.compile_cone(request).unwrap_err();
                let cone_id = match &err {
                    SubprocessConeCompileError::BinaryNotFound { cone_id, .. }
                    | SubprocessConeCompileError::SpawnFailure { cone_id, .. }
                    | SubprocessConeCompileError::ExitNonZero { cone_id, .. }
                    | SubprocessConeCompileError::ArtifactMissing { cone_id, .. } => cone_id,
                };
                assert_eq!(
                    cone_id, "demo@0.0.0",
                    "driver scheduler 依赖 cone_id 在所有失败分支保持稳定：{err:?}"
                );
            },
        );
    }

    #[test]
    fn hex_lower_encodes_known_bytes() {
        assert_eq!(super::hex_lower(&[]), "");
        assert_eq!(super::hex_lower(&[0xab, 0xcd, 0xef]), "abcdef");
        assert_eq!(super::hex_lower(&[0x00, 0x10]), "0010");
    }

    #[test]
    fn concurrency_strategy_is_object_safe() {
        // 编译期断言：trait 必须是 object-safe 的，driver 才能用 `Box<dyn ConcurrencyStrategy>`
        // 注入策略。
        fn _assert_object_safe(_: &dyn ConcurrencyStrategy) {}
        let strat = FixedJobsStrategy::new(nz(2));
        _assert_object_safe(&strat);
    }

    #[test]
    fn subprocess_cone_compiler_is_object_safe() {
        fn _assert_object_safe(_: &dyn SubprocessConeCompiler) {}
        let compiler = LocalProcessConeCompiler::new();
        _assert_object_safe(&compiler);
    }
}
