//! Fixtures 运行器（`scoop test`）。
//!
//! 设计目标：
//! - fixtures 是 Scoop 实现正确性的“底座”，必须可长期维护
//! - 每个 `.scoop` 文件可通过注释声明期望（pass/fail、错误包含等）
//!
//! 当前阶段支持：
//! - parse fixtures（经 AST stage dispatcher 调用 parser）
//! - resolve fixtures（最小名字绑定：import + TypeRef 解析）
//! - typecheck fixtures（T0403：TypeRef lowering + 泛型 arity 检查）
//! - infer fixtures（T05：类型推断阶段；当前先复用 typecheck pipeline，逐步打开更多推断能力）
//! - comptime fixtures：执行 `const val` 的常量折叠（`const fun` 解释器 v0，T1202c）
//! - build fixtures：调用 `scoop build`（产出 `.ll`/`.o`/`.s` 等单文件，用于排查）
//! - run-pass fixtures：通过 `scoop run` 真正执行，并做 stdout/stderr golden 比对（需要启用 `scoop` 的 `llvm` feature）
//!
//! 目录路由（phase）：
//! - `tests/fixtures/parse/**` → parse
//! - `tests/fixtures/build/**` → build
//! - `tests/fixtures/resolve/**` → resolve
//! - `tests/fixtures/resolve_multi/<case>/**` → resolve（多文件编译单元：按目录为单位）
//! - `tests/fixtures/resolve_cone/<case>/<cone>/**` → resolve（多 cone：每个 cone 子目录作为独立可见性边界）
//! - `tests/fixtures/typecheck_multi/<case>/**` → typecheck（多文件编译单元：按目录为单位）
//! - `tests/fixtures/typecheck_cone/<case>/<cone>/**` → typecheck（多 cone：每个 cone 子目录作为独立可见性边界）
//! - `tests/fixtures/typecheck_cone_archive/<case>/<pkg>/**` → typecheck（真实 `.cone` 依赖：先打包依赖，再注入 `api.scoopir`）
//! - `tests/fixtures/unsafe_nogc/**` → typecheck（系统编程通道：unsafe/NoGC/extern 的静态门禁）
//! - `tests/fixtures/comptime/**` → comptime（执行 `const val` 常量折叠并与 `.comptime` golden 比对）
//! - `tests/fixtures/codegen/**` / `tests/fixtures/run-pass/**` → run-pass
//! - `tests/fixtures/runtime_gc/**` → run-pass
//! - `tests/fixtures/run_pass_cone/<case>/**` → run-pass（cone 包：以目录为单位 build + exec）
//! - `tests/fixtures/infer/**` → infer
//! - `tests/fixtures/hir/**` → hir（HIR lowering + `.hir` golden 比对）
//! - `tests/fixtures/mir/**` → mir（MIR lowering + `.mir` golden 比对）
//! - `tests/fixtures/mir_refactor/**` → mir_refactor（refactor direct-style MIR stable dump + `.mir` golden 比对）
//! - `tests/fixtures/effect_facts/**` → effect_facts（refactor effect-facts stable dump + `.effectfacts` golden 比对）
//! - `tests/fixtures/effect_lowered/**` → effect_lowered（refactor late-lowered stable dump + `.effectlowered` golden 比对）
//! - `tests/fixtures/scoopir/**` → scoopir（public API 导出 + `.scoopir.json` golden 比对）
//! - 其它一级目录会被识别为 phase，但目前统一返回“未实现”的诊断。

mod expectations;
mod run_pass;

use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::Command;

use miette::Diagnostic;
use miette::{Context as _, IntoDiagnostic as _, Result, miette};
use scoopc::opt::OptLevel;
use scoopc::session::SessionOptions;
use thiserror::Error;

use expectations::{Expect, FixtureExpectation};

/// run-pass phase 运行时可注入的环境变量集合。
///
/// 说明：
/// - 该能力用于把 `scoop test` 的全局开关（例如 `--gc-stress/--gc-move/--threads`）映射为 env，
///   由 fixtures runner 在 run-pass 子进程上统一注入；
/// - fixture 文件内的 `// ENV:` 仍然优先级更高（同名 key 会覆盖这里的注入值）。
#[derive(Debug, Clone, Default)]
pub struct RunPassEnvOverrides {
    env: Vec<(String, String)>,
}

impl RunPassEnvOverrides {
    pub fn new() -> Self {
        Self { env: Vec::new() }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.env.push((key.into(), value.into()));
    }

    pub(crate) fn apply_to_command(&self, cmd: &mut Command) {
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
    }
}

pub(crate) fn current_scoop_exe_path() -> std::io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    if current.is_file() {
        return Ok(current);
    }

    if let Some(stripped) = strip_deleted_exe_suffix(&current)
        && stripped.is_file()
    {
        return Ok(stripped);
    }

    if let Some(argv0) = std::env::args_os().next() {
        let argv0 = PathBuf::from(argv0);
        if argv0.is_file() {
            return Ok(argv0);
        }
        if argv0.is_relative() {
            let cwd_candidate = std::env::current_dir()?.join(&argv0);
            if cwd_candidate.is_file() {
                return Ok(cwd_candidate);
            }
        }
    }

    Ok(current)
}

fn strip_deleted_exe_suffix(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_str()?;
    let stripped = file_name.strip_suffix(" (deleted)")?;
    Some(path.with_file_name(stripped))
}

fn new_fixture_session(session_options: SessionOptions) -> Result<scoopc::session::Session> {
    scoopc::session::Session::with_options(session_options).wrap_err("加载 fixtures session 失败")
}

pub fn run_all(
    fixtures_root: &Path,
    opt_level: Option<OptLevel>,
    session_options: SessionOptions,
    run_pass_env: &RunPassEnvOverrides,
) -> Result<usize> {
    if fixtures_root.is_file() {
        let session = new_fixture_session(session_options)?;
        let rel_root = fixtures_root.parent().unwrap_or(fixtures_root);
        run_one(&session, rel_root, fixtures_root, opt_level, run_pass_env)
            .wrap_err_with(|| format!("fixture 失败：{}", fixtures_root.display()))?;
        return Ok(1);
    }

    if is_resolve_multi_case_root(fixtures_root) {
        let session = new_fixture_session(session_options)?;
        let case_root = fixtures_root.parent().unwrap_or(fixtures_root);
        return run_resolve_multi_case(&session, case_root, fixtures_root)
            .wrap_err_with(|| format!("resolve_multi case 失败：{}", fixtures_root.display()));
    }
    if is_resolve_cone_case_root(fixtures_root) {
        let session = new_fixture_session(session_options)?;
        let case_root = fixtures_root.parent().unwrap_or(fixtures_root);
        return run_resolve_cone_case(&session, case_root, fixtures_root)
            .wrap_err_with(|| format!("resolve_cone case 失败：{}", fixtures_root.display()));
    }
    if is_typecheck_multi_case_root(fixtures_root) {
        let session = new_fixture_session(session_options)?;
        let case_root = fixtures_root.parent().unwrap_or(fixtures_root);
        return run_typecheck_multi_case(&session, case_root, fixtures_root)
            .wrap_err_with(|| format!("typecheck_multi case 失败：{}", fixtures_root.display()));
    }
    if is_typecheck_cone_case_root(fixtures_root) {
        let session = new_fixture_session(session_options)?;
        let case_root = fixtures_root.parent().unwrap_or(fixtures_root);
        return run_typecheck_cone_case(&session, case_root, fixtures_root)
            .wrap_err_with(|| format!("typecheck_cone case 失败：{}", fixtures_root.display()));
    }
    if is_typecheck_cone_archive_case_root(fixtures_root) {
        let session = new_fixture_session(session_options)?;
        let case_root = fixtures_root.parent().unwrap_or(fixtures_root);
        return run_typecheck_cone_archive_case(&session, case_root, fixtures_root).wrap_err_with(
            || {
                format!(
                    "typecheck_cone_archive case 失败：{}",
                    fixtures_root.display()
                )
            },
        );
    }
    if is_run_pass_cone_case_root(fixtures_root) {
        let run_pass_cone_root = fixtures_root.parent().unwrap_or(fixtures_root);
        return run_run_pass_cone_case(
            run_pass_cone_root,
            fixtures_root,
            opt_level,
            session_options,
            run_pass_env,
        )
        .wrap_err_with(|| format!("run_pass_cone case 失败：{}", fixtures_root.display()));
    }

    // T0307：`resolve_multi/<case>/` 采用“目录作为编译单元”的形式，因此需要把这些 `.scoop`
    // 从单文件扫描里排除，并由专门的 case 运行器以“多文件 + 单一 index”方式执行。
    let resolve_multi_root = fixtures_root.join("resolve_multi");
    let resolve_multi_cases = collect_resolve_multi_cases(&resolve_multi_root)?;
    // T0321a：`resolve_cone/<case>/<cone>/` 用于模拟“多个 cone（包/依赖边界）”。
    let resolve_cone_root = fixtures_root.join("resolve_cone");
    let resolve_cone_cases = collect_resolve_cone_cases(&resolve_cone_root)?;
    let typecheck_multi_root = fixtures_root.join("typecheck_multi");
    let typecheck_multi_cases = collect_typecheck_multi_cases(&typecheck_multi_root)?;
    let typecheck_cone_root = fixtures_root.join("typecheck_cone");
    let typecheck_cone_cases = collect_typecheck_cone_cases(&typecheck_cone_root)?;
    let typecheck_cone_archive_root = fixtures_root.join("typecheck_cone_archive");
    let typecheck_cone_archive_cases =
        collect_typecheck_cone_archive_cases(&typecheck_cone_archive_root)?;
    let run_pass_cone_root = run_pass_cone_root(fixtures_root);
    let run_pass_cone_cases = collect_run_pass_cone_cases(&run_pass_cone_root)?;

    let mut files = Vec::new();
    let mut skip_dirs: Vec<&Path> = Vec::new();
    if resolve_multi_root.is_dir() {
        skip_dirs.push(resolve_multi_root.as_path());
    }
    if resolve_cone_root.is_dir() {
        skip_dirs.push(resolve_cone_root.as_path());
    }
    if typecheck_multi_root.is_dir() {
        skip_dirs.push(typecheck_multi_root.as_path());
    }
    if typecheck_cone_root.is_dir() {
        skip_dirs.push(typecheck_cone_root.as_path());
    }
    if typecheck_cone_archive_root.is_dir() {
        skip_dirs.push(typecheck_cone_archive_root.as_path());
    }
    if run_pass_cone_root.is_dir() {
        skip_dirs.push(run_pass_cone_root.as_path());
    }
    collect_scoop_files(fixtures_root, &mut files, &skip_dirs)?;
    files.sort();

    if files.is_empty()
        && resolve_multi_cases.is_empty()
        && typecheck_multi_cases.is_empty()
        && typecheck_cone_cases.is_empty()
        && typecheck_cone_archive_cases.is_empty()
        && run_pass_cone_cases.is_empty()
    {
        return Err(miette!(
            "fixtures 目录下未发现任何 .scoop 文件：{}",
            fixtures_root.display()
        ));
    }

    let mut ok = 0usize;
    for file in files {
        // 每个独立 fixture 都要使用新 Session：sysroot AST 会携带可变的 typecheck/HIR side
        // tables，若跨 fixture 复用同一 Session，会让后续 golden/snapshot 变成顺序敏感。
        let session = new_fixture_session(session_options)?;
        run_one(&session, fixtures_root, &file, opt_level, run_pass_env)
            .wrap_err_with(|| format!("fixture 失败：{}", file.display()))?;
        ok += 1;
    }

    for case_dir in resolve_multi_cases {
        let session = new_fixture_session(session_options)?;
        ok += run_resolve_multi_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("resolve_multi case 失败：{}", case_dir.display()))?;
    }

    for case_dir in resolve_cone_cases {
        let session = new_fixture_session(session_options)?;
        ok += run_resolve_cone_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("resolve_cone case 失败：{}", case_dir.display()))?;
    }

    for case_dir in typecheck_multi_cases {
        let session = new_fixture_session(session_options)?;
        ok += run_typecheck_multi_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("typecheck_multi case 失败：{}", case_dir.display()))?;
    }

    for case_dir in typecheck_cone_cases {
        let session = new_fixture_session(session_options)?;
        ok += run_typecheck_cone_case(&session, fixtures_root, &case_dir)
            .wrap_err_with(|| format!("typecheck_cone case 失败：{}", case_dir.display()))?;
    }

    for case_dir in typecheck_cone_archive_cases {
        let session = new_fixture_session(session_options)?;
        ok += run_typecheck_cone_archive_case(&session, fixtures_root, &case_dir).wrap_err_with(
            || format!("typecheck_cone_archive case 失败：{}", case_dir.display()),
        )?;
    }

    for case_dir in run_pass_cone_cases {
        ok += run_run_pass_cone_case(
            fixtures_root,
            &case_dir,
            opt_level,
            session_options,
            run_pass_env,
        )
        .wrap_err_with(|| format!("run_pass_cone case 失败：{}", case_dir.display()))?;
    }

    Ok(ok)
}

