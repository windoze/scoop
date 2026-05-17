//! `scoop build` 子命令。
//!
//! T0805：实现“前端检查 + 输出路径准备”。
//!
//! T0806：在启用 `scoop` 的 LLVM 后端时（默认开启；可用 `--no-default-features` 关闭），额外执行：
//! - 生成最小 object（当前阶段仍是固定 `main → ret 0`）；
//! - 调用 clang 链接 object + 早期 C runtime，产出可执行文件。

mod deps;
mod incremental;
pub(crate) mod layout;

use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use scoopc::opt::OptLevel;
use scoopc::session::SessionOptions;
use thiserror::Error;

type BuildInput = scoopc::frontend::ProjectInput;
type BuildContext = scoopc::frontend::ProjectContext;
type FrontendOutput = scoopc::frontend::FrontendOutput;
pub(crate) type EntryPackageMissingMain = scoopc::frontend::EntryPackageMissingMain;
pub(crate) type EntryPackageMainNotInConsumerCone =
    scoopc::frontend::EntryPackageMainNotInConsumerCone;

fn emit_frontend_warnings(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    warnings: &[scoopc::warnings::CompileWarning],
) {
    for warning in warnings {
        let source = find_warning_source(session, front, warning.file());
        let (line, col) = source
            .and_then(|source| source.offset_to_line_col(warning.span().start).ok())
            .unwrap_or((1, 1));
        eprintln!(
            "{}:{line}:{col}: {}",
            warning.file().display(),
            warning.render()
        );
    }
}

fn find_warning_source<'a>(
    session: &'a scoopc::session::Session,
    front: &'a FrontendOutput,
    path: &Path,
) -> Option<&'a scoopc::source::SourceFile> {
    front
        .input()
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .or_else(|| {
            session
                .sysroot()
                .files
                .iter()
                .find(|file| file.source.path() == path)
                .map(|file| &file.source)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEmit {
    /// 产出可执行文件（默认）。
    Executable,
    /// 产出 LLVM IR（`.ll`）。
    LlvmIr,
    /// 产出 object 文件（`.o` / `.obj`）。
    Obj,
    /// 产出汇编（`.s` / `.asm`）。
    Asm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildProfile::Debug => "debug",
            BuildProfile::Release => "release",
        }
    }

    pub fn from_debug_release_flags(debug: bool, release: bool) -> Self {
        // 冲突由 clap 处理；这里保持行为稳定且易于复用（T1122/T1123）。
        let _ = debug;
        if release {
            BuildProfile::Release
        } else {
            BuildProfile::Debug
        }
    }
}

pub(crate) fn default_opt_level_for_profile(profile: BuildProfile) -> OptLevel {
    match profile {
        BuildProfile::Debug => OptLevel::O0,
        BuildProfile::Release => OptLevel::O2,
    }
}

pub(crate) fn resolve_opt_level(
    cli_opt_level: Option<OptLevel>,
    manifest_opt_level: Option<OptLevel>,
    profile: BuildProfile,
) -> OptLevel {
    cli_opt_level
        .or(manifest_opt_level)
        .unwrap_or_else(|| default_opt_level_for_profile(profile))
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub emit: BuildEmit,
    /// （cone 包模式）入口 package（覆盖 `Cone.toml` 的 `native-build.entry-package`）。
    pub entry_package: Option<String>,
    /// cone build profile（影响默认产物目录布局：`build/<profile>/...`）。
    pub profile: BuildProfile,
    /// 优化等级（CLI 覆盖；未指定时走 manifest/profile 默认策略）。
    pub opt_level: Option<OptLevel>,
    /// 是否启用粗粒度增量构建（T1124）。
    pub incremental: bool,
    /// 构造编译 session 时使用的统一配置。
    pub session_options: SessionOptions,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            emit: BuildEmit::Executable,
            entry_package: None,
            profile: BuildProfile::Debug,
            opt_level: None,
            incremental: true,
            session_options: SessionOptions::new(),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("`--entry-package` 仅支持 cone 包目录输入：{input}")]
#[diagnostic(code(scoop::driver::entry_package_only_for_cone))]
pub(crate) struct EntryPackageOnlyForCone {
    input: String,
}

