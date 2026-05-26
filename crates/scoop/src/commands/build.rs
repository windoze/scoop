//! `scoop build` facade.
//!
//! The facade owns CLI, project graph loading, per-cone scheduling, and link
//! subprocess orchestration. Compiler stages run only inside `scoopc`.

pub(crate) mod concurrency;
mod incremental;
pub(crate) mod layout;
mod scheduler;
mod virtual_cone;

use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use scoop_project_model::{ConeKind, OptLevel, SourceConeGraph, SourceConeRole};
use thiserror::Error;

use super::FacadeSessionOptions;

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

impl BuildEmit {
    #[cfg(feature = "llvm")]
    fn compiler_kind(self) -> Option<&'static str> {
        match self {
            BuildEmit::Executable => None,
            BuildEmit::LlvmIr => Some("llvm-ir"),
            BuildEmit::Obj => Some("obj"),
            BuildEmit::Asm => Some("asm"),
        }
    }
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
    pub profile: BuildProfile,
    pub opt_level: Option<OptLevel>,
    pub incremental: bool,
    pub jobs: std::num::NonZeroUsize,
    pub session_options: FacadeSessionOptions,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            emit: BuildEmit::Executable,
            entry_package: None,
            profile: BuildProfile::Debug,
            opt_level: None,
            incremental: true,
            jobs: concurrency::default_build_jobs(),
            session_options: FacadeSessionOptions::new(),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("`--entry-package` 仅支持 cone 包目录输入：{input}")]
#[diagnostic(code(scoop::driver::entry_package_only_for_cone))]
pub(crate) struct EntryPackageOnlyForCone {
    input: String,
}

struct LoadedBuildProject {
    cone_root: PathBuf,
    graph: SourceConeGraph,
    manifest: scoop_project_model::ConeManifest,
}

struct VirtualConeCleanup(Option<PathBuf>);