fn has_parent_dir_name(path: &Path, name: &str) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|dir| dir == std::ffi::OsStr::new(name))
}

fn is_resolve_multi_case_root(fixtures_root: &Path) -> bool {
    fixtures_root.is_dir() && has_parent_dir_name(fixtures_root, "resolve_multi")
}

fn is_resolve_cone_case_root(fixtures_root: &Path) -> bool {
    fixtures_root.is_dir() && has_parent_dir_name(fixtures_root, "resolve_cone")
}

fn is_typecheck_multi_case_root(fixtures_root: &Path) -> bool {
    fixtures_root.is_dir() && has_parent_dir_name(fixtures_root, "typecheck_multi")
}

fn is_typecheck_cone_case_root(fixtures_root: &Path) -> bool {
    fixtures_root.is_dir() && has_parent_dir_name(fixtures_root, "typecheck_cone")
}

fn is_typecheck_cone_archive_case_root(fixtures_root: &Path) -> bool {
    fixtures_root.is_dir() && has_parent_dir_name(fixtures_root, "typecheck_cone_archive")
}

fn is_run_pass_cone_case_root(fixtures_root: &Path) -> bool {
    fixtures_root.is_dir()
        && has_parent_dir_name(fixtures_root, "run_pass_cone")
        && fixtures_root.join("Cone.toml").is_file()
        && fixtures_root.join("src").join("main.scoop").is_file()
}

fn run_pass_cone_root(fixtures_root: &Path) -> PathBuf {
    if fixtures_root
        .file_name()
        .is_some_and(|name| name == std::ffi::OsStr::new("run_pass_cone"))
    {
        fixtures_root.to_path_buf()
    } else {
        fixtures_root.join("run_pass_cone")
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("run_pass_cone case 缺少入口文件 `src/main.scoop`：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_pass_cone_missing_main_scoop))]
struct RunPassConeMissingMainScoop {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("run_pass_cone 未生成可执行文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_pass_cone_missing_exe))]
struct RunPassConeMissingExe {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 run-pass golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::run_pass_cone_golden_read_failed))]
struct RunPassConeGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

/// 运行一个 `tests/fixtures/run_pass_cone/<case>/` 用例（cone 包：以目录为单位 build + exec）。
///
/// 约定：
/// - case 目录本身是一个 cone root（包含 `Cone.toml` + `src/**.scoop`）；
/// - 期望与 golden 读取自 `src/main.scoop` 的文件头注释（复用 `FixtureExpectation` 语法）；
/// - 运行时 stdout/stderr 断言使用 `fixtures/run_pass.rs` 的公共执行器（捕获 + golden 对比）。
fn run_run_pass_cone_case(
    fixtures_root: &Path,
    case_dir: &Path,
    opt_level: Option<OptLevel>,
    session_options: SessionOptions,
    run_pass_env: &RunPassEnvOverrides,
) -> Result<usize> {
    let rel_case = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
    let expect_file_path = case_dir.join("src").join("main.scoop");

    if !expect_file_path.is_file() {
        return Err(RunPassConeMissingMainScoop {
            path: expect_file_path.display().to_string(),
            fixture: rel_case.display().to_string(),
        }
        .into());
    }

    let source = scoopc::source::SourceFile::load(&expect_file_path)?;
    let exp = FixtureExpectation::from_source(source.text());

    let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = (|| {
        // T1123：cone 项目目录下的 `scoop run` 应能：
        // - 在 build 目录为空时自动构建并运行（默认 debug）；
        // - `--release` 时运行 release 产物（输出目录不同）。
        let cone_root = case_dir.canonicalize().into_diagnostic()?;
        let manifest = scoopc::cone::ConeManifest::load_from_dir(&cone_root)?;

        // fixtures 需要自清理：先尽力清掉旧 build 产物，避免“上一次残留”影响本次断言。
        let build_root = cone_root.join("build");
        let _ = std::fs::remove_dir_all(&build_root);

        let scoop_exe = current_scoop_exe_path()
            .into_diagnostic()
            .wrap_err("无法定位当前 scoop 可执行文件")?;

        let want_release = exp.args.iter().any(|a| a == "--release");
        let profile = if want_release {
            crate::commands::build::BuildProfile::Release
        } else {
            crate::commands::build::BuildProfile::Debug
        };

        let exe = crate::commands::build::layout::cone_exe_path(
            &cone_root,
            None,
            profile.as_str(),
            &manifest.cone.name,
        );

        let out = match exp.expect {
            // `EXPECT: pass`：通过 `scoop run`（cone mode）端到端执行并断言 stdout/stderr。
            Expect::Pass => {
                // 与 run-pass fixtures 一致：未启用 LLVM 时仅做“golden 文件可读性”校验并跳过执行。
                if !cfg!(feature = "llvm") {
                    validate_run_pass_golden_files_readable(&expect_file_path, &exp)?;
                    return Ok(());
                }

                let cmd = build_run_pass_cone_run_command(
                    &scoop_exe,
                    case_dir,
                    opt_level,
                    session_options,
                    run_pass_env,
                    &exp,
                );
                run_pass::run_fixture_command(rel_case, &expect_file_path, &exp, cmd)?;

                if !exe.is_file() {
                    return Err(box_diagnostic(RunPassConeMissingExe {
                        path: exe.display().to_string(),
                        fixture: rel_case.display().to_string(),
                    }));
                }

                Ok(())
            }
            // `EXPECT: fail`：仍走 driver 内部执行路径，以便断言稳定错误码（例如 entry-package 校验）。
            Expect::Fail => {
                let build_result = crate::commands::build::run(
                    cone_root.clone(),
                    None,
                    crate::commands::build::BuildOptions {
                        profile: crate::commands::build::BuildProfile::Debug,
                        opt_level,
                        session_options,
                        ..crate::commands::build::BuildOptions::default()
                    },
                );

                match build_result {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        let e =
                            match e.downcast::<crate::commands::build::EntryPackageMissingMain>() {
                                Ok(diag) => return Err(box_diagnostic(diag)),
                                Err(e) => e,
                            };
                        let e = match e
                            .downcast::<crate::commands::build::EntryPackageMainNotInConsumerCone>()
                        {
                            Ok(diag) => return Err(box_diagnostic(diag)),
                            Err(e) => e,
                        };
                        let e =
                            match e.downcast::<crate::commands::build::EntryPackageOnlyForCone>() {
                                Ok(diag) => return Err(box_diagnostic(diag)),
                                Err(e) => e,
                            };

                        Err(box_report(e))
                    }
                }
            }
        };

        let _ = std::fs::remove_dir_all(&build_root);
        out
    })();

    match (exp.expect, result) {
        (Expect::Pass, Ok(())) => Ok(1),
        (Expect::Pass, Err(e)) => Err(miette!("期望通过，但执行失败：{e}")),
        (Expect::Fail, Ok(())) => Err(miette!("期望失败，但执行成功")),
        (Expect::Fail, Err(e)) => {
            assert_diagnostic_matches(&source, &exp, &*e)?;
            Ok(1)
        }
    }
}

fn build_run_pass_cone_run_command(
    scoop_exe: &Path,
    case_dir: &Path,
    opt_level: Option<OptLevel>,
    _session_options: SessionOptions,
    run_pass_env: &RunPassEnvOverrides,
    exp: &FixtureExpectation<'_>,
) -> Command {
    let mut cmd = Command::new(scoop_exe);
    cmd.arg("run");
    if let Some(level) = opt_level {
        let args_has_opt_level = exp
            .args
            .iter()
            .any(|a| a == "--opt-level" || a == "--opt_level" || a == "-O" || a.starts_with("-O"));
        if !args_has_opt_level {
            cmd.arg("--opt-level").arg(level.as_str());
        }
    }
    // 约定：run_pass_cone fixtures 的 `// ARGS:` 传给 `scoop run` 本身（例如 `--release`）。
    if !exp.args.is_empty() {
        cmd.args(&exp.args);
    }
    cmd.current_dir(case_dir);
    run_pass_env.apply_to_command(&mut cmd);
    cmd
}

fn validate_run_pass_golden_files_readable(
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    if let Some(stdout_rel) = exp.run_stdout {
        let path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(stdout_rel);
        let _ = std::fs::read_to_string(&path).map_err(|e| {
            box_diagnostic(RunPassConeGoldenReadFailed {
                path: path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;
    }

    if let Some(stderr_rel) = exp.run_stderr {
        let path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(stderr_rel);
        let _ = std::fs::read_to_string(&path).map_err(|e| {
            box_diagnostic(RunPassConeGoldenReadFailed {
                path: path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;
    }

    if let Some(stdin_rel) = exp.run_stdin {
        let path = fixture_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(stdin_rel);
        let _ = std::fs::read(&path).map_err(|e| {
            box_diagnostic(RunPassConeGoldenReadFailed {
                path: path.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;
    }

    Ok(())
}

fn run_one(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    path: &Path,
    opt_level: Option<OptLevel>,
    run_pass_env: &RunPassEnvOverrides,
) -> Result<()> {
    let source = scoopc::source::SourceFile::load(path)?;
    let exp = FixtureExpectation::from_source(source.text());
    // T0102/T0107：`// ARGS:`/`RUN-STDOUT`/`EXPECT-EXIT`/`TIMEOUT` 等指令会被解析并结构化存储；
    // 当前阶段只有部分 phase 会消费它们（例如 build phase 会消费 emit 相关 ARGS，run-pass 会消费 env/stdout/stderr 等）。

    let rel = path.strip_prefix(fixtures_root).unwrap_or(path);
    let phase = match phase_name(fixtures_root, rel) {
        None => FixturePhase::Parse,
        Some(name) if name == "parse" || name == "spec_doctest" => FixturePhase::Parse,
        Some(name) if name == "build" => FixturePhase::Build,
        Some(name) if name == "resolve" => FixturePhase::Resolve,
        Some(name) if name == "typecheck" || name == "unsafe_nogc" => FixturePhase::Typecheck,
        Some(name) if name == "infer" => FixturePhase::Infer,
        Some(name) if name == "comptime" => FixturePhase::Comptime,
        Some(name) if name == "codegen" || name == "run-pass" || name == "runtime_gc" => {
            FixturePhase::RunPass
        }
        Some(name) if name == "hir" => FixturePhase::Hir,
        Some(name) if name == "mir" => FixturePhase::Mir,
        Some(name) if name == "mir_refactor" => FixturePhase::MirRefactor,
        Some(name) if name == "effect_facts" => FixturePhase::EffectFacts,
        Some(name) if name == "effect_lowered" => FixturePhase::EffectLowered,
        Some(name) if name == "scoopir" => FixturePhase::ScoopIr,
        Some(other) => FixturePhase::Unimplemented(other.to_string_lossy().to_string()),
    };

    let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = match phase {
        FixturePhase::Parse => parse_fixture(session, &source, path, &exp),
        FixturePhase::Build => build_fixture(session, rel, path, opt_level, &exp),
        FixturePhase::Resolve => resolve_fixture(session, &source),
        FixturePhase::Typecheck => typecheck_fixture(session, &source, &exp),
        FixturePhase::Infer => infer_fixture(session, &source, &exp),
        FixturePhase::Comptime => comptime_fixture(session, &source, path),
        FixturePhase::RunPass => {
            run_pass::run_fixture(rel, path, opt_level, session.options(), &exp, run_pass_env)
        }
        FixturePhase::Hir => hir_fixture(session, &source, path),
        FixturePhase::Mir => mir_fixture(session, &source, path),
        FixturePhase::MirRefactor => mir_refactor_fixture(session, &source, path),
        FixturePhase::EffectFacts => effect_facts_fixture(session, &source, path),
        FixturePhase::EffectLowered => effect_lowered_fixture(session, &source, path),
        FixturePhase::ScoopIr => scoopir_fixture(session, &source, path),
        FixturePhase::Unimplemented(phase) => Err(box_diagnostic(UnimplementedPhase {
            phase,
            fixture: rel.display().to_string(),
        })),
    };

    match (exp.expect, result) {
        (Expect::Pass, Ok(())) => Ok(()),
        (Expect::Pass, Err(e)) => Err(miette!("期望通过，但执行失败：{e}")),
        (Expect::Fail, Ok(())) => Err(miette!("期望失败，但执行成功")),
        (Expect::Fail, Err(e)) => {
            assert_diagnostic_matches(&source, &exp, &*e)?;
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FixturePhase {
    Parse,
    Build,
    Resolve,
    Typecheck,
    Infer,
    Comptime,
    RunPass,
    Hir,
    Mir,
    MirRefactor,
    EffectFacts,
    EffectLowered,
    ScoopIr,
    Unimplemented(String),
}

#[derive(Debug, Error, Diagnostic)]
#[error("resolve_multi case 需要至少 2 个 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::resolve_multi_case_too_small))]
struct ResolveMultiCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("resolve_cone case 需要至少 2 个 cone 子目录（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::resolve_cone_case_too_small))]
struct ResolveConeCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "resolve_cone case 的 cone 子目录 `{cone}` 下未发现任何 `.scoop` 文件（fixture: {fixture}）"
)]
#[diagnostic(code(scoop::fixtures::resolve_cone_cone_empty))]
struct ResolveConeConeEmpty {
    fixture: String,
    cone: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_multi case 需要至少 2 个 `.scoop` 文件（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_multi_case_too_small))]
struct TypecheckMultiCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_cone case 需要至少 2 个 cone 子目录（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_cone_case_too_small))]
struct TypecheckConeCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "typecheck_cone case 的 cone 子目录 `{cone}` 下未发现任何 `.scoop` 文件（fixture: {fixture}）"
)]
#[diagnostic(code(scoop::fixtures::typecheck_cone_cone_empty))]
struct TypecheckConeConeEmpty {
    fixture: String,
    cone: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_cone_archive case 需要至少 2 个 package 子目录（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_cone_archive_case_too_small))]
struct TypecheckConeArchiveCaseTooSmall {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("typecheck_cone_archive case 需要且仅需要 1 个 consumer package（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::typecheck_cone_archive_consumer_not_unique))]
struct TypecheckConeArchiveConsumerNotUnique {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("EXPECT-MONOMORPH-HIT 不匹配：期望 {expected}，但得到 {found}")]
#[diagnostic(code(scoop::fixtures::monomorph_hit_mismatch))]
struct MonomorphHitMismatch {
    expected: usize,
    found: usize,
}

#[derive(Debug, Error, Diagnostic)]
#[error("EXPECT-MONOMORPH-MISS 不匹配：期望 {expected}，但得到 {found}")]
#[diagnostic(code(scoop::fixtures::monomorph_miss_mismatch))]
struct MonomorphMissMismatch {
    expected: usize,
    found: usize,
}

#[derive(Debug, Error, Diagnostic)]
#[error("EXPECT-TYPE-MONOMORPH-HIT 不匹配：期望 {expected}，但得到 {found}")]
#[diagnostic(code(scoop::fixtures::type_monomorph_hit_mismatch))]
struct TypeMonomorphHitMismatch {
    expected: usize,
    found: usize,
}

#[derive(Debug, Error, Diagnostic)]
#[error("EXPECT-TYPE-MONOMORPH-MISS 不匹配：期望 {expected}，但得到 {found}")]
#[diagnostic(code(scoop::fixtures::type_monomorph_miss_mismatch))]
struct TypeMonomorphMissMismatch {
    expected: usize,
    found: usize,
}

#[derive(Debug, Error, Diagnostic)]
#[error("fixtures phase `{phase}` 未实现（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::unimplemented_phase))]
struct UnimplementedPhase {
    phase: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 AST golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::ast_golden_read_failed))]
struct AstGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("AST snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::ast_golden_mismatch))]
struct AstGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 comptime golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::comptime_golden_read_failed))]
struct ComptimeGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("comptime snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::comptime_golden_mismatch))]
struct ComptimeGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 HIR golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::hir_golden_read_failed))]
struct HirGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("HIR snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::hir_golden_mismatch))]
struct HirGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 MIR golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::mir_golden_read_failed))]
struct MirGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "MIR snapshot 与 golden 不一致：{path}（fixture: {fixture}；line {line}；expected: {expected_line}；actual: {actual_line}）"
)]
#[diagnostic(code(scoop::fixtures::mir_golden_mismatch))]
struct MirGoldenMismatch {
    path: String,
    fixture: String,
    line: usize,
    expected_line: String,
    actual_line: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 effect-facts golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::effect_facts_golden_read_failed))]