/// 执行 `scoop build <input> [-o <output>]`。
///
/// 当前阶段验收点：
/// - 输入可通过 parse/resolve/typecheck 时返回 `Ok(())`；
/// - 当启用 LLVM 后端时（默认已启用；若你用了 `--no-default-features` 则需要显式开启）：
///   - 默认产出可执行文件；
///   - 若指定 `--emit-llvm/--emit-obj/--emit-asm`，则改为产出对应单文件产物。
pub fn run(input: PathBuf, output: Option<PathBuf>, options: BuildOptions) -> Result<()> {
    let BuildOptions {
        emit,
        entry_package,
        profile,
        opt_level: opt_level_override,
        incremental,
        session_options,
    } = options;
    let session_options = session_options.with_env_fallback();
    let incremental = incremental && session_options.sysroot_overlay().is_none();

    let entry_package_for_fingerprint = entry_package.clone();

    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let context = load_build_context_with_options(&input, entry_package, &session_options)?;
    let opt_level = resolve_opt_level(
        opt_level_override,
        context.input().cone_manifest().native_build.opt_level,
        profile,
    );
    let output = output
        .unwrap_or_else(|| default_output_path_for_input_and_emit(context.input(), emit, profile));
    ensure_output_parent_dir(&output)?;

    if output.exists() && output.is_dir() {
        return Err(miette::miette!("输出路径是目录：{}", output.display()));
    }

    // T1124：粗粒度增量构建（仅对 cone 项目 + 可执行产物生效）。
    //
    // 重要：为避免污染 run-pass fixtures 的 stdout，这里统一把“cache hit”信息输出到 stderr。
    let mut computed_fingerprint: Option<incremental::BuildFingerprint> = None;
    let incremental_ctx = if !incremental
        || !cfg!(feature = "llvm")
        || emit != BuildEmit::Executable
        || !context.input().is_explicit_cone()
    {
        None
    } else {
        let root = context.input().cone_root().to_path_buf();
        let expected_out = layout::cone_exe_path(
            &root,
            None,
            profile.as_str(),
            &context.input().cone_manifest().cone.name,
        );
        if output != expected_out {
            None
        } else {
            let build_json = layout::cone_build_json_path(&root, None, profile.as_str());
            Some((root, build_json))
        }
    };

    if let Some((cone_root, build_json)) = incremental_ctx.clone()
        && output.is_file()
        && let Some(cached) = incremental::read_cached_fingerprint(&build_json)?
    {
        let fp = incremental::compute_cone_build_fingerprint(
            &cone_root,
            profile.as_str(),
            entry_package_for_fingerprint.as_deref(),
            opt_level,
        )?;
        if fp.fingerprint == cached {
            eprintln!("skipping build (cache hit)");
            return Ok(());
        }
        computed_fingerprint = Some(fp);
    }

    let session = scoopc::session::Session::with_options(session_options.clone())?;

    let warning_capture = scoopc::warnings::begin_capture();
    let front = run_frontend(&session, context)?;
    let warnings = warning_capture.finish();
    emit_frontend_warnings(&session, &front, &warnings);
    // 非 llvm 构建下，codegen 分支会被编译掉；这里显式访问一次 main 以避免 dead_code 警告，
    // 同时也作为“加载逻辑能稳定定位入口”的最小一致性校验。
    let _ = front.main_source();

    match emit {
        BuildEmit::Executable => {
            // 只有在启用 LLVM 后端时才会真正生成可执行文件；默认构建仍保持“前端检查”可用。
            #[cfg(feature = "llvm")]
            run_codegen_and_link(&session, &front, &output, profile, opt_level)?;
        }
        BuildEmit::LlvmIr => {
            #[cfg(feature = "llvm")]
            {
                let _extern_libs = emit_llvm_artifact_for_build(
                    &session,
                    &front,
                    &output,
                    opt_level,
                    scoopc::pipeline::LlvmArtifactKind::LlvmIr,
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-llvm` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- build --emit-llvm <file> -o <out.ll>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
                ));
            }
        }
        BuildEmit::Obj => {
            #[cfg(feature = "llvm")]
            {
                let _extern_libs = emit_llvm_artifact_for_build(
                    &session,
                    &front,
                    &output,
                    opt_level,
                    scoopc::pipeline::LlvmArtifactKind::Object,
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-obj` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- build --emit-obj <file> -o <out.o>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
                ));
            }
        }
        BuildEmit::Asm => {
            #[cfg(feature = "llvm")]
            {
                let _extern_libs = emit_llvm_artifact_for_build(
                    &session,
                    &front,
                    &output,
                    opt_level,
                    scoopc::pipeline::LlvmArtifactKind::Asm,
                )?;
            }
            #[cfg(not(feature = "llvm"))]
            {
                let _ = &session;
                let _ = &output;
                return Err(miette::miette!(
                    "`--emit-asm` 需要启用 LLVM 后端：请使用 `cargo run -p scoop -- build --emit-asm <file> -o <out.s>`（若你用了 `--no-default-features`，去掉它或加上 `--features llvm`）"
                ));
            }
        }
    }

    if let Some((cone_root, build_json)) = incremental_ctx {
        // 仅当最终产物存在时才更新 build.json，避免“只有前端检查”时产生误导性的缓存条目。
        if output.is_file() {
            let fp = match computed_fingerprint {
                Some(fp) => fp,
                None => incremental::compute_cone_build_fingerprint(
                    &cone_root,
                    profile.as_str(),
                    entry_package_for_fingerprint.as_deref(),
                    opt_level,
                )?,
            };
            incremental::write_build_json(
                &build_json,
                profile.as_str(),
                entry_package_for_fingerprint.as_deref(),
                opt_level,
                &fp,
            )?;
        }
    }

    Ok(())
}

fn load_build_input_with_options(
    input: &Path,
    entry_package_override: Option<String>,
    session_options: &SessionOptions,
) -> Result<BuildInput> {
    if input.is_file() && entry_package_override.is_some() {
        return Err(EntryPackageOnlyForCone {
            input: input.display().to_string(),
        }
        .into());
    }
    scoopc::frontend::load_project_input_from_path(input, entry_package_override, session_options)
}

#[cfg_attr(not(test), allow(dead_code))]
fn load_build_context(
    input: &Path,
    entry_package_override: Option<String>,
) -> Result<BuildContext> {
    load_build_context_with_options(input, entry_package_override, &SessionOptions::new())
}