impl Drop for VirtualConeCleanup {
    fn drop(&mut self) {
        if let Some(root) = self.0.take() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

/// 执行 `scoop build <input> [-o <output>]`。
pub fn run(input: PathBuf, output: Option<PathBuf>, options: BuildOptions) -> Result<()> {
    let BuildOptions {
        emit,
        entry_package,
        profile,
        opt_level: opt_level_override,
        incremental,
        jobs,
        session_options,
    } = options;
    let session_options = session_options.with_env_fallback();
    let incremental = incremental && session_options.sysroot_overlay().is_none();

    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;
    let original_input_was_file = input.is_file();
    if original_input_was_file && entry_package.is_some() {
        return Err(EntryPackageOnlyForCone {
            input: input.display().to_string(),
        }
        .into());
    }

    if emit != BuildEmit::Executable {
        #[cfg(feature = "llvm")]
        {
            let opt_level = resolve_opt_level(opt_level_override, None, profile);
            let output = output.unwrap_or_else(|| default_output_path_for_emit(emit));
            ensure_output_parent_dir(&output)?;
            run_emit_subprocess(&input, &output, emit, opt_level, &session_options)?;
            return Ok(());
        }
        #[cfg(not(feature = "llvm"))]
        {
            let _ = output;
            let _ = opt_level_override;
            let flag = match emit {
                BuildEmit::LlvmIr => "--emit-llvm",
                BuildEmit::Obj => "--emit-obj",
                BuildEmit::Asm => "--emit-asm",
                BuildEmit::Executable => unreachable!(),
            };
            return Err(miette::miette!(
                "`{flag}` 需要启用 LLVM 后端：请使用默认 features 构建 `scoop`"
            ));
        }
    }

    let mut virtual_cleanup = VirtualConeCleanup(None);
    let build_root = if original_input_was_file {
        let root = virtual_cone::materialize_single_file(&input, profile)?;
        virtual_cleanup.0 = Some(root.clone());
        root
    } else {
        input.clone()
    };
    let project = load_build_project(&build_root, &session_options)?;
    let opt_level = resolve_opt_level(
        opt_level_override,
        project.manifest.native_build.opt_level,
        profile,
    );
    let output = output.unwrap_or_else(|| {
        if original_input_was_file {
            default_output_path_for_emit(BuildEmit::Executable)
        } else {
            layout::cone_exe_path(
                &project.cone_root,
                None,
                profile.as_str(),
                &project.manifest.cone.name,
            )
        }
    });
    ensure_output_parent_dir(&output)?;
    if output.exists() && output.is_dir() {
        return Err(miette::miette!("输出路径是目录：{}", output.display()));
    }

    let incremental_ctx = if !incremental || !cfg!(feature = "llvm") || original_input_was_file {
        None
    } else {
        let expected_out = layout::cone_exe_path(
            &project.cone_root,
            None,
            profile.as_str(),
            &project.manifest.cone.name,
        );
        if output == expected_out {
            Some((
                project.cone_root.clone(),
                layout::cone_build_json_path(&project.cone_root, None, profile.as_str()),
            ))
        } else {
            None
        }
    };

    let mut computed_fingerprint = None;
    if let Some((cone_root, build_json)) = incremental_ctx.clone()
        && output.is_file()
        && let Some(cached) = incremental::read_cached_fingerprint(&build_json)?
    {
        let fp = incremental::compute_cone_build_fingerprint_with_session_options(
            &cone_root,
            profile.as_str(),
            entry_package.as_deref(),
            opt_level,
            &session_options,
        )?;
        if fp.fingerprint == cached {
            eprintln!("skipping build (cache hit)");
            return Ok(());
        }
        computed_fingerprint = Some(fp);
    }

    let fp = match computed_fingerprint {
        Some(fp) => fp,
        None => incremental::compute_cone_build_fingerprint_with_session_options(
            &project.cone_root,
            profile.as_str(),
            entry_package.as_deref(),
            opt_level,
            &session_options,
        )?,
    };

    let concurrency_strategy: Box<dyn concurrency::ConcurrencyStrategy> =
        Box::new(concurrency::FixedJobsStrategy::new(jobs));
    let subprocess_cone_compiler: Box<dyn concurrency::SubprocessConeCompiler> =
        Box::new(concurrency::LocalProcessConeCompiler::new());

    scheduler::dispatch_local_dependency_cones(
        &project.graph,
        &fp,
        &scheduler::ConeBuildDispatch {
            strategy: &*concurrency_strategy,
            compiler: &*subprocess_cone_compiler,
            opt_level,
            extra_sysroot_dependencies: session_options.extra_sysroot_dependencies(),
            sysroot_overlay: session_options.sysroot_overlay(),
        },
    )?;

    #[cfg(feature = "llvm")]
    run_link_from_artifacts(&project, &output, profile, &fp)?;
    #[cfg(not(feature = "llvm"))]
    return Err(miette::miette!(
        "子命令 `build` 需要启用 LLVM 后端才能生成可执行文件"
    ));

    if let Some((cone_root, build_json)) = incremental_ctx
        && output.is_file()
    {
        let fp = incremental::compute_cone_build_fingerprint_with_session_options(
            &cone_root,
            profile.as_str(),
            entry_package.as_deref(),
            opt_level,
            &session_options,
        )?;
        incremental::write_build_json(
            &build_json,
            profile.as_str(),
            entry_package.as_deref(),
            opt_level,
            &fp,
        )?;
    }

    Ok(())
}

fn load_build_project(
    cone_root: &Path,
    session_options: &FacadeSessionOptions,
) -> Result<LoadedBuildProject> {
    if !cone_root.is_dir() {
        return Err(miette::miette!(
            "输入既不是文件也不是目录：{}",
            cone_root.display()
        ));
    }
    let pkg = scoop_project_model::load_cone_source_package(cone_root)?;
    if pkg.manifest.cone.kind != ConeKind::Bin {
        return Err(miette::miette!(
            "只有 `bin` cone 可作为 executable consumer 输入；`{}` 声明为 `{}` cone",
            pkg.manifest.cone.name,
            pkg.manifest.cone.kind
        ));
    }
    let sysroot_root = scoop_project_model::default_sysroot_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（source cone graph）")?;
    let graph = scoop_project_model::load_source_cone_graph_for_consumer_package(
        pkg.clone(),
        &sysroot_root,
        session_options.sysroot_overlay(),
        &[],
        session_options.extra_sysroot_dependencies(),
    )?;
    Ok(LoadedBuildProject {
        cone_root: pkg.root,
        graph,
        manifest: pkg.manifest,
    })
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
fn run_emit_subprocess(
    input: &Path,
    output: &Path,
    emit: BuildEmit,
    opt_level: OptLevel,
    session_options: &FacadeSessionOptions,
) -> Result<()> {
    let kind = emit
        .compiler_kind()
        .expect("non-executable emits have compiler artifact kind");
    let mut cmd = crate::compiler_tool::command()?;
    cmd.arg("emit-artifact");
    cmd.arg("--kind").arg(kind);
    crate::compiler_tool::arg_path(&mut cmd, "--input", input);
    crate::compiler_tool::arg_path(&mut cmd, "--out", output);
    cmd.arg("--opt-level").arg(opt_level.as_str());
    session_options.apply_to_command(&mut cmd);
    crate::compiler_tool::run_capture(cmd, "emit-artifact")?;
    Ok(())
}

#[cfg(feature = "llvm")]
fn run_link_from_artifacts(
    project: &LoadedBuildProject,
    output: &Path,
    profile: BuildProfile,
    build_fingerprint: &incremental::BuildFingerprint,
) -> Result<()> {
    let consumer_id = project.graph.consumer_id();
    let consumer_fp = build_fingerprint
        .per_cone
        .get(&consumer_id)
        .ok_or_else(|| {
            miette::miette!(
                "link-cone 准备阶段缺少 consumer cone {} 的 fingerprint",
                consumer_id.as_u32()
            )
        })?;
    let consumer_objects = artifact_object_files(&consumer_fp.artifact_dir, "consumer")?;
    let (consumer_obj, consumer_extra_objs) = split_consumer_objects(consumer_objects.paths)?;

    let mut dep_objs = consumer_extra_objs;
    let mut extern_libs = Vec::new();
    append_unique(&mut extern_libs, consumer_objects.extern_libs);
    for unit in project.graph.compilation_units() {
        if unit.role() != SourceConeRole::LocalDependency {
            continue;
        }
        let Some(cone_fp) = build_fingerprint.per_cone.get(&unit.id()) else {
            continue;
        };
        let objects = artifact_object_files(
            &cone_fp.artifact_dir,
            &format!("cone {}", unit.id().as_u32()),
        )?;
        dep_objs.extend(objects.paths);
        append_unique(&mut extern_libs, objects.extern_libs);
    }

    let link_plan = native_link_plan(&project.graph)?;
    let cone = &project.manifest.cone;
    let cone_key = format!("{}@{}", cone.name, cone.version);
    let link_dir = layout::cone_link_dir(&project.cone_root, None, profile.as_str(), &cone_key);
    let parent_inputs_fingerprint = build_fingerprint
        .per_cone
        .get(&build_fingerprint.consumer_cone_id)
        .map(|fp| fp.inputs_fingerprint.clone())
        .unwrap_or_default();

    run_link_cone_subprocess(LinkConeDispatchRequest {
        kind: cone.kind,
        consumer_obj,
        dep_objs,
        runtime_artifact_dir: link_dir.join("runtime"),
        output_dir: link_dir,
        binary_path: output.to_path_buf(),
        extern_libs,
        link_flags: link_plan.link_flags,
        linker: link_plan.linker.map(PathBuf::from),
        inputs_fingerprint: parent_inputs_fingerprint,
        cone_id: Some(cone_key),
    })
}

#[cfg(feature = "llvm")]
struct ArtifactObjectFiles {
    paths: Vec<PathBuf>,
    extern_libs: Vec<String>,
}

#[cfg(feature = "llvm")]
fn artifact_object_files(artifact_dir: &Path, label: &str) -> Result<ArtifactObjectFiles> {
    let (manifest, _) = scoop_project_model::read_manifest_and_inputs_fingerprint(artifact_dir)
        .map_err(|err| {
            miette::miette!(
                "link-cone 准备阶段无法读取 {label} artifact `{}`: {err}",
                artifact_dir.display()
            )
        })?;
    let mut paths = Vec::with_capacity(manifest.object_files.len());
    for file_name in manifest.object_files {
        let path = artifact_dir
            .join(scoop_project_model::CONE_ARTIFACT_OBJS_DIR_NAME)
            .join(&file_name)
            .canonicalize()
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "{label} artifact `{}` 缺少 object `{file_name}`",
                    artifact_dir.display()
                )
            })?;
        paths.push(path);
    }
    Ok(ArtifactObjectFiles {
        paths,
        extern_libs: manifest.extern_libs,
    })
}