struct EffectFactsGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("effect-facts snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::effect_facts_golden_mismatch))]
struct EffectFactsGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 effect-lowered golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::effect_lowered_golden_read_failed))]
struct EffectLoweredGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("effect-lowered snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::effect_lowered_golden_mismatch))]
struct EffectLoweredGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 ScoopIR golden 文件：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::scoopir_golden_read_failed))]
struct ScoopIrGoldenReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("ScoopIR snapshot 与 golden 不一致：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::scoopir_golden_mismatch))]
struct ScoopIrGoldenMismatch {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("ScoopIR JSON 序列化失败：{fixture}")]
#[diagnostic(code(scoop::fixtures::scoopir_json_serialize_failed))]
struct ScoopIrJsonSerializeFailed {
    fixture: String,
    #[source]
    source: serde_json::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures 未生成期望产物：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_artifact_missing))]
struct BuildArtifactMissing {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures 产物为空：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_artifact_empty))]
struct BuildArtifactEmpty {
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法创建 build fixtures 输出目录：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_output_dir_create_failed))]
struct BuildOutputDirCreateFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 build fixtures 产物元数据：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_artifact_metadata_failed))]
struct BuildArtifactMetadataFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("无法读取 build fixtures LLVM IR 产物：{path}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_llvm_ir_read_failed))]
struct BuildLlvmIrReadFailed {
    path: String,
    fixture: String,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures LLVM IR 未包含期望子串：{substring}（fixture: {fixture}；path: {path}）")]
#[diagnostic(code(scoop::fixtures::build_llvm_ir_missing_substring))]
struct BuildLlvmIrMissingSubstring {
    substring: String,
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures LLVM IR 包含禁止子串：{substring}（fixture: {fixture}；path: {path}）")]
#[diagnostic(code(scoop::fixtures::build_llvm_ir_unexpected_substring))]
struct BuildLlvmIrUnexpectedSubstring {
    substring: String,
    path: String,
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures LLVM IR 子串断言需要 `--emit-llvm`（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_llvm_ir_assert_requires_emit_llvm))]
struct BuildLlvmIrAssertRequiresEmitLlvm {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures 指令缺少 opt-level 参数值（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_missing_opt_level_value))]
struct BuildMissingOptLevelValue {
    fixture: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("build fixtures 指令包含无效的 opt-level：{value}（fixture: {fixture}）")]
#[diagnostic(code(scoop::fixtures::build_invalid_opt_level))]
struct BuildInvalidOptLevel {
    value: String,
    fixture: String,
}

fn parse_opt_level_from_fixture_args(
    args: &[String],
    fixture_path: &Path,
) -> std::result::Result<Option<OptLevel>, Box<dyn miette::Diagnostic>> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        // 支持 `-O2` / `-Os` / `-Oz`
        if let Some(rest) = arg.strip_prefix("-O") {
            if rest.is_empty() {
                // 支持 `-O 2`
                let Some(value) = iter.next() else {
                    return Err(box_diagnostic(BuildMissingOptLevelValue {
                        fixture: fixture_path.display().to_string(),
                    }));
                };
                return OptLevel::parse(value).map(Some).map_err(|e| {
                    box_diagnostic(BuildInvalidOptLevel {
                        value: e.value,
                        fixture: fixture_path.display().to_string(),
                    })
                });
            }

            return OptLevel::parse(rest).map(Some).map_err(|e| {
                box_diagnostic(BuildInvalidOptLevel {
                    value: e.value,
                    fixture: fixture_path.display().to_string(),
                })
            });
        }

        // 支持 `--opt-level 2`
        if arg == "--opt-level" {
            let Some(value) = iter.next() else {
                return Err(box_diagnostic(BuildMissingOptLevelValue {
                    fixture: fixture_path.display().to_string(),
                }));
            };
            return OptLevel::parse(value).map(Some).map_err(|e| {
                box_diagnostic(BuildInvalidOptLevel {
                    value: e.value,
                    fixture: fixture_path.display().to_string(),
                })
            });
        }

        // 支持 `--opt-level=2`
        if let Some(rest) = arg.strip_prefix("--opt-level=") {
            return OptLevel::parse(rest).map(Some).map_err(|e| {
                box_diagnostic(BuildInvalidOptLevel {
                    value: e.value,
                    fixture: fixture_path.display().to_string(),
                })
            });
        }
    }

    Ok(None)
}