fn load_build_context_with_options(
    input: &Path,
    entry_package_override: Option<String>,
    session_options: &SessionOptions,
) -> Result<BuildContext> {
    let input = load_build_input_with_options(input, entry_package_override, session_options)?;
    let deps = if input.is_explicit_cone() {
        deps::load_dependency_graph(input.cone_manifest(), input.cone_root())?
    } else {
        Vec::new()
    };
    Ok(BuildContext::new(input, deps))
}

fn default_output_path_for_input_and_emit(
    input: &BuildInput,
    emit: BuildEmit,
    profile: BuildProfile,
) -> PathBuf {
    if emit == BuildEmit::Executable && input.is_explicit_cone() {
        return layout::cone_exe_path(
            input.cone_root(),
            None,
            profile.as_str(),
            &input.cone_manifest().cone.name,
        );
    }
    default_output_path_for_emit(emit)
}

fn default_stdlib_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("无法读取目录：{}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        let ty = entry.file_type().into_diagnostic()?;
        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }
        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

fn run_frontend(
    session: &scoopc::session::Session,
    context: BuildContext,
) -> Result<FrontendOutput> {
    scoopc::frontend::run_project_frontend(session, context)
}

fn ensure_output_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)
        .into_diagnostic()
        .wrap_err("无法创建输出目录")?;
    Ok(())
}

fn default_output_path_for_emit(emit: BuildEmit) -> PathBuf {
    match emit {
        BuildEmit::Executable => {
            let ext = std::env::consts::EXE_EXTENSION;
            if ext.is_empty() {
                PathBuf::from("a.out")
            } else {
                PathBuf::from(format!("a.{ext}"))
            }
        }
        BuildEmit::LlvmIr => PathBuf::from("a.ll"),
        BuildEmit::Obj => {
            if cfg!(windows) {
                PathBuf::from("a.obj")
            } else {
                PathBuf::from("a.o")
            }
        }
        BuildEmit::Asm => {
            if cfg!(windows) {
                PathBuf::from("a.asm")
            } else {
                PathBuf::from("a.s")
            }
        }
    }
}

#[cfg(feature = "llvm")]
fn emit_llvm_artifact_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    output: &Path,
    opt_level: OptLevel,
    artifact: scoopc::pipeline::LlvmArtifactKind,
) -> Result<Vec<String>> {
    // P6-T05 handoff：`build --emit-*`、`run`（通过 executable build）和 build fixtures
    // 都必须经由同一 LLVM stage helper，避免为某个产物种类保留测试专用语义入口。
    scoopc::pipeline::emit_project_llvm_artifact_to_file(
        session, front, output, opt_level, artifact,
    )
    .map_err(Into::into)
}

#[cfg(feature = "llvm")]
fn run_codegen_and_link(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    output: &Path,
    profile: BuildProfile,
    opt_level: OptLevel,
) -> Result<()> {
    // T1121：cone 包的 build 产物应落在项目内 `build/<profile>/...`，而不是 `/tmp`。
    // - cone 包：写入 `build/<profile>/obj/`（由 `scoop build --debug/--release` 控制）。
    // - 单文件模式：仍使用临时目录（保持行为不变）。
    let is_cone = front.input().is_explicit_cone();

    let (work_dir, keep_work_dir) = if is_cone {
        let root = front.input().cone_root();
        let dir = layout::cone_obj_dir(root, None, profile.as_str());
        std::fs::create_dir_all(&dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法创建 build obj 目录：{}", dir.display()))?;
        (dir, true)
    } else {
        (super::temp::make_temp_dir("scoop_build")?, false)
    };

    let obj = work_dir.join(layout::obj_file_name("main"));

    let extern_libs = emit_llvm_artifact_for_build(
        session,
        front,
        &obj,
        opt_level,
        scoopc::pipeline::LlvmArtifactKind::Object,
    )?;

    // T1115：cone native build 的 `c-sources/c-flags`：
    // - 额外把用户声明的 C 源文件编译成 `.o`；
    // - `c-flags` 仅作用于这些 user sources（不影响 runtime/c 的编译选项）。
    let mut extra_objs: Vec<PathBuf> = Vec::new();
    let mut use_cxx_linker_driver = false;
    if front.input().is_explicit_cone() {
        let root = front.input().cone_root();
        let manifest = front.input().cone_manifest();
        if !manifest.native_build.c_sources.is_empty() {
            extra_objs.reserve(manifest.native_build.c_sources.len());
            for (idx, rel) in manifest.native_build.c_sources.iter().enumerate() {
                let src = root.join(rel);
                let out_obj = work_dir.join(layout::obj_file_name(&format!("cone_c_{idx}")));
                crate::toolchain::compile_c_source_to_obj(
                    root,
                    &src,
                    &out_obj,
                    &manifest.native_build.c_flags,
                )?;
                extra_objs.push(out_obj);
            }
        }

        // T1116：cone native build 的 `cxx-sources/cxx-flags`：
        // - 额外把用户声明的 C++ 源文件编译成 `.o`；
        // - `cxx-flags` 仅作用于这些 user sources；
        // - 当存在 C++ 源码时，最终链接默认使用 C++ driver（见下方 link options）。
        if !manifest.native_build.cxx_sources.is_empty() {
            use_cxx_linker_driver = true;
            extra_objs.reserve(manifest.native_build.cxx_sources.len());
            for (idx, rel) in manifest.native_build.cxx_sources.iter().enumerate() {
                let src = root.join(rel);
                let out_obj = work_dir.join(layout::obj_file_name(&format!("cone_cxx_{idx}")));
                crate::toolchain::compile_cxx_source_to_obj(
                    root,
                    &src,
                    &out_obj,
                    &manifest.native_build.cxx_flags,
                )?;
                extra_objs.push(out_obj);
            }
        }
    }

    // T1114：把 Cone.toml 的 `native-build.linker/link-flags` 透传到最终链接命令。
    let mut linker = front
        .input()
        .is_explicit_cone()
        .then(|| front.input().cone_manifest().native_build.linker.as_deref())
        .flatten();
    if use_cxx_linker_driver && linker.is_none() {
        // 默认策略（v0）：仅在用户启用 `cxx-sources` 时才切换到 C++ driver，
        // 以避免在纯 C/纯 Scoop 场景引入额外工具链依赖。
        linker = Some("clang++");
    }
    let options = crate::toolchain::LinkOptions {
        linker,
        link_flags: if front.input().is_explicit_cone() {
            front
                .input()
                .cone_manifest()
                .native_build
                .link_flags
                .as_slice()
        } else {
            &[]
        },
    };
    let mut objs: Vec<PathBuf> = Vec::with_capacity(1 + extra_objs.len());
    objs.push(obj.clone());
    objs.extend(extra_objs);

    if is_cone {
        let runtime_objs = crate::toolchain::compile_runtime_c_sources_to_obj_dir(&work_dir)?;
        objs.extend(runtime_objs);
        crate::toolchain::link_objs(&objs, output, &extern_libs, options)?;
    } else {
        crate::toolchain::link_objs_with_runtime(&objs, output, &extern_libs, options)?;
    }

    if !keep_work_dir {
        let _ = std::fs::remove_dir_all(&work_dir);
    }
    Ok(())
}

#[cfg(all(feature = "llvm", test))]
fn lower_hir_for_build_with_request_root_mode(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
    request_root_mode: scoopc::frontend::MirRequestRootMode,
) -> Result<scoopc::hir::LoweredHir> {
    scoopc::frontend::lower_hir_for_codegen_with_request_root_mode(
        session,
        front,
        opt_level,
        request_root_mode,
    )
}

#[cfg(all(feature = "llvm", test))]
fn lower_main_hir_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
) -> Result<scoopc::hir::LoweredHir> {
    lower_hir_for_build_with_request_root_mode(
        session,
        front,
        opt_level,
        scoopc::frontend::MirRequestRootMode::EntryMain,
    )
}