#[cfg(feature = "llvm")]
fn split_consumer_objects(mut paths: Vec<PathBuf>) -> Result<(PathBuf, Vec<PathBuf>)> {
    if paths.is_empty() {
        return Err(miette::miette!("consumer cone artifact 没有 object 文件"));
    }
    let primary_idx = paths
        .iter()
        .position(|path| path.file_name().and_then(|s| s.to_str()) == Some("scoop.o"))
        .unwrap_or(0);
    let primary = paths.remove(primary_idx);
    Ok((primary, paths))
}

#[cfg(feature = "llvm")]
fn append_unique(out: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !out.iter().any(|existing| existing == &value) {
            out.push(value);
        }
    }
}

#[cfg(feature = "llvm")]
#[derive(Debug, Clone)]
struct LinkConeDispatchRequest {
    kind: ConeKind,
    consumer_obj: PathBuf,
    dep_objs: Vec<PathBuf>,
    runtime_artifact_dir: PathBuf,
    output_dir: PathBuf,
    binary_path: PathBuf,
    extern_libs: Vec<String>,
    link_flags: Vec<String>,
    linker: Option<PathBuf>,
    inputs_fingerprint: Vec<u8>,
    cone_id: Option<String>,
}

#[cfg(feature = "llvm")]
#[derive(Debug, Error, Diagnostic)]
#[error(
    "scoopc link-cone 失败（cone={label}, status={status}）\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
)]
#[diagnostic(code(scoopld::link_failed))]
struct LinkConeSubprocessLinkFailed {
    label: String,
    status: String,
    stdout: String,
    stderr: String,
}