fn build_fixture(
    session: &scoopc::session::Session,
    rel_fixture: &Path,
    fixture_path: &Path,
    opt_level: Option<OptLevel>,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let emit_llvm = exp.args.iter().any(|a| a == "--emit-llvm");
    let emit_obj = exp.args.iter().any(|a| a == "--emit-obj");
    let emit_asm = exp.args.iter().any(|a| a == "--emit-asm");
    let emit_requested = emit_llvm || emit_obj || emit_asm;
    let needs_llvm_ir_assertions =
        !exp.build_llvm_contains.is_empty() || !exp.build_llvm_not_contains.is_empty();

    // 约定：build fixtures 主要用于产出后端相关单文件产物（`.ll`/`.o`/`.s`）做排查；
    // 若未请求 emit，则该 fixture 在当前阶段视为“无操作”直接通过。
    if !emit_requested {
        return Ok(());
    }

    if needs_llvm_ir_assertions && !emit_llvm {
        return Err(box_diagnostic(BuildLlvmIrAssertRequiresEmitLlvm {
            fixture: fixture_path.display().to_string(),
        }));
    }

    let emit = if emit_llvm {
        crate::commands::build::BuildEmit::LlvmIr
    } else if emit_obj {
        crate::commands::build::BuildEmit::Obj
    } else {
        crate::commands::build::BuildEmit::Asm
    };

    // 与 run-pass fixtures 一致：当未启用 LLVM 后端时，为保持 `scoop test` 可回归，这里跳过实际产物生成。
    if !cfg!(feature = "llvm") {
        return Ok(());
    }

    // 约定：build fixtures 可通过 `// ARGS: -O2` / `--opt-level 2` 在单个 fixture 内固定优化等级，
    // 用于回归“优化确实发生”（T1602）。全局 `scoop test -O...` 仍然优先级更高。
    let opt_level = opt_level.or(parse_opt_level_from_fixture_args(&exp.args, fixture_path)?);

    let mut out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fixtures")
        .join(rel_fixture);
    let ext = match emit {
        crate::commands::build::BuildEmit::LlvmIr => "ll",
        crate::commands::build::BuildEmit::Obj => {
            if cfg!(windows) {
                "obj"
            } else {
                "o"
            }
        }
        crate::commands::build::BuildEmit::Asm => {
            if cfg!(windows) {
                "asm"
            } else {
                "s"
            }
        }
        crate::commands::build::BuildEmit::Executable => std::env::consts::EXE_EXTENSION,
    };
    out.set_extension(ext);

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            box_diagnostic(BuildOutputDirCreateFailed {
                path: parent.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;
    }
    let _ = std::fs::remove_file(&out);

    crate::commands::build::run(
        fixture_path.to_path_buf(),
        Some(out.clone()),
        crate::commands::build::BuildOptions {
            emit,
            opt_level,
            session_options: session.options(),
            ..crate::commands::build::BuildOptions::default()
        },
    )
    .map_err(box_report)?;

    if !out.is_file() {
        return Err(box_diagnostic(BuildArtifactMissing {
            path: out.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    let size = std::fs::metadata(&out)
        .map_err(|e| {
            box_diagnostic(BuildArtifactMetadataFailed {
                path: out.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?
        .len();
    if size == 0 {
        return Err(box_diagnostic(BuildArtifactEmpty {
            path: out.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    if needs_llvm_ir_assertions && matches!(emit, crate::commands::build::BuildEmit::LlvmIr) {
        let ir = std::fs::read_to_string(&out).map_err(|e| {
            box_diagnostic(BuildLlvmIrReadFailed {
                path: out.display().to_string(),
                fixture: fixture_path.display().to_string(),
                source: e,
            })
        })?;

        for &substring in &exp.build_llvm_contains {
            if !ir.contains(substring) {
                return Err(box_diagnostic(BuildLlvmIrMissingSubstring {
                    substring: substring.to_string(),
                    path: out.display().to_string(),
                    fixture: fixture_path.display().to_string(),
                }));
            }
        }

        for &substring in &exp.build_llvm_not_contains {
            if ir.contains(substring) {
                return Err(box_diagnostic(BuildLlvmIrUnexpectedSubstring {
                    substring: substring.to_string(),
                    path: out.display().to_string(),
                    fixture: fixture_path.display().to_string(),
                }));
            }
        }
    }

    Ok(())
}

fn parse_file_via_ast_stage(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> std::result::Result<scoopc::ast::File, scoopc::parser::ParseError> {
    scoopc::pipeline::load_ast_stage_output_for_dump(session, source)
        .map(scoopc::pipeline::AstStageOutput::into_ast)
}

fn parse_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let ast = parse_file_via_ast_stage(session, source).map_err(box_diagnostic)?;

    let Some(golden_rel) = exp.ast_golden else {
        return Ok(());
    };

    let golden_path = fixture_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(golden_rel);

    let expected = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(AstGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;

    let actual = format!("{ast:#?}\n");
    let expected = normalize_newlines(&expected);
    let actual = normalize_newlines(&actual);

    if expected != actual {
        return Err(box_diagnostic(AstGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn comptime_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let ast = parse_file_via_ast_stage(session, source).map_err(box_diagnostic)?;
    let bindings = scoopc::comptime::eval_const_bindings_in_file(session.sysroot(), source, &ast)
        .map_err(box_diagnostic)?;

    let actual = normalize_newlines(&format_const_bindings_for_fixture(&bindings));

    let golden_path = fixture_path.with_extension("comptime");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(ComptimeGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(ComptimeGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn format_const_bindings_for_fixture(bindings: &[scoopc::comptime::ConstBinding]) -> String {
    let mut out = String::new();
    for b in bindings {
        out.push_str(&b.name);
        out.push_str(" = ");
        out.push_str(&format_const_value_for_fixture(&b.value));
        out.push('\n');
    }
    out
}

fn format_const_value_for_fixture(v: &scoopc::comptime::ConstValue) -> String {
    use scoopc::comptime::{ConstEnum, ConstStruct, ConstValue};

    match v {
        ConstValue::Unit => "()".to_string(),
        ConstValue::Bool(b) => b.to_string(),
        ConstValue::Char(ch) => format!("{ch:?}"),
        ConstValue::Int(i) => {
            if i.ty.signed {
                i.as_i128().to_string()
            } else {
                i.as_u128().to_string()
            }
        }
        ConstValue::Float(f) => format_const_float_for_fixture(*f),
        ConstValue::String(s) => format!("{s:?}"),
        ConstValue::Tuple(items) => {
            let inner = items
                .iter()
                .map(format_const_value_for_fixture)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({inner})")
        }
        ConstValue::Struct(ConstStruct { ty, fields }) => {
            if fields.is_empty() {
                return format!("{ty} {{}}");
            }
            let inner = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_const_value_for_fixture(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{ty} {{ {inner} }}")
        }
        ConstValue::Enum(ConstEnum {
            ty,
            variant,
            payload,
        }) => {
            let mut out = String::new();
            if let Some(ty) = ty {
                out.push_str(ty);
                out.push('.');
            }
            out.push_str(variant);
            if !payload.is_empty() {
                let inner = payload
                    .iter()
                    .map(format_const_value_for_fixture)
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push('(');
                out.push_str(&inner);
                out.push(')');
            }
            out
        }
    }
}

fn format_const_float_for_fixture(f: scoopc::comptime::ConstFloat) -> String {
    match f {
        scoopc::comptime::ConstFloat::Float64(bits) => {
            normalize_float_text(f64::from_bits(bits).to_string(), f64::from_bits(bits))
        }
        scoopc::comptime::ConstFloat::Float32(bits) => {
            let value = f32::from_bits(bits);
            format!(
                "{}f32",
                normalize_float_text(value.to_string(), f64::from(value))
            )
        }
    }
}

fn normalize_float_text(mut text: String, value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn hir_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let lowered =
        scoopc::pipeline::lower_typed_hir_for_dump(session, source).map_err(box_diagnostic)?;
    let actual = normalize_newlines(&format!("{:#?}\n", lowered.file));

    let golden_path = fixture_path.with_extension("hir");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(HirGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(HirGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn mir_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let lowered = scoopc::pipeline::lower_direct_style_mir_for_dump(session, source)
        .map_err(box_diagnostic)?;
    let actual = normalize_newlines(&format!("{:#?}\n", lowered.file));

    assert_mir_golden_matches(&actual, fixture_path)
}

fn mir_refactor_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let output = scoopc::pipeline::load_direct_style_mir_stage_output_for_dump(session, source)
        .map_err(box_diagnostic)?;
    if std::env::var_os("SCOOP_FIXTURE_REPRO_DIR").is_some() {
        let _ = std::fs::write(
            fixture_path.with_extension("actual.raw.mir"),
            format!("{:#?}\n", output.file()),
        );
    }
    let actual = normalize_newlines(&output.stable_dump());

    assert_mir_golden_matches(&actual, fixture_path)
}

fn effect_facts_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let actual = crate::commands::dump_effect_facts::render_effect_facts_output(session, source)
        .map_err(box_report)?;
    let actual = normalize_newlines(&actual);

    assert_effect_facts_golden_matches(&actual, fixture_path)
}

fn effect_lowered_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let actual =
        crate::commands::dump_effect_lowered::render_effect_lowered_output(session, source)
            .map_err(box_report)?;
    let actual = normalize_newlines(&actual);

    assert_effect_lowered_golden_matches(&actual, fixture_path)
}

fn assert_mir_golden_matches(
    actual: &str,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let golden_path = fixture_path.with_extension("mir");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(MirGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);
    if std::env::var_os("SCOOP_FIXTURE_REPRO_DIR").is_some() {
        let _ = std::fs::write(fixture_path.with_extension("actual.mir"), actual);
    }

    if expected != actual {
        let (line, expected_line, actual_line) = first_mismatch_line(&expected, actual);
        return Err(box_diagnostic(MirGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            line,
            expected_line,
            actual_line,
        }));
    }

    Ok(())
}

fn first_mismatch_line(expected: &str, actual: &str) -> (usize, String, String) {
    let expected_lines = expected.lines().collect::<Vec<_>>();
    let actual_lines = actual.lines().collect::<Vec<_>>();
    let max_len = expected_lines.len().max(actual_lines.len());

    for idx in 0..max_len {
        let expected_line = expected_lines.get(idx).copied().unwrap_or("<missing>");
        let actual_line = actual_lines.get(idx).copied().unwrap_or("<missing>");
        if expected_line != actual_line {
            return (idx + 1, expected_line.to_string(), actual_line.to_string());
        }
    }

    (0, "<identical>".to_string(), "<identical>".to_string())
}

fn assert_effect_facts_golden_matches(
    actual: &str,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let golden_path = fixture_path.with_extension("effectfacts");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(EffectFactsGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(EffectFactsGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn assert_effect_lowered_golden_matches(
    actual: &str,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let golden_path = fixture_path.with_extension("effectlowered");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(EffectLoweredGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(EffectLoweredGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn scoopir_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    fixture_path: &Path,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // v0：导出 public type/fun header，不包含函数体。
    let mut ast = parse_file_via_ast_stage(session, source).map_err(box_diagnostic)?;
    {
        let sources = [source];
        let mut files = [&mut ast];
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )
        .map_err(box_diagnostic)?;
    }

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));
    let index = scoopc::resolve::Index::build(&pairs).map_err(box_diagnostic)?;

    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(box_diagnostic)?;
    env.extend_from_file(source, &ast, &index)
        .map_err(box_diagnostic)?;

    let hir =
        scoopc::pipeline::lower_typed_hir_for_dump(session, source).map_err(box_diagnostic)?;
    let ir = scoopc::cone::scoopir::export_public_api_for_source(source, &index, &env, &hir)
        .map_err(box_boxed_diagnostic)?;

    let actual_raw = serde_json::to_string_pretty(&ir).map_err(|e| {
        box_diagnostic(ScoopIrJsonSerializeFailed {
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let actual = normalize_newlines(&format!("{actual_raw}\n"));

    let golden_path = fixture_path.with_extension("scoopir.json");
    let expected_raw = std::fs::read_to_string(&golden_path).map_err(|e| {
        box_diagnostic(ScoopIrGoldenReadFailed {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
            source: e,
        })
    })?;
    let expected = normalize_newlines(&expected_raw);

    if expected != actual {
        return Err(box_diagnostic(ScoopIrGoldenMismatch {
            path: golden_path.display().to_string(),
            fixture: fixture_path.display().to_string(),
        }));
    }

    Ok(())
}

fn resolve_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let mut ast = parse_file_via_ast_stage(session, source).map_err(box_diagnostic)?;
    {
        let sources = [source];
        let mut files = [&mut ast];
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )
        .map_err(box_diagnostic)?;
    }

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = scoopc::resolve::Index::build(&pairs).map_err(box_diagnostic)?;
    scoopc::resolve::check_file_bindings(source, &mut ast, &index).map_err(box_diagnostic)?;
    Ok(())
}

fn typecheck_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    let mut ast = parse_file_via_ast_stage(session, source).map_err(box_diagnostic)?;
    {
        let sources = [source];
        let mut files = [&mut ast];
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )
        .map_err(box_diagnostic)?;
    }

    // 先运行不依赖 resolver/index 的 typecheck 预检查：
    // - T0404：声明头类型注解的最小约束
    // - T0409：struct 字段声明的最小约束（重复字段、`var`、默认值）
    scoopc::typecheck::check_file_headers(source, &ast).map_err(box_diagnostic)?;
    scoopc::typecheck::check_file_struct_decls(source, &ast).map_err(box_diagnostic)?;

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = scoopc::resolve::Index::build(&pairs).map_err(box_diagnostic)?;

    // typecheck phase 的前置条件：签名中的类型引用应当已 resolve（至少保证存在性/可见性）。
    let headers =
        scoopc::resolve::check_file_headers(source, &ast, &index).map_err(box_diagnostic)?;

    // T0406：表达式类型检查需要 resolver 在 AST 上写回 ValueIdent.resolved。
    // 因此 typecheck phase 在通过 headers 解析后，还需要进一步解析函数体/initializer（bodies）。
    //
    // 说明：
    // - 这里复用 resolver 的 block scope + value ident 解析逻辑（T0304/T0305/T0308）；
    // - 若 bodies 中存在未定义值引用，将以 resolve 错误提前失败（避免后续 typecheck 重复报错）。
    scoopc::resolve::check_file_bodies(source, &mut ast, &index, &headers)
        .map_err(box_diagnostic)?;

    // 构建 type env：sysroot + 当前文件（用于跨文件 type position 查询）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(box_diagnostic)?;
    env.extend_from_file(source, &ast, &index)
        .map_err(box_diagnostic)?;

    // T1326c：允许 typecheck fixtures 通过 `// ARGS: --target-platform <id>` 覆盖目标平台，
    // 用于回归“平台能力 gate”的诊断行为（不影响默认 host 行为）。
    if let Some(platform_id) = parse_target_platform_from_fixture_args(&exp.args) {
        env.set_target_platform(scoopc::target::TargetPlatform::new(platform_id));
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    scoopc::typecheck::check_file_annotations(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    // T0431/T0432：属性（class/value type）的最小语义检查。
    scoopc::typecheck::check_file_properties(source, &ast, &index, &env)
        .map_err(box_boxed_diagnostic)?;
    // T0439：class 继承与 override 的最小语义检查。
    scoopc::typecheck::check_file_inheritance(source, &ast, &index).map_err(box_diagnostic)?;

    // T0440：interface 实现列表 + 抽象成员实现检查（默认方法不要求实现）。
    scoopc::typecheck::check_file_interfaces(source, &ast, &index, &env).map_err(box_diagnostic)?;
    // T0609：override/interface impl 的 effect row 不能增加（R_over ⊆ R_base）。
    scoopc::typecheck::check_file_override_effects(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_boxed_diagnostic)?;

    scoopc::typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    scoopc::typecheck::check_file_where_clauses(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    scoopc::typecheck::check_file_overload_conflicts(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    scoopc::typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .map_err(box_diagnostic)?;

    // T0449：计算 enum/Option 的布局元数据（niche/boxing/lint）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(box_diagnostic)?;

    Ok(())
}

fn parse_target_platform_from_fixture_args(args: &[String]) -> Option<String> {
    // 目前只为 fixtures 引入一个非常小的解析器：
    // - `--target-platform <id>`
    // - `--target-platform=<id>`
    //
    // 说明：这不是 `scoop` CLI 的稳定接口；仅用于回归 typecheck 的 platform gating 行为。
    let mut it = args.iter().peekable();
    while let Some(arg) = it.next() {
        if let Some(v) = arg.strip_prefix("--target-platform=") {
            return Some(v.to_string());
        }
        if arg == "--target-platform"
            && let Some(v) = it.peek()
        {
            return Some((*v).to_string());
        }
    }
    None
}

fn infer_fixture(
    session: &scoopc::session::Session,
    source: &scoopc::source::SourceFile,
    _exp: &FixtureExpectation<'_>,
) -> std::result::Result<(), Box<dyn miette::Diagnostic>> {
    // `infer` fixtures 必须消费 authoritative typed HIR 主线，确保观测到唯一 production
    // frontend 发布的 receiver/call contract 诊断。
    scoopc::pipeline::load_typed_hir_stage_output_for_dump(session, source)
        .map(|_| ())
        .map_err(box_diagnostic)
}

/// 运行一个 `tests/fixtures/resolve_multi/<case>/` 的多文件编译单元。
///
/// 规则（当前阶段）：
/// - case 目录下必须有 2+ 个 `.scoop` 文件
/// - 先把 case 内所有文件 + sysroot 一起构建 `Index`
/// - 再对 case 内每个文件分别运行 `check_file_bindings`，并按各自文件头注释断言 pass/fail
fn run_resolve_multi_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut paths = Vec::new();
    collect_scoop_files(case_dir, &mut paths, &[])?;
    paths.sort();

    if paths.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(ResolveMultiCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    let mut sources = Vec::with_capacity(paths.len());
    let mut asts = Vec::with_capacity(paths.len());
    for path in &paths {
        let source = scoopc::source::SourceFile::load(path)?;
        let ast = parse_file_via_ast_stage(session, &source).map_err(miette::Report::new)?;
        sources.push(source);
        asts.push(ast);
    }
    {
        let source_refs = sources.iter().collect::<Vec<_>>();
        let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )
        .map_err(miette::Report::new)?;
    }

    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    for (s, a) in sources.iter().zip(asts.iter()) {
        pairs.push((s, a));
    }

    let index = scoopc::resolve::Index::build(&pairs).map_err(miette::Report::new)?;

    for (source, ast) in sources.iter().zip(asts.iter_mut()) {
        let exp = FixtureExpectation::from_source(source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> =
            scoopc::resolve::check_file_bindings(source, ast, &index).map_err(box_diagnostic);

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(source, &exp, &*e)?;
            }
        }
    }

    Ok(paths.len())
}

/// 运行一个 `tests/fixtures/resolve_cone/<case>/<cone>/` 的“多 cone”用例。
///
/// 规则（当前阶段，T0321a）：
/// - case 目录下必须有 2+ 个 cone 子目录（每个子目录代表一个 cone/依赖边界）
/// - 每个 cone 子目录下至少有 1 个 `.scoop` 文件
/// - 将所有 cone 的文件 + sysroot 一起构建 `Index`（但每个文件携带不同的 cone id）
/// - 对 cone 内每个文件分别运行 `check_file_bindings`，并按各自文件头注释断言 pass/fail
fn run_resolve_cone_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut cone_dirs = Vec::new();
    for entry in std::fs::read_dir(case_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", case_dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_dir() {
            cone_dirs.push(entry.path());
        }
    }

    cone_dirs.sort();
    if cone_dirs.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(ResolveConeCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    struct ConeFile {
        cone: scoopc::resolve::ConeId,
        source: scoopc::source::SourceFile,
        ast: scoopc::ast::File,
    }

    let mut files: Vec<ConeFile> = Vec::new();
    let mut ok = 0usize;

    for (idx, cone_dir) in cone_dirs.iter().enumerate() {
        let mut paths = Vec::new();
        collect_scoop_files(cone_dir, &mut paths, &[])?;
        paths.sort();

        if paths.is_empty() {
            let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
            let cone = cone_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            return Err(ResolveConeConeEmpty {
                fixture: rel.display().to_string(),
                cone,
            }
            .into());
        }

        // cone id 0 保留给 sysroot；fixture 的 cone 从 1 开始稳定分配（按目录名排序）。
        let cone_id = scoopc::resolve::ConeId::new((idx as u32) + 1);

        for path in paths {
            let source = scoopc::source::SourceFile::load(&path)?;
            let ast = parse_file_via_ast_stage(session, &source).map_err(miette::Report::new)?;
            files.push(ConeFile {
                cone: cone_id,
                source,
                ast,
            });
            ok += 1;
        }
    }

    {
        let ambient_files = session
            .sysroot()
            .files
            .iter()
            .map(|file| scoopc::resolve::IndexedFile {
                cone: scoopc::resolve::ConeId::DEFAULT,
                source: &file.source,
                file: &file.ast,
            })
            .collect::<Vec<_>>();
        let source_entries = files
            .iter()
            .map(|file| (file.cone, file.source.clone()))
            .collect::<Vec<_>>();
        let indexed_sources = source_entries
            .iter()
            .map(|(cone, source)| (*cone, source))
            .collect::<Vec<_>>();
        let mut ast_refs = files
            .iter_mut()
            .map(|file| &mut file.ast)
            .collect::<Vec<_>>();
        scoopc::comptime::trim_package_level_comptime_ifs_in_indexed_compilation_unit(
            &ambient_files,
            &indexed_sources,
            &mut ast_refs,
        )
        .map_err(miette::Report::new)?;
    }

    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for f in &files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: f.cone,
            source: &f.source,
            file: &f.ast,
        });
    }

    let index = scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::new)?;

    for f in files.iter_mut() {
        let exp = FixtureExpectation::from_source(f.source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> =
            scoopc::resolve::check_file_bindings(&f.source, &mut f.ast, &index)
                .map_err(box_diagnostic);

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(&f.source, &exp, &*e)?;
            }
        }
    }

    Ok(ok)
}

/// 运行一个 `tests/fixtures/typecheck_cone/<case>/<cone>/` 的“多 cone”用例。
///
/// 规则（当前阶段，T0629a）：
/// - case 目录下必须有 2+ 个 cone 子目录（每个子目录代表一个 cone/依赖边界）
/// - 每个 cone 子目录下至少有 1 个 `.scoop` 文件
/// - 将所有 cone 的文件 + sysroot 一起构建 `Index`（每个文件携带不同的 cone id）
/// - 构建 type env：sysroot + 全部 cone 的文件（用于跨 cone 的 TypeRef lowering）
/// - 对 cone 内每个文件分别运行 typecheck pipeline，并按各自文件头注释断言 pass/fail
fn run_typecheck_cone_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut cone_dirs = Vec::new();
    for entry in std::fs::read_dir(case_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", case_dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_dir() {
            cone_dirs.push(entry.path());
        }
    }

    cone_dirs.sort();
    if cone_dirs.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(TypecheckConeCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    struct ConeFile {
        cone: scoopc::resolve::ConeId,
        source: scoopc::source::SourceFile,
        ast: scoopc::ast::File,
    }

    let mut files: Vec<ConeFile> = Vec::new();
    let mut ok = 0usize;

    for (idx, cone_dir) in cone_dirs.iter().enumerate() {
        let mut paths = Vec::new();
        collect_scoop_files(cone_dir, &mut paths, &[])?;
        paths.sort();

        if paths.is_empty() {
            let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
            let cone = cone_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            return Err(TypecheckConeConeEmpty {
                fixture: rel.display().to_string(),
                cone,
            }
            .into());
        }

        // cone id 0 保留给 sysroot；fixture 的 cone 从 1 开始稳定分配（按目录名排序）。
        let cone_id = scoopc::resolve::ConeId::new((idx as u32) + 1);

        for path in paths {
            let source = scoopc::source::SourceFile::load(&path)?;
            let ast = parse_file_via_ast_stage(session, &source).map_err(miette::Report::new)?;
            files.push(ConeFile {
                cone: cone_id,
                source,
                ast,
            });
            ok += 1;
        }
    }

    {
        let ambient_files = session
            .sysroot()
            .files
            .iter()
            .map(|file| scoopc::resolve::IndexedFile {
                cone: scoopc::resolve::ConeId::DEFAULT,
                source: &file.source,
                file: &file.ast,
            })
            .collect::<Vec<_>>();
        let source_entries = files
            .iter()
            .map(|file| (file.cone, file.source.clone()))
            .collect::<Vec<_>>();
        let indexed_sources = source_entries
            .iter()
            .map(|(cone, source)| (*cone, source))
            .collect::<Vec<_>>();
        let mut ast_refs = files
            .iter_mut()
            .map(|file| &mut file.ast)
            .collect::<Vec<_>>();
        scoopc::comptime::trim_package_level_comptime_ifs_in_indexed_compilation_unit(
            &ambient_files,
            &indexed_sources,
            &mut ast_refs,
        )
        .map_err(miette::Report::new)?;
    }

    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for f in &files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: f.cone,
            source: &f.source,
            file: &f.ast,
        });
    }

    let index = scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::new)?;

    // type env：sysroot + 全部 cone 的文件（用于跨 cone TypeRef lowering）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::new)?;
    for f in &files {
        env.extend_from_file(&f.source, &f.ast, &index)
            .map_err(miette::Report::new)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    for f in files.iter_mut() {
        let exp = FixtureExpectation::from_source(f.source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = (|| {
            // 先运行不依赖 resolver/index 的 typecheck 预检查。
            scoopc::typecheck::check_file_headers(&f.source, &f.ast).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_struct_decls(&f.source, &f.ast)
                .map_err(box_diagnostic)?;

            // resolver phase：headers + bodies。
            let headers = scoopc::resolve::check_file_headers(&f.source, &f.ast, &index)
                .map_err(box_diagnostic)?;
            scoopc::resolve::check_file_bodies(&f.source, &mut f.ast, &index, &headers)
                .map_err(box_diagnostic)?;

            // typecheck phase。
            scoopc::typecheck::check_file_annotations(
                &f.source,
                &f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_properties(&f.source, &f.ast, &index, &env)
                .map_err(box_boxed_diagnostic)?;
            scoopc::typecheck::check_file_inheritance(&f.source, &f.ast, &index)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_interfaces(&f.source, &f.ast, &index, &env)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_override_effects(
                &f.source,
                &f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_boxed_diagnostic)?;
            scoopc::typecheck::check_file_type_refs(
                &f.source,
                &f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_where_clauses(
                &f.source,
                &f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_overload_conflicts(
                &f.source,
                &f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_exprs(
                &f.source,
                &f.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            Ok(())
        })();

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(&f.source, &exp, &*e)?;
            }
        }
    }

    // 对整个编译单元中出现过的类型做一次 layout/metadata 计算（与 typecheck_multi 对齐）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::new)?;

    Ok(ok)
}

/// 运行一个 `tests/fixtures/typecheck_cone_archive/<case>/<pkg>/` 的“真实 `.cone` 依赖注入”用例。
///
/// 规则（当前阶段，T1105）：
/// - case 目录下必须有 2+ 个 package 子目录（每个子目录为一个 cone root：包含 `Cone.toml` + `src/**.scoop`）
/// - case 中必须且仅必须有 1 个 consumer package（其 `Cone.toml` 含 `[dependencies]`）
/// - runner 会先把依赖 packages 打成 `.cone`，再从 `.cone` 读取 `api.scoopir` 并注入：
///   - `resolve::Index`（import/name resolution）
///   - `typecheck::TypeEnv`（TypeRef lowering）
/// - 最后仅对 consumer package 的源文件运行 typecheck pipeline，并按文件头注释断言 pass/fail
fn run_typecheck_cone_archive_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut pkg_dirs = Vec::new();
    for entry in std::fs::read_dir(case_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", case_dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_dir() {
            pkg_dirs.push(entry.path());
        }
    }

    pkg_dirs.sort();
    if pkg_dirs.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(TypecheckConeArchiveCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    let mut pkgs: Vec<scoopc::cone::ConeSourcePackage> = Vec::new();
    for dir in &pkg_dirs {
        pkgs.push(scoopc::cone::load_cone_source_package(dir)?);
    }

    let consumer_indices = pkgs
        .iter()
        .enumerate()
        .filter(|(_idx, pkg)| !pkg.manifest.dependencies.is_empty())
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();

    if consumer_indices.len() != 1 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(TypecheckConeArchiveConsumerNotUnique {
            fixture: rel.display().to_string(),
        }
        .into());
    }
    let consumer_idx = consumer_indices[0];

    // 先把所有 packages 写成 `.cone`，并以 cone name 建索引（供 consumer 依赖查找）。
    let out_dir = crate::commands::temp::make_temp_dir("scoop_fixtures_cone_archive")?;
    let mut cone_paths: std::collections::HashMap<String, PathBuf> =
        std::collections::HashMap::new();

    for (idx, pkg) in pkgs.iter().enumerate() {
        // 说明：consumer package 往往依赖其它 cone，因此在未实现“依赖图打包”（T1107）之前，
        // 这里仅打包“无依赖的 leaf packages”，用于模拟下游通过 `.cone` 消费 public API 的路径。
        if idx == consumer_idx {
            continue;
        }

        let out = out_dir.join(format!(
            "{}-{}.cone",
            pkg.manifest.cone.name, pkg.manifest.cone.version
        ));
        scoopc::cone::write_cone_archive_v0(session, pkg, &out)?;
        cone_paths.insert(pkg.manifest.cone.name.clone(), out);
    }

    let consumer_pkg = &pkgs[consumer_idx];

    // consumer sources：按 cone source package 的规则加载 `src/**/*.scoop`（稳定排序）。
    let mut sources: Vec<scoopc::source::SourceFile> = Vec::new();
    let mut asts: Vec<scoopc::ast::File> = Vec::new();
    for path in &consumer_pkg.sources {
        let source = scoopc::source::SourceFile::load(path)?;
        let ast = parse_file_via_ast_stage(session, &source).map_err(miette::Report::new)?;
        sources.push(source);
        asts.push(ast);
    }
    {
        let source_refs = sources.iter().collect::<Vec<_>>();
        let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )
        .map_err(miette::Report::new)?;
    }

    // 先构建 Index：sysroot + consumer sources（cone=1）。
    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(1),
            source,
            file: ast,
        });
    }

    let mut index =
        scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::new)?;

    // T0629b：cone archive fixtures 使用真实 `Cone.toml`，因此可在这里注入导出入口配置，
    // 让 typecheck 按 entry point 规则强制 `Pure!`。
    index.set_export_entry_points(consumer_pkg.manifest.export_entry_points.clone());

    // type env：sysroot + consumer files（用于当前 cone 的 TypeRef lowering）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::new)?;
    for (source, ast) in sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::new)?;
    }

    // 注入依赖 `.cone` 的 public API（T1105）。
    //
    // cone id 分配约定：
    // - 0：sysroot
    // - 1：consumer
    // - 2+：按依赖在 Cone.toml 中出现顺序分配（稳定）
    let mut pre_specialized_fun_keys: std::collections::HashSet<
        scoopc::cone::pre_specialize::PreSpecializedFunKey,
    > = std::collections::HashSet::new();
    let mut pre_specialized_type_keys: std::collections::HashSet<
        scoopc::cone::pre_specialize::PreSpecializedTypeKey,
    > = std::collections::HashSet::new();
    for (next_dep_cone, dep_name) in (2_u32..).zip(consumer_pkg.manifest.dependencies.keys()) {
        let Some(path) = cone_paths.get(dep_name) else {
            return Err(miette!(
                "consumer 依赖 `{dep_name}` 未在该 case 中找到对应的 package（case: {}）",
                case_dir.display()
            ));
        };

        let dep = scoopc::cone::load_cone_archive_api(path)?;
        let dep_cone = scoopc::resolve::ConeId::new(next_dep_cone);
        scoopc::cone::inject_cone_dependency_public_api(&mut index, &mut env, dep_cone, &dep)?;
        if let Some(file) = dep.pre_specialize.as_ref() {
            pre_specialized_fun_keys.extend(file.fun_key_set());
            pre_specialized_type_keys.extend(file.type_key_set());
        }
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();
    let mut ok = 0usize;

    for (source, ast) in sources.iter().zip(asts.iter_mut()) {
        let exp = FixtureExpectation::from_source(source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = (|| {
            // 先运行不依赖 resolver/index 的 typecheck 预检查。
            scoopc::typecheck::check_file_headers(source, ast).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_struct_decls(source, ast).map_err(box_diagnostic)?;

            // resolver phase：headers + bodies。
            let headers =
                scoopc::resolve::check_file_headers(source, ast, &index).map_err(box_diagnostic)?;
            scoopc::resolve::check_file_bodies(source, ast, &index, &headers)
                .map_err(box_diagnostic)?;

            // typecheck phase。
            scoopc::typecheck::check_file_annotations(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_properties(source, ast, &index, &env)
                .map_err(box_boxed_diagnostic)?;
            scoopc::typecheck::check_file_inheritance(source, ast, &index)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_interfaces(source, ast, &index, &env)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_override_effects(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_boxed_diagnostic)?;

            let want_monomorph_counts =
                exp.expect_monomorph_hit.is_some() || exp.expect_monomorph_miss.is_some();
            let want_type_monomorph_counts =
                exp.expect_type_monomorph_hit.is_some() || exp.expect_type_monomorph_miss.is_some();

            let type_inst_keys_from_type_refs = if want_type_monomorph_counts {
                Some(
                    scoopc::typecheck::check_file_type_refs_with_type_instantiation_keys(
                        source,
                        ast,
                        &index,
                        &headers.imports,
                        &env,
                        &mut types,
                        builtins,
                    )
                    .map_err(box_diagnostic)?,
                )
            } else {
                scoopc::typecheck::check_file_type_refs(
                    source,
                    ast,
                    &index,
                    &headers.imports,
                    &env,
                    &mut types,
                    builtins,
                )
                .map_err(box_diagnostic)?;
                None
            };
            scoopc::typecheck::check_file_where_clauses(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_overload_conflicts(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            let (monomorph_keys, type_inst_keys_from_exprs) = match (
                want_monomorph_counts,
                want_type_monomorph_counts,
            ) {
                (true, true) => {
                    scoopc::typecheck::check_file_exprs_with_monomorph_and_type_instantiation_keys(
                        source,
                        ast,
                        &index,
                        &headers.imports,
                        &env,
                        &mut types,
                        builtins,
                    )
                    .map_err(box_diagnostic)?
                }
                (true, false) => (
                    scoopc::typecheck::check_file_exprs_with_monomorph_keys(
                        source,
                        ast,
                        &index,
                        &headers.imports,
                        &env,
                        &mut types,
                        builtins,
                    )
                    .map_err(box_diagnostic)?,
                    Vec::new(),
                ),
                (false, true) => (
                    Vec::new(),
                    scoopc::typecheck::check_file_exprs_with_type_instantiation_keys(
                        source,
                        ast,
                        &index,
                        &headers.imports,
                        &env,
                        &mut types,
                        builtins,
                    )
                    .map_err(box_diagnostic)?,
                ),
                (false, false) => {
                    scoopc::typecheck::check_file_exprs(
                        source,
                        ast,
                        &index,
                        &headers.imports,
                        &env,
                        &mut types,
                        builtins,
                    )
                    .map_err(box_diagnostic)?;
                    (Vec::new(), Vec::new())
                }
            };

            if want_monomorph_counts {
                let mut hit = 0usize;
                let mut miss = 0usize;
                for k in monomorph_keys {
                    let key = scoopc::cone::pre_specialize::PreSpecializedFunKey {
                        fqn: k.symbol.fqn,
                        type_args: k
                            .type_args
                            .iter()
                            .copied()
                            .map(|id| types.display(id).to_string())
                            .collect(),
                    };
                    if pre_specialized_fun_keys.contains(&key) {
                        hit += 1;
                    } else {
                        miss += 1;
                    }
                }

                if let Some(expected) = exp.expect_monomorph_hit
                    && hit != expected
                {
                    return Err(box_diagnostic(MonomorphHitMismatch {
                        expected,
                        found: hit,
                    }));
                }
                if let Some(expected) = exp.expect_monomorph_miss
                    && miss != expected
                {
                    return Err(box_diagnostic(MonomorphMissMismatch {
                        expected,
                        found: miss,
                    }));
                }
            }

            if want_type_monomorph_counts {
                let mut used: std::collections::HashSet<
                    scoopc::cone::pre_specialize::PreSpecializedTypeKey,
                > = std::collections::HashSet::new();

                for k in type_inst_keys_from_type_refs.into_iter().flatten() {
                    used.insert(scoopc::cone::pre_specialize::PreSpecializedTypeKey {
                        fqn: k.fqn,
                        type_args: k
                            .type_args
                            .iter()
                            .copied()
                            .map(|id| types.display(id).to_string())
                            .collect(),
                    });
                }
                for k in type_inst_keys_from_exprs {
                    used.insert(scoopc::cone::pre_specialize::PreSpecializedTypeKey {
                        fqn: k.fqn,
                        type_args: k
                            .type_args
                            .iter()
                            .copied()
                            .map(|id| types.display(id).to_string())
                            .collect(),
                    });
                }

                let mut hit = 0usize;
                let mut miss = 0usize;
                for k in used {
                    if pre_specialized_type_keys.contains(&k) {
                        hit += 1;
                    } else {
                        miss += 1;
                    }
                }

                if let Some(expected) = exp.expect_type_monomorph_hit
                    && hit != expected
                {
                    return Err(box_diagnostic(TypeMonomorphHitMismatch {
                        expected,
                        found: hit,
                    }));
                }
                if let Some(expected) = exp.expect_type_monomorph_miss
                    && miss != expected
                {
                    return Err(box_diagnostic(TypeMonomorphMissMismatch {
                        expected,
                        found: miss,
                    }));
                }
            }
            Ok(())
        })();

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(source, &exp, &*e)?;
            }
        }

        ok += 1;
    }

    // 对整个编译单元中出现过的类型做一次 layout/metadata 计算（与 typecheck_multi 对齐）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::new)?;

    Ok(ok)
}

/// 运行一个 `tests/fixtures/typecheck_multi/<case>/` 的多文件编译单元。
///
/// 规则（当前阶段）：
/// - case 目录下必须有 2+ 个 `.scoop` 文件
/// - 先把 case 内所有文件 + sysroot 一起构建 `Index`
/// - 构建 type env：sysroot + case 全部文件（用于跨文件的 TypeRef lowering / arity 检查）
/// - 再对 case 内每个文件分别运行 typecheck pipeline，并按各自文件头注释断言 pass/fail
fn run_typecheck_multi_case(
    session: &scoopc::session::Session,
    fixtures_root: &Path,
    case_dir: &Path,
) -> Result<usize> {
    let mut paths = Vec::new();
    collect_scoop_files(case_dir, &mut paths, &[])?;
    paths.sort();

    if paths.len() < 2 {
        let rel = case_dir.strip_prefix(fixtures_root).unwrap_or(case_dir);
        return Err(TypecheckMultiCaseTooSmall {
            fixture: rel.display().to_string(),
        }
        .into());
    }

    let mut sources = Vec::with_capacity(paths.len());
    let mut asts = Vec::with_capacity(paths.len());
    for path in &paths {
        let source = scoopc::source::SourceFile::load(path)?;
        let ast = parse_file_via_ast_stage(session, &source).map_err(miette::Report::new)?;
        sources.push(source);
        asts.push(ast);
    }
    {
        let source_refs = sources.iter().collect::<Vec<_>>();
        let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )
        .map_err(miette::Report::new)?;
    }

    // 先构建单一 Index（sysroot + case）。
    let mut pairs: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    for (s, a) in sources.iter().zip(asts.iter()) {
        pairs.push((s, a));
    }
    let index = scoopc::resolve::Index::build(&pairs).map_err(miette::Report::new)?;

    // type env：sysroot + case 全部文件（用于跨文件 TypeRef lowering）。
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::new)?;
    for (source, ast) in sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::new)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    for (source, ast) in sources.iter().zip(asts.iter_mut()) {
        let exp = FixtureExpectation::from_source(source.text());

        let result: std::result::Result<(), Box<dyn miette::Diagnostic>> = (|| {
            // 先运行不依赖 resolver/index 的 typecheck 预检查。
            scoopc::typecheck::check_file_headers(source, ast).map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_struct_decls(source, ast).map_err(box_diagnostic)?;

            // resolver phase：headers + bodies。
            let headers =
                scoopc::resolve::check_file_headers(source, ast, &index).map_err(box_diagnostic)?;
            scoopc::resolve::check_file_bodies(source, ast, &index, &headers)
                .map_err(box_diagnostic)?;

            // typecheck phase。
            scoopc::typecheck::check_file_annotations(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_properties(source, ast, &index, &env)
                .map_err(box_boxed_diagnostic)?;
            scoopc::typecheck::check_file_inheritance(source, ast, &index)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_interfaces(source, ast, &index, &env)
                .map_err(box_diagnostic)?;
            scoopc::typecheck::check_file_override_effects(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_boxed_diagnostic)?;
            scoopc::typecheck::check_file_type_refs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_where_clauses(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_overload_conflicts(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            scoopc::typecheck::check_file_exprs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(box_diagnostic)?;

            Ok(())
        })();

        match (exp.expect, result) {
            (Expect::Pass, Ok(())) => {}
            (Expect::Pass, Err(e)) => return Err(miette!("期望通过，但执行失败：{e}")),
            (Expect::Fail, Ok(())) => return Err(miette!("期望失败，但执行成功")),
            (Expect::Fail, Err(e)) => {
                assert_diagnostic_matches(source, &exp, &*e)?;
            }
        }
    }

    // T0449：对整个编译单元中出现过的类型做一次 layout/metadata 计算。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::new)?;

    Ok(paths.len())
}

fn box_diagnostic<E>(e: E) -> Box<dyn miette::Diagnostic>
where
    E: miette::Diagnostic + 'static,
{
    Box::new(e)
}

fn box_boxed_diagnostic<E>(e: Box<E>) -> Box<dyn miette::Diagnostic>
where
    E: miette::Diagnostic + 'static,
{
    e
}

fn box_report(e: miette::Report) -> Box<dyn miette::Diagnostic> {
    e.into()
}

fn assert_diagnostic_matches(
    source: &scoopc::source::SourceFile,
    exp: &FixtureExpectation<'_>,
    diag: &dyn miette::Diagnostic,
) -> Result<()> {
    if let Some(expected_code) = exp.error_code {
        let actual_code = diag.code().map(|c| c.to_string());
        if actual_code.as_deref() != Some(expected_code) {
            return Err(miette!(
                "错误码不匹配：期望 {expected_code:?}，实际为：{actual_code:?}"
            ));
        }
    }

    if let Some((line, col)) = exp.error_at {
        let (actual_line, actual_col) = primary_label_line_col(source, diag)?;
        if (actual_line, actual_col) != (line, col) {
            return Err(miette!(
                "错误位置不匹配：期望 {line}:{col}，实际为：{actual_line}:{actual_col}"
            ));
        }
    }

    if let Some(needle) = exp.error_contains {
        let msg = diag.to_string();
        if !msg.contains(needle) {
            return Err(miette!(
                "错误信息不匹配：期望包含 {needle:?}，实际为：{msg}"
            ));
        }
    }

    Ok(())
}

fn primary_label_line_col(
    source: &scoopc::source::SourceFile,
    diag: &dyn miette::Diagnostic,
) -> Result<(usize, usize)> {
    let mut first: Option<(usize, usize)> = None;
    let mut primary: Option<(usize, usize)> = None;

    if let Some(labels) = diag.labels() {
        for l in labels {
            first.get_or_insert((l.offset(), l.len()));
            if l.primary() {
                primary = Some((l.offset(), l.len()));
                break;
            }
        }
    }

    let (offset, len) = primary
        .or(first)
        .ok_or_else(|| miette!("诊断未提供 labels/span，无法断言错误位置"))?;

    if let Some(source_code) = diag.source_code() {
        let span: miette::SourceSpan = (offset, len).into();
        if let Ok(contents) = source_code.read_span(&span, 0, 0) {
            return Ok((contents.line() + 1, contents.column() + 1));
        }
    }

    source.offset_to_line_col(offset)
}

fn collect_scoop_files_inner(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    skip_dirs: &[&Path],
) -> Result<()> {
    if skip_dirs.iter().any(|skip| dir.starts_with(skip)) {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;

        if ty.is_dir() {
            collect_scoop_files_inner(&path, out, skip_dirs)?;
            continue;
        }

        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>, skip_dirs: &[&Path]) -> Result<()> {
    collect_scoop_files_inner(dir, out, skip_dirs)
}

fn collect_resolve_multi_cases(resolve_multi_root: &Path) -> Result<Vec<PathBuf>> {
    if !resolve_multi_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(resolve_multi_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", resolve_multi_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    // 稳定排序（便于定位错误）。
    cases.sort();
    Ok(cases)
}

fn collect_resolve_cone_cases(resolve_cone_root: &Path) -> Result<Vec<PathBuf>> {
    if !resolve_cone_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(resolve_cone_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", resolve_cone_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn collect_typecheck_multi_cases(typecheck_multi_root: &Path) -> Result<Vec<PathBuf>> {
    if !typecheck_multi_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(typecheck_multi_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", typecheck_multi_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn collect_typecheck_cone_cases(typecheck_cone_root: &Path) -> Result<Vec<PathBuf>> {
    if !typecheck_cone_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(typecheck_cone_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", typecheck_cone_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn collect_typecheck_cone_archive_cases(
    typecheck_cone_archive_root: &Path,
) -> Result<Vec<PathBuf>> {
    if !typecheck_cone_archive_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(typecheck_cone_archive_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", typecheck_cone_archive_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn collect_run_pass_cone_cases(run_pass_cone_root: &Path) -> Result<Vec<PathBuf>> {
    if !run_pass_cone_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in std::fs::read_dir(run_pass_cone_root)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", run_pass_cone_root.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if entry.file_type().into_diagnostic()?.is_dir() {
            cases.push(path);
        }
    }

    cases.sort();
    Ok(cases)
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// 返回 fixture 的一级目录名（即 phase 目录）。
///
/// 例如：
/// - `parse/hello.scoop` → Some("parse")
/// - `resolve/foo/bar.scoop` → Some("resolve")
/// - `hello.scoop` → None（直接放在根目录下，按 parse 处理以保持兼容）
fn phase_dir(rel: &Path) -> Option<&std::ffi::OsStr> {
    let mut comps = rel.components();
    let first = comps.next();
    let second = comps.next();
    match (first, second) {
        (Some(Component::Normal(name)), Some(_)) => Some(name),
        _ => None,
    }
}

fn phase_name<'a>(fixtures_root: &'a Path, rel: &'a Path) -> Option<&'a std::ffi::OsStr> {
    phase_dir(rel).or_else(|| {
        let mut comps = rel.components();
        match (comps.next(), comps.next()) {
            (Some(Component::Normal(_)), None) => fixtures_root
                .ancestors()
                .filter_map(Path::file_name)
                .find(|name| is_phase_dir_name(name)),
            _ => None,
        }
    })
}

fn is_phase_dir_name(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            "parse"
                | "spec_doctest"
                | "build"
                | "resolve"
                | "typecheck"
                | "unsafe_nogc"
                | "infer"
                | "comptime"
                | "codegen"
                | "run-pass"
                | "runtime_gc"
                | "hir"
                | "mir"
                | "mir_refactor"
                | "effect_facts"
                | "effect_lowered"
                | "scoopir"
        )
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        RunPassEnvOverrides, build_run_pass_cone_run_command, is_run_pass_cone_case_root,
        phase_name, run_all, run_pass_cone_root, strip_deleted_exe_suffix,
    };

    fn command_args(cmd: &std::process::Command) -> Vec<String> {
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn phase_name_uses_relative_phase_dir_when_present() {
        let fixtures_root = Path::new("tests/fixtures");
        let rel = Path::new("unsafe_nogc/extern_call_requires_unsafe_is_error.scoop");

        assert_eq!(
            phase_name(fixtures_root, rel),
            Some(OsStr::new("unsafe_nogc"))
        );
    }

    #[test]
    fn phase_name_falls_back_to_root_phase_dir_for_single_file_subset() {
        let fixtures_root = Path::new("tests/fixtures/unsafe_nogc");
        let rel = Path::new("extern_call_requires_unsafe_is_error.scoop");

        assert_eq!(
            phase_name(fixtures_root, rel),
            Some(OsStr::new("unsafe_nogc"))
        );
    }

    #[test]
    fn phase_name_falls_back_to_root_phase_dir_for_effect_facts_single_file_subset() {
        let fixtures_root = Path::new("tests/fixtures/effect_facts");
        let rel = Path::new("dispatch_and_resume_call.scoop");

        assert_eq!(
            phase_name(fixtures_root, rel),
            Some(OsStr::new("effect_facts"))
        );
    }

    #[test]
    fn phase_name_falls_back_to_root_phase_dir_for_effect_lowered_single_file_subset() {
        let fixtures_root = Path::new("tests/fixtures/effect_lowered");
        let rel = Path::new("dispatch_and_resume_call.scoop");

        assert_eq!(
            phase_name(fixtures_root, rel),
            Some(OsStr::new("effect_lowered"))
        );
    }

    #[test]
    fn phase_name_falls_back_to_root_phase_dir_for_mir_refactor_single_file_subset() {
        let fixtures_root = Path::new("tests/fixtures/mir_refactor");
        let rel = Path::new("generic_materialization.scoop");

        assert_eq!(
            phase_name(fixtures_root, rel),
            Some(OsStr::new("mir_refactor"))
        );
    }

    #[test]
    fn phase_name_walks_up_to_phase_dir_for_nested_single_file_subset() {
        let fixtures_root = Path::new("tests/fixtures/infer/effects");
        let rel = Path::new("use_site_eff_row_receiver_mismatch_is_error.scoop");

        assert_eq!(phase_name(fixtures_root, rel), Some(OsStr::new("infer")));
    }

    #[test]
    fn phase_name_keeps_root_level_files_parse_by_default() {
        let fixtures_root = Path::new("tests/fixtures");
        let rel = Path::new("hello.scoop");

        assert_eq!(phase_name(fixtures_root, rel), None);
    }

    #[test]
    fn strip_deleted_exe_suffix_recovers_real_path() {
        let deleted = Path::new("/tmp/target/debug/scoop (deleted)");
        let stripped = strip_deleted_exe_suffix(deleted).unwrap();
        assert_eq!(stripped, Path::new("/tmp/target/debug/scoop"));
    }

    #[test]
    fn run_pass_cone_root_uses_subset_root_directly() {
        let subset_root = Path::new("tests/fixtures/run_pass_cone");
        assert_eq!(run_pass_cone_root(subset_root), subset_root);
    }

    #[test]
    fn is_run_pass_cone_case_root_detects_cone_case_dir() {
        let dir = tempdir().unwrap();
        let case_dir = dir.path().join("run_pass_cone").join("cone_case");
        let src_dir = case_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(case_dir.join("Cone.toml"), "[cone]\nname = \"cone-case\"\n").unwrap();
        fs::write(
            src_dir.join("main.scoop"),
            "// EXPECT: pass\npackage fixtures.run_pass_cone.case\npublic fun main() / Pure! {}",
        )
        .unwrap();

        assert!(is_run_pass_cone_case_root(&case_dir));
    }

    #[test]
    fn run_pass_cone_single_pipeline_omits_selector() {
        let exp = super::expectations::FixtureExpectation::from_source(
            "// EXPECT: pass\n// ARGS: --release\n",
        );
        let cmd = build_run_pass_cone_run_command(
            Path::new("scoop"),
            Path::new("tests/fixtures/run_pass_cone/demo"),
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
            &exp,
        );

        let args = command_args(&cmd);
        assert_eq!(args.first().map(String::as_str), Some("run"));
        assert!(!args.iter().any(|arg| arg == "--effect-pipeline"));
        assert!(args.iter().any(|arg| arg == "--release"));
    }

    #[test]
    fn run_all_treats_typecheck_multi_case_root_as_single_case() {
        let dir = tempdir().unwrap();
        let case_dir = dir
            .path()
            .join("typecheck_multi")
            .join("cross_file_box_case");
        fs::create_dir_all(&case_dir).unwrap();

        fs::write(
            case_dir.join("defs.scoop"),
            r#"
// EXPECT: pass

package fixtures.typecheck_multi.cross_file_box_case

struct Box<T>(val value: T) {
    val bodyCopy: T = value
    val readBack: T
        get() = this.bodyCopy
}
"#,
        )
        .unwrap();
        fs::write(
            case_dir.join("use.scoop"),
            r#"// EXPECT: fail
// EXPECT-ERROR-CODE: scoop::typecheck::initializer_type_mismatch

package fixtures.typecheck_multi.cross_file_box_case

val bad: Int = Box("oops").bodyCopy
"#,
        )
        .unwrap();

        let ok = run_all(
            &case_dir,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 2);
    }

    #[test]
    fn run_all_treats_resolve_multi_case_root_as_single_case() {
        let case_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/resolve_multi/cross_file_type_ref");

        let ok = run_all(
            &case_dir,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 2);
    }

    #[test]
    fn run_all_treats_resolve_cone_case_root_as_single_case() {
        let case_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/resolve_cone/cross_cone_visibility");

        let ok = run_all(
            &case_dir,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 4);
    }

    #[test]
    fn run_all_treats_typecheck_cone_case_root_as_single_case() {
        let case_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/typecheck_cone/cross_cone_extension_imports");

        let ok = run_all(
            &case_dir,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 3);
    }

    #[test]
    fn run_all_treats_typecheck_cone_archive_case_root_as_single_case() {
        let case_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/typecheck_cone_archive/deps_api_injection");

        let ok = run_all(
            &case_dir,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 1);
    }

    #[test]
    fn run_all_accepts_single_fixture_file() {
        let dir = tempdir().unwrap();
        let fixture_dir = dir.path().join("parse");
        fs::create_dir_all(&fixture_dir).unwrap();
        let fixture = fixture_dir.join("single_file_subset.scoop");
        fs::write(
            &fixture,
            "// EXPECT: pass\npackage fixtures.parse.single_file_subset\npublic fun main() / Pure! {}\n",
        )
        .unwrap();

        let ok = run_all(
            &fixture,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 1);
    }

    #[test]
    fn infer_fixtures_use_refactor_typed_hir_diagnostics() {
        let dir = tempdir().unwrap();
        let fixture_dir = dir.path().join("infer").join("effects");
        fs::create_dir_all(&fixture_dir).unwrap();

        let workspace_fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let fixture = fixture_dir.join("use_site_eff_row_receiver_mismatch_is_error.scoop");
        fs::copy(
            workspace_fixtures
                .join("infer/effects/use_site_eff_row_receiver_mismatch_is_error.scoop"),
            &fixture,
        )
        .unwrap();

        let ok = run_all(
            &fixture,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();

        assert_eq!(ok, 1);
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_fixtures_propagate_single_pipeline_session_options_to_build_command() {
        let dir = tempdir().unwrap();
        let fixture_dir = dir.path().join("build");
        fs::create_dir_all(&fixture_dir).unwrap();
        let fixture = fixture_dir.join("refactor_abi_visibility.scoop");
        fs::write(
            &fixture,
            r#"// EXPECT: pass
// ARGS: --emit-llvm
// BUILD-LLVM-CONTAINS: __scoop_priv0__refactor_dynamic_invoke__h

package fixtures.build_fixture_visibility

effect Ping {
    fun hit(): Unit
}

fun hiddenWorker(): Unit / Ping {
    Ping.hit()
}

fun main(): Int {
    return 0
}
"#,
        )
        .unwrap();

        let ok = run_all(
            &fixture_dir,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 1);
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn run_all_recreates_session_between_independent_fixtures() {
        let repro_root = std::env::var_os("SCOOP_FIXTURE_REPRO_DIR").map(PathBuf::from);
        let temp_dir = if repro_root.is_none() {
            Some(tempdir().unwrap())
        } else {
            None
        };
        let root = if let Some(path) = repro_root {
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            path
        } else {
            temp_dir.as_ref().unwrap().path().to_path_buf()
        };
        let build_dir = root.join("build");
        let mir_refactor_dir = root.join("mir_refactor");
        fs::create_dir_all(&build_dir).unwrap();
        fs::create_dir_all(&mir_refactor_dir).unwrap();

        let workspace_fixtures =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let build_fixture = build_dir.join("effect_no_perform_no_handler_symbols_basic.scoop");
        let mir_fixture = mir_refactor_dir.join("aggregate_transport.scoop");

        fs::copy(
            workspace_fixtures.join("build/effect_no_perform_no_handler_symbols_basic.scoop"),
            &build_fixture,
        )
        .unwrap();
        fs::copy(
            workspace_fixtures.join("mir_refactor/aggregate_transport.scoop"),
            &mir_fixture,
        )
        .unwrap();
        fs::copy(
            workspace_fixtures.join("mir_refactor/aggregate_transport.mir"),
            mir_fixture.with_extension("mir"),
        )
        .unwrap();

        let ok = run_all(
            &root,
            None,
            scoopc::session::SessionOptions::new(),
            &RunPassEnvOverrides::new(),
        )
        .unwrap();
        assert_eq!(ok, 2);
    }
}
