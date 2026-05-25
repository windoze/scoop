//! Per-cone 多进程并发编译的 CLI 参数与抽象边界（P10-T05）。
//!
//! 该模块只负责发布"如何选择并发数"和"如何在子进程里跑一个 cone"这两条 trait，
//! 以及它们的本地默认实现的占位。本任务**不**引入任何并发执行行为：driver 仍然
//! 在主进程内顺序遍历 cone DAG，CLI 选项与 trait 仅作为下游 P10-T06 子进程并发
//! driver 的接入点。
//!
//! 设计基线：
//! - `ConcurrencyStrategy`：把"如何决定并发数"与"如何调度"解耦。本任务只实现
//!   `FixedJobsStrategy`，未来可以挂接按物理 CPU 数 / 内存 / 远端 worker 池等
//!   策略。
//! - `SubprocessConeCompiler`：把"实际跑一个 cone 的子进程"与"调度 driver"解耦。
//!   本任务只提供 `LocalProcessConeCompiler` 占位实现，`compile_cone` 返回稳定
//!   错误（实际 fork+exec 留给 P10-T06）。

use std::num::NonZeroUsize;
use std::path::PathBuf;

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

/// 跑单个 cone 的子进程编译执行体抽象（P10-T05；占位）。
///
/// driver 把 cone DAG 的拓扑遍历和并发调度自己处理；每一步要把"实际跑 cone"
/// 的过程交给本 trait 的实现，由后者负责 fork+exec / RPC / 任意分布式编译路径。
///
/// 输入：
/// - `cone_id`：本次要编译的 cone 在 driver 视角的稳定标识（StableConeKey 字符串化形式）；
/// - `upstream_artifacts`：本 cone 可读取的所有上游 cone artifact 目录；
/// - `inputs_fingerprint`：本 cone 的输入 fingerprint（与 P10-T04 fingerprint chain 一致）；
/// - `output_artifact_dir`：本 cone artifact 输出目录。
///
/// 输出：
/// - `output_artifact_dir` 中应包含完整的 [`crate::commands::build::layout`] 兼容布局；
/// - 同时要返回 `outputs.fingerprint` 让 driver 把它写到 build.json，供下游 cone
///   作为 inputs 之一。
pub trait SubprocessConeCompiler: std::fmt::Debug + Send + Sync {
    /// 在子进程中编译单个 cone。占位实现可返回
    /// [`SubprocessConeCompileError::NotYetImplemented`]；P10-T06 落地真实子进程派发时
    /// 才会被 driver 实际调用。
    #[allow(dead_code)]
    fn compile_cone(
        &self,
        request: ConeCompileRequest,
    ) -> Result<ConeCompileResponse, SubprocessConeCompileError>;
}

/// 调度 driver 派发给 [`SubprocessConeCompiler::compile_cone`] 的请求载荷。
///
/// 字段在 P10-T05 阶段尚未被实际读取（占位实现 `LocalProcessConeCompiler::compile_cone`
/// 直接返回 `NotYetImplemented`）。P10-T06 落地真实子进程派发时会消费这些字段。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConeCompileRequest {
    /// 待编译 cone 的稳定标识（StableConeKey 字符串化形式）。
    pub cone_id: String,
    /// 本 cone 可读取的所有上游 cone artifact 目录。
    pub upstream_artifact_dirs: Vec<PathBuf>,
    /// 本 cone 输入 fingerprint（与 P10-T04 fingerprint chain 一致）。
    pub inputs_fingerprint: Vec<u8>,
    /// 本 cone 的 artifact 输出目录。driver 应在此目录下找到 manifest / facts / objs。
    pub output_artifact_dir: PathBuf,
}

/// [`SubprocessConeCompiler::compile_cone`] 完成后返回给调度 driver 的应答。
///
/// 字段在 P10-T05 阶段尚未被实际读取；P10-T06 子进程 driver 会消费这些字段。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConeCompileResponse {
    /// 实际写出的 cone artifact 目录（应等于 [`ConeCompileRequest::output_artifact_dir`]）。
    pub output_artifact_dir: PathBuf,
    /// 本 cone 的 outputs fingerprint（写入 build.json 前的最终值）。
    pub outputs_fingerprint: Vec<u8>,
}

/// [`SubprocessConeCompiler::compile_cone`] 可能返回的稳定错误形态。
///
/// 本任务（P10-T05）只引入 `NotYetImplemented` 与 `Io`；P10-T06 落地真实子进程
/// 调用时再引入 `SpawnFailure` / `ExitNonZero` / `ArtifactMissing` 等更细分类。
/// `Io` 在 P10-T05 暂未由占位实现构造，但保留 surface 给 P10-T06。
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SubprocessConeCompileError {
    #[error("per-cone 子进程编译尚未实现（P10-T05 占位；将在 P10-T06 落地）：cone_id={cone_id}")]
    NotYetImplemented { cone_id: String },
    #[error("per-cone 子进程编译 IO 失败：{path}")]
    #[allow(dead_code)]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// 本机进程内的占位 [`SubprocessConeCompiler`] 实现（P10-T05 不引入并发执行行为）。
///
/// `compile_cone` 总是返回 [`SubprocessConeCompileError::NotYetImplemented`]。实际 fork+exec
/// 流程留给 P10-T06；driver 在 P10-T05 阶段不应调用本实现，应继续走 in-process 顺序路径。
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
        Err(SubprocessConeCompileError::NotYetImplemented {
            cone_id: request.cone_id,
        })
    }
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
    fn local_process_cone_compiler_returns_not_yet_implemented() {
        let compiler = LocalProcessConeCompiler::new();
        let request = ConeCompileRequest {
            cone_id: "demo@0.0.0".to_string(),
            upstream_artifact_dirs: Vec::new(),
            inputs_fingerprint: Vec::new(),
            output_artifact_dir: PathBuf::from("/nonexistent"),
        };
        let err = compiler.compile_cone(request).unwrap_err();
        match err {
            SubprocessConeCompileError::NotYetImplemented { cone_id } => {
                assert_eq!(cone_id, "demo@0.0.0");
            }
            other => panic!("unexpected error: {other:?}"),
        }
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