#[cfg(feature = "llvm")]
#[derive(Debug, Error, Diagnostic)]
#[error(
    "scoopc link-cone 子进程失败（cone={label}, status={status}）\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
)]
#[diagnostic(code(scoop::driver::link_cone_failed))]
struct LinkConeSubprocessFailed {
    label: String,
    status: String,
    stdout: String,
    stderr: String,
}

#[cfg(feature = "llvm")]
fn run_link_cone_subprocess(request: LinkConeDispatchRequest) -> Result<()> {
    let label = request.cone_id.as_deref().unwrap_or("link-cone");
    let mut cmd = crate::compiler_tool::command()?;
    cmd.arg("link-cone");
    cmd.arg("--kind").arg(request.kind.as_str());
    cmd.arg("--consumer-obj").arg(&request.consumer_obj);
    for dep_obj in &request.dep_objs {
        cmd.arg("--dep-obj").arg(dep_obj);
    }
    cmd.arg("--runtime-artifact-dir")
        .arg(&request.runtime_artifact_dir);
    cmd.arg("--out").arg(&request.output_dir);
    cmd.arg("--binary-out").arg(&request.binary_path);
    cmd.arg("--inputs-fingerprint")
        .arg(hex_lower(&request.inputs_fingerprint));
    if let Some(cone_id) = request.cone_id.as_deref() {
        cmd.arg("--cone-id").arg(cone_id);
    }
    if let Some(linker) = &request.linker {
        cmd.arg("--linker").arg(linker);
    }
    for lib in &request.extern_libs {
        cmd.arg("--extern-lib").arg(lib);
    }
    for flag in &request.link_flags {
        cmd.arg("--link-flag").arg(flag);
    }

    let output = cmd
        .output()
        .into_diagnostic()
        .wrap_err("无法启动 scoopc link-cone")?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = output.status.to_string();
        if stderr.contains("scoopld::link_failed") {
            return Err(LinkConeSubprocessLinkFailed {
                label: label.to_string(),
                status,
                stdout,
                stderr,
            }
            .into());
        }
        return Err(LinkConeSubprocessFailed {
            label: label.to_string(),
            status,
            stdout,
            stderr,
        }
        .into());
    }
    Ok(())
}

#[cfg(feature = "llvm")]
fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(feature = "llvm")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeLinkPlan {
    linker: Option<String>,
    link_flags: Vec<String>,
}

#[cfg(feature = "llvm")]
/// Builds the final native linker configuration from every loaded source cone.
fn native_link_plan(graph: &SourceConeGraph) -> Result<NativeLinkPlan> {
    let mut linker: Option<(String, String)> = None;
    let mut link_flags = Vec::new();
    let mut use_cxx_linker_driver = false;

    for node in graph.nodes() {
        if let Some(candidate) = node.native_build.linker.as_deref() {
            if let Some((existing, owner)) = &linker {
                if existing != candidate {
                    return Err(miette::miette!(
                        "loaded source cones declare conflicting `[native-build].linker` values: `{owner}` uses `{existing}`, `{}` uses `{candidate}`",
                        node.manifest.cone.name
                    ));
                }
            } else {
                linker = Some((candidate.to_owned(), node.manifest.cone.name.clone()));
            }
        }
        if !node.native_build.cxx_sources.is_empty() {
            use_cxx_linker_driver = true;
        }
        link_flags.extend(node.native_build.link_flags.iter().cloned());
    }

    let linker = linker
        .map(|(linker, _owner)| linker)
        .or_else(|| use_cxx_linker_driver.then(|| "clang++".to_string()));
    Ok(NativeLinkPlan { linker, link_flags })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_opt_level_prefers_cli_over_manifest() {
        let out = resolve_opt_level(Some(OptLevel::O2), Some(OptLevel::O0), BuildProfile::Debug);
        assert_eq!(out, OptLevel::O2);
    }

    #[test]
    fn resolve_opt_level_uses_manifest_when_cli_missing() {
        let out = resolve_opt_level(None, Some(OptLevel::Oz), BuildProfile::Release);
        assert_eq!(out, OptLevel::Oz);
    }

    #[test]
    fn default_output_path_for_emit_matches_platform() {
        assert_eq!(
            default_output_path_for_emit(BuildEmit::LlvmIr),
            PathBuf::from("a.ll")
        );
        assert!(
            default_output_path_for_emit(BuildEmit::Executable)
                .to_string_lossy()
                .starts_with('a')
        );
    }
}