#[cfg(all(feature = "llvm", test))]
fn abi_visibility_lowered_hir_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
) -> Result<Option<scoopc::hir::LoweredHir>> {
    // 这条附加 handoff 只负责把 request-source 范围内的 callable ABI shell 暴露给
    // LLVM stage；真正的 reachable body lowering 仍由 entry-main rooted build lowering 决定。
    lower_hir_for_build_with_request_root_mode(
        session,
        front,
        opt_level,
        scoopc::frontend::MirRequestRootMode::RequestSources,
    )
    .map(Some)
}

#[cfg(all(feature = "llvm", test))]
fn build_codegen_source_map(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
) -> (scoopc::source::SourceMap, scoopc::source::SourceId) {
    scoopc::frontend::build_source_map(session, front.input())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use scoopc::opt::OptLevel;
    #[cfg(feature = "llvm")]
    use scoopc::pipeline::LlvmArtifactKind;
    use tempfile::tempdir;

    #[cfg(feature = "llvm")]
    fn session() -> scoopc::session::Session {
        use scoopc::session::SessionOptions;

        scoopc::session::Session::with_options(SessionOptions::new()).unwrap()
    }

    #[cfg(feature = "llvm")]
    fn write_reachable_legacy_effect_fixture(input: &std::path::Path) {
        std::fs::write(
            input,
            r#"
package fixtures.reachable_legacy

effect Ping {
    fun hit(): Unit
}

fun hiddenWorker(): Unit / Ping {
    Ping.hit()
}

fun main(): Int {
    return handle {
        hiddenWorker()
        0
    } with {
        Ping.hit(), _k -> 0
    }
}
"#,
        )
        .unwrap();
    }

    #[test]
    fn resolve_opt_level_prefers_cli_over_manifest() {
        let out = super::resolve_opt_level(
            Some(OptLevel::O2),
            Some(OptLevel::O0),
            super::BuildProfile::Debug,
        );
        assert_eq!(out, OptLevel::O2);
    }

    #[test]
    fn resolve_opt_level_uses_manifest_when_cli_missing() {
        let out = super::resolve_opt_level(None, Some(OptLevel::Oz), super::BuildProfile::Release);
        assert_eq!(out, OptLevel::Oz);
    }

    #[test]
    fn resolve_opt_level_defaults_by_profile() {
        assert_eq!(
            super::resolve_opt_level(None, None, super::BuildProfile::Debug),
            OptLevel::O0
        );
        assert_eq!(
            super::resolve_opt_level(None, None, super::BuildProfile::Release),
            OptLevel::O2
        );
    }

    #[test]
    fn build_frontend_ok_and_creates_parent_dir() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("nested").join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(input, Some(out), super::BuildOptions::default()).unwrap();
        assert!(dir.path().join("nested").is_dir());
    }

    #[test]
    fn build_accepts_cone_package_dir_and_finds_main() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-pkg"
version = "0.0.0"
"#,
        )
        .unwrap();

        std::fs::write(src.join("main.scoop"), "fun main() {}\n").unwrap();
        std::fs::write(src.join("util.scoop"), "fun helper() {}\n").unwrap();

        let out = dir.path().join("out").join("a");
        super::run(pkg, Some(out), super::BuildOptions::default()).unwrap();
    }

    #[test]
    fn build_cone_package_can_load_cone_deps_for_frontend() {
        let dir = tempdir().unwrap();

        // 1) 准备一个被依赖的 lib cone（用于打成 `.cone`）。
        let lib = dir.path().join("lib");
        let lib_src = lib.join("src");
        std::fs::create_dir_all(&lib_src).unwrap();
        std::fs::write(
            lib.join("Cone.toml"),
            r#"
[cone]
name = "fixture-lib"
version = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            lib_src.join("api.scoop"),
            r#"
package fixtures.t1107.lib

import scoop.core.*

public struct Token(val value: Int)
"#,
        )
        .unwrap();
        // 说明：cone source package 约定必须存在 `src/main.scoop`（即使它只是库）。
        std::fs::write(lib_src.join("main.scoop"), "package fixtures.t1107.lib\n").unwrap();

        // 2) 准备一个 consumer app cone：依赖 `fixture-lib`，并在类型层引用 Token。
        let app = dir.path().join("app");
        let app_src = app.join("src");
        let app_cone = app.join("cone");
        std::fs::create_dir_all(&app_src).unwrap();
        std::fs::create_dir_all(&app_cone).unwrap();
        std::fs::write(
            app.join("Cone.toml"),
            r#"
[cone]
name = "fixture-app"
version = "0.0.0"

[dependencies]
fixture-lib = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            app_src.join("main.scoop"),
            r#"
package fixtures.t1107.app

import scoop.core.*
import fixtures.t1107.lib.*

public fun unused(x: Token): Int / Pure! {
    1
}

public fun main() / Pure! {
    println("ok")
}
"#,
        )
        .unwrap();

        // 3) 把 lib 打成 `.cone` 放到 `app/cone/`，让 build 在默认搜索路径下可找到。
        let session = scoopc::session::Session::new().unwrap();
        let pkg = scoopc::cone::load_cone_source_package(&lib).unwrap();
        let out_cone = app_cone.join("fixture-lib-0.0.0.cone");
        scoopc::cone::write_cone_archive_v0(&session, &pkg, &out_cone).unwrap();

        let out = dir.path().join("out").join("a");
        super::run(app, Some(out), super::BuildOptions::default()).unwrap();
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn build_produces_executable_and_it_runs() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("a");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(input, Some(out.clone()), super::BuildOptions::default()).unwrap();
        assert!(out.is_file(), "build 应写出可执行文件");

        let status = std::process::Command::new(&out).status().unwrap();
        assert!(status.success(), "可执行文件应返回 0");
    }

    #[cfg(all(feature = "llvm", not(windows)))]
    #[test]
    fn build_cone_package_with_cone_deps_produces_exe_and_stdout_ok() {
        let dir = tempdir().unwrap();

        let lib = dir.path().join("lib");
        let lib_src = lib.join("src");
        std::fs::create_dir_all(&lib_src).unwrap();
        std::fs::write(
            lib.join("Cone.toml"),
            r#"
[cone]
name = "fixture-lib"
version = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            lib_src.join("api.scoop"),
            r#"
package fixtures.t1107.lib

import scoop.core.*

public struct Token(val value: Int)
"#,
        )
        .unwrap();
        std::fs::write(lib_src.join("main.scoop"), "package fixtures.t1107.lib\n").unwrap();

        let app = dir.path().join("app");
        let app_src = app.join("src");
        let app_cone = app.join("cone");
        std::fs::create_dir_all(&app_src).unwrap();
        std::fs::create_dir_all(&app_cone).unwrap();
        std::fs::write(
            app.join("Cone.toml"),
            r#"
[cone]
name = "fixture-app"
version = "0.0.0"

[dependencies]
fixture-lib = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            app_src.join("main.scoop"),
            r#"
package fixtures.t1107.app

import scoop.core.*
import fixtures.t1107.lib.*

public fun unused(x: Token): Int / Pure! {
    1
}

public fun main() / Pure! {
    println("ok")
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let pkg = scoopc::cone::load_cone_source_package(&lib).unwrap();
        let out_cone = app_cone.join("fixture-lib-0.0.0.cone");
        scoopc::cone::write_cone_archive_v0(&session, &pkg, &out_cone).unwrap();

        let out = dir.path().join("out").join("a");
        super::run(app, Some(out.clone()), super::BuildOptions::default()).unwrap();
        assert!(out.is_file(), "build 应写出可执行文件");

        let output = std::process::Command::new(&out).output().unwrap();
        assert!(output.status.success(), "可执行文件应返回 0");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "ok\n");
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_emit_llvm_writes_ll_file() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("main.ll");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/spec_doctest/overview_minimal_main.scoop");

        super::run(
            input,
            Some(out.clone()),
            super::BuildOptions {
                emit: super::BuildEmit::LlvmIr,
                ..super::BuildOptions::default()
            },
        )
        .unwrap();

        let ll = std::fs::read_to_string(&out).unwrap();
        assert!(ll.contains("define i32 @main("), "应输出 LLVM IR");
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_emit_llvm_dynamic_entry_publication_keeps_plain_carrier_targets_buildable() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("dynamic_entry_publication.ll");

        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../tests/fixtures/build/effect_lowered_dynamic_entry_publication_emit_llvm.scoop",
        );

        super::run(
            input,
            Some(out.clone()),
            super::BuildOptions {
                emit: super::BuildEmit::LlvmIr,
                opt_level: Some(OptLevel::O0),
                ..super::BuildOptions::default()
            },
        )
        .unwrap();

        let ll = std::fs::read_to_string(&out).unwrap();
        assert!(
            ll.contains("define i64 @__scoop_abi0_fun__fixtures_build_Base_ping__h")
                && ll.contains("define i64 @__scoop_abi0_fun__fixtures_build_Derived_ping__h")
                && ll.contains(
                    "define ptr addrspace(1) @__scoop_abi0_fun__fixtures_build_makeClosure__h"
                )
                && ll.contains("define internal i64 @__scoop_priv0__closure_body__h"),
            "metadata-only plain carrier targets 也必须有已发布的 plain callable body:\n{ll}"
        );
        assert!(
            !ll.contains("__scoop_priv0__lowered_vtable_dynamic_entry__h")
                && !ll.contains("__scoop_priv0__lowered_itable_dynamic_entry__h")
                && !ll.contains("__scoop_priv0__lowered_closure_dynamic_entry__h")
                && !ll.contains("%scoop.lowered.Step__h"),
            "NoOutward carrier target 不应重新发布 effect-step dynamic entry 或 Step shell:\n{ll}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_single_file_request_roots_exclude_stdlib_support_sources() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        std::fs::write(&input, "fun main() {}\n").unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();

        assert_eq!(
            front.input().mir_request_source_paths(),
            vec![input.clone()],
            "单文件 build 只能让用户入口源贡献 MIR request roots"
        );
        assert!(
            front
                .monomorph_requests()
                .iter()
                .all(|request| request.request_source_path != input),
            "不含泛型调用的单文件入口不应产生用户源 monomorph roots；support-source 请求只可作为 reachable binding 存在: {:?}",
            front.monomorph_requests()
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_cone_request_roots_exclude_stdlib_support_sources() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-request-roots"
version = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            src.join("main.scoop"),
            "package fixture.request_roots\nfun main() {}\n",
        )
        .unwrap();
        std::fs::write(
            src.join("helper.scoop"),
            "package fixture.request_roots\nfun helper() {}\n",
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&pkg, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let cone_root = pkg.canonicalize().unwrap();
        let roots = front.input().mir_request_source_paths();

        assert_eq!(
            roots.len(),
            2,
            "consumer cone 的两个 source 都应是 request roots"
        );
        assert!(
            roots.iter().all(|path| path.starts_with(&cone_root)),
            "cone build request roots 不应包含 stdlib/sysroot support sources: {roots:?}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_context_keeps_bare_file_input_as_virtual_cone_inside_cone_root() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-file-mode"
version = "0.0.0"
"#,
        )
        .unwrap();
        let main = src.join("main.scoop");
        let helper = src.join("helper.scoop");
        std::fs::write(
            &main,
            "package fixture.file_mode\nfun main(): Int { return 0 }\n",
        )
        .unwrap();
        std::fs::write(
            &helper,
            "package fixture.file_mode\nfun helper(): Int { return 1 }\n",
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&main, None).unwrap();

        assert!(
            build_context.input().is_virtual_cone(),
            "裸文件输入即使位于 cone root 下，也必须保持 virtual-cone contract"
        );
        assert!(
            build_context.deps().is_empty(),
            "virtual-cone contract 不应偷偷解析 explicit cone 依赖"
        );
        assert_eq!(
            build_context.input().mir_request_source_paths(),
            vec![main.clone()]
        );

        let front = super::run_frontend(&session, build_context).unwrap();
        assert!(
            front
                .input()
                .sources()
                .iter()
                .all(|source| source.path() != helper.as_path()),
            "bare file -> project frontend 不应自动把同 cone 的其它源文件塞进 context"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_entry_roots_skip_same_file_unreachable_generic_helper() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");

        std::fs::write(
            &input,
            r#"
package fixture.request_roots

fun <T> id(x: T): T {
    return x
}

fun helperOnly(): Int {
    return id<Int>(1)
}

fun main(): Int {
    return 0
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        assert!(
            front
                .monomorph_requests()
                .iter()
                .any(|request| request.key.symbol.fqn == "fixture.request_roots.id"),
            "test setup 应先证明同源 helper 的泛型调用会被 typecheck 记录为 request"
        );

        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let materialized = lowered
            .materialized_mir()
            .expect("build frontend 应保留 materialized MIR");
        assert!(
            materialized
                .instance_keys
                .iter()
                .all(|key| key.template.fqn != "fixture.request_roots.id"),
            "entry-main rooted materialization 不应让同源未触达 helper 的 id<Int> 成为实例 root: {:?}",
            materialized.instance_keys
        );
        assert!(
            lowered.file.items.iter().all(|item| !matches!(
                item,
                scoopc::hir::Item::Fun(fun) if fun.fqn == "fixture.request_roots.id::<Int>"
            )),
            "HIR 兼容输出不应包含未从 main 触达的 id::<Int>"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_entry_roots_skip_unreachable_cone_source_generic_helper() {
        let dir = tempdir().unwrap();
        let pkg = dir.path().join("pkg");
        let src = pkg.join("src");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(
            pkg.join("Cone.toml"),
            r#"
[cone]
name = "fixture-entry-roots"
version = "0.0.0"
"#,
        )
        .unwrap();
        std::fs::write(
            src.join("main.scoop"),
            "package fixture.entry_roots\nfun main(): Int { return 0 }\n",
        )
        .unwrap();
        std::fs::write(
            src.join("helper.scoop"),
            r#"
package fixture.entry_roots

fun <T> id(x: T): T {
    return x
}

fun helperOnly(): Int {
    return id<Int>(1)
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&pkg, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        assert!(
            front.input().mir_request_source_paths().len() >= 2,
            "cone build 仍应把 consumer cone sources 作为 request-source 过滤集合"
        );
        assert!(
            front
                .monomorph_requests()
                .iter()
                .any(|request| request.key.symbol.fqn == "fixture.entry_roots.id"),
            "test setup 应先证明非入口源文件中的泛型调用会被收集到 request 列表"
        );

        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let materialized = lowered
            .materialized_mir()
            .expect("build frontend 应保留 materialized MIR");
        assert!(
            materialized
                .instance_keys
                .iter()
                .all(|key| key.template.fqn != "fixture.entry_roots.id"),
            "entry-main rooted materialization 不应让 consumer cone 内未触达 helper 的 id<Int> 成为实例 root: {:?}",
            materialized.instance_keys
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_handles_imported_fun_signature_hints_with_utf8_comments() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/std_sync_basic.scoop");

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();

        assert!(
            lowered.file.items.iter().any(|item| {
                matches!(
                    item,
                    scoopc::hir::Item::Fun(fun) if fun.fqn == "main" || fun.fqn.ends_with(".main")
                )
            }),
            "build frontend 应成功 lower `std_sync_basic.scoop`，而不是在 imported fun signature hint 上 panic"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_keeps_distinct_effect_row_generic_instances() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");

        std::fs::write(
            &input,
            r#"
package fixtures.t5000e2c

import scoop.core.*

effect Boom {
    fun boom(): Int
}

effect Zap {
    fun zap(): Int
}

fun <T, eff E> id(x: T): T {
    return x
}

fun <T, eff E> wrap(x: T): T {
    return id<T, eff E>(x)
}

private fun entry(): Int {
    val a = wrap<Int, eff Boom>(1)
    val b = wrap<Int, eff Zap>(2)
    return a + b
}

fun main(): Int / Pure! {
    return entry()
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let materialized = lowered
            .materialized_mir()
            .expect("build frontend 应保留 materialized MIR");
        let callable_view = lowered
            .materialized_callable_view()
            .expect("build frontend 应暴露 materialized callable view");
        let materialized_fun_fqns = materialized
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                scoopc::mir::Item::Fun(fun) if fun.body.is_some() => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let lowered_fun_fqns = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                scoopc::hir::Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        for fqn in [
            "fixtures.t5000e2c.wrap::<Int, eff fixtures.t5000e2c.Boom>",
            "fixtures.t5000e2c.wrap::<Int, eff fixtures.t5000e2c.Zap>",
            "fixtures.t5000e2c.id::<Int, eff fixtures.t5000e2c.Boom>",
            "fixtures.t5000e2c.id::<Int, eff fixtures.t5000e2c.Zap>",
        ] {
            assert!(
                lowered_fun_fqns.contains(&fqn),
                "build frontend lowering 应保留实例 `{fqn}`，实际函数集合为: {lowered_fun_fqns:?}"
            );
            assert!(
                materialized_fun_fqns.contains(&fqn),
                "build frontend 应保留实例 `{fqn}` 的 materialized MIR body，实际 MIR 函数集合为: {materialized_fun_fqns:?}"
            );
            let root = callable_view
                .callable(fqn)
                .expect("callable view 应能直接查询 materialized root body");
            let owner = callable_view
                .owner_of_callable(fqn)
                .expect("callable view 应能从 root body 反查所属实例");
            let family = callable_view
                .instance(owner)
                .expect("callable view 应能从实例读取 canonical family");
            assert_eq!(root.fqn, fqn);
            assert_eq!(family.root_fqn(), fqn);
            assert!(
                family.summary().body_known,
                "有 body 的 root callable 应在 canonical view 中携带 body-known summary"
            );
        }
        assert_eq!(
            callable_view.instances().count(),
            materialized.instance_keys.len(),
            "callable view 应覆盖 production frontend 保留的全部实例"
        );
        for family in callable_view.instances() {
            assert!(
                family.summary().body_known == family.root_body().is_some(),
                "canonical callable view 中的 body-known 应与 root body 是否存在一致：{}",
                family.root_fqn()
            );
        }
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_production_codegen_entry_consumes_materialized_pass_view() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        let out = dir.path().join("build.ll");

        std::fs::write(
            &input,
            r#"
package fixtures.t5000h0c

import scoop.core.*

fun <T> id(x: T): T {
    return x
}

object Box {
    fun <T> memberId(x: T): T {
        return id(x)
    }
}

fun main(): Int {
    val a: Int = id(1)
    val b: Int = Box.memberId(2)
    return a + b
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let (source_map, entry_source_id) = super::build_codegen_source_map(&session, &front);

        scoopc::pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            None,
            &out,
            front.input().entry_main_fqn(),
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .expect(
            "build frontend 的 stage-only production codegen 应显式消费 materialized pass view",
        );
        let ir = std::fs::read_to_string(&out).unwrap();

        for symbol_prefix in [
            "__scoop_abi0_fun__fixtures_t5000h0c_id__h",
            "__scoop_abi0_fun__fixtures_t5000h0c_Box_memberId__h",
        ] {
            assert!(
                ir.contains(symbol_prefix),
                "build production codegen 入口应通过 AbiMangler 保留实例级 exported identity `{symbol_prefix}`，实际 IR:\n{ir}"
            );
        }
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_frontend_does_not_eager_materialize_unused_owner_specialized_getter() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");

        std::fs::write(
            &input,
            r#"
package fixtures.t5000e2r

import scoop.core.*

struct Box<T>(val value: T) {
    val doubled: T
        get() = this.value
}

fun entry(): Int {
    val box: Box<Int> = Box(1)
    val unused: Box<String> = Box("x")
    return box.doubled
}

fun main(): Int / Pure! {
    return entry()
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let lowered_member_fqns = lowered
            .member_funs
            .iter()
            .map(|fun| fun.fqn.as_str())
            .collect::<Vec<_>>();

        assert!(
            lowered_member_fqns.contains(&"fixtures.t5000e2r.Box.doubled::<Int>"),
            "build frontend 应保留从请求根实际可达的 getter 实例，实际成员函数集合为: {lowered_member_fqns:?}"
        );
        assert!(
            !lowered_member_fqns.contains(&"fixtures.t5000e2r.Box.doubled::<String>"),
            "build frontend 不应因为 `TypeStore` 中出现 `Box<String>` 就 eager materialize 未调用 getter，实际成员函数集合为: {lowered_member_fqns:?}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_publishes_request_source_abi_shells_for_unreachable_effectful_helpers() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        let out = dir.path().join("abi.ll");

        std::fs::write(
            &input,
            r#"
package fixtures.build_abi_visibility

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

        let session = session();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let abi_visibility_lowered =
            super::abi_visibility_lowered_hir_for_build(&session, &front, OptLevel::O0)
                .unwrap()
                .expect("build 应额外构造 request-source ABI visibility handoff");
        let (source_map, entry_source_id) = super::build_codegen_source_map(&session, &front);

        scoopc::pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            Some(abi_visibility_lowered),
            &out,
            front.input().entry_main_fqn(),
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .unwrap();

        let ir = std::fs::read_to_string(&out).unwrap();
        assert!(
            ir.contains("__scoop_priv0__lowered_dynamic_invoke__h")
                && ir.contains("__scoop_priv0__lowered_direct_invoke__h"),
            "ABI visibility handoff 应让不可达 effectful helper 的 canonical invoke shell family 出现在 build IR 中：\n{ir}"
        );
        assert!(
            !ir.contains("scoop.effect.frame."),
            "纯 main 的 build 不应为了 ABI shell 可见性而偷偷生成 legacy effect frame IR：\n{ir}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_lowers_reachable_self_contained_effect_body_without_legacy_frames() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        let out = dir.path().join("lowered.ll");

        write_reachable_legacy_effect_fixture(&input);

        let session = session();
        let build_context = super::load_build_context(&input, None).unwrap();
        let front = super::run_frontend(&session, build_context).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let abi_visibility_lowered =
            super::abi_visibility_lowered_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let (source_map, entry_source_id) = super::build_codegen_source_map(&session, &front);

        scoopc::pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            abi_visibility_lowered,
            &out,
            front.input().entry_main_fqn(),
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .expect("reachable self-contained handle 应由 lowering 正常生成 IR");

        let ir = std::fs::read_to_string(&out).unwrap();
        assert!(
            ir.contains("__scoop_priv0__lowered_"),
            "IR 应包含 canonical private symbol family，而不是空壳输出：\n{ir}"
        );
        assert!(
            !ir.contains("scoop_effect_handler_stack") && !ir.contains("scoop_effect_outcome"),
            "IR 不应回落到 legacy handler-stack/outcome runtime：\n{ir}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn no_hidden_legacy_fallback_for_default_build_output() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        let out = dir.path().join("default_lowered.ll");

        write_reachable_legacy_effect_fixture(&input);

        super::run(
            input,
            Some(out.clone()),
            super::BuildOptions {
                emit: super::BuildEmit::LlvmIr,
                opt_level: Some(OptLevel::O0),
                ..super::BuildOptions::default()
            },
        )
        .expect("default build should lower without hidden legacy fallback");
        let ir = std::fs::read_to_string(&out).unwrap();

        assert!(
            ir.contains("__scoop_priv0__lowered_"),
            "default build should emit canonical private symbols:\n{ir}"
        );
        assert!(
            !ir.contains("scoop_effect_handler_stack") && !ir.contains("scoop_effect_outcome"),
            "default build must not retry or embed legacy handler-stack/outcome lowering:\n{ir}"
        );
    }
}
