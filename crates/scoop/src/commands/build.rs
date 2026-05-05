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

#[derive(Debug)]
struct BuildInput {
    /// 当前编译单元的全部源文件。
    ///
    /// 约定（T1315a）：
    /// - 始终包含 stdlib 注入的 `stdlib/*.scoop`（纯 Scoop prelude）；
    /// - 单文件模式下额外包含 1 个 user source；
    /// - cone 包模式下额外包含 `src/**/*.scoop`。
    sources: Vec<scoopc::source::SourceFile>,
    /// 可执行入口（选定的 `fun main` 所在源文件）在 `sources` 中的下标。
    main_index: usize,
    /// cone 包模式下的“锚点 main 文件”（`src/main.scoop`）在 `sources` 中的下标。
    ///
    /// 用途：
    /// - 当未显式配置 `entry-package` 时，用它的 package 作为默认 entry package；
    /// - 未来其它 driver/fixture 逻辑也可以用它作为“case 的稳定入口文件”。
    cone_anchor_main_index: Option<usize>,
    /// 若输入为 cone 包目录，则包含其 root 与 manifest（用于 T1107 依赖图解析）。
    cone_root: Option<PathBuf>,
    cone_manifest: Option<scoopc::cone::ConeManifest>,
    /// （cone 包模式）入口 package 覆盖（来自 CLI）。
    entry_package_override: Option<String>,
    /// 已选择的入口函数 FQN（仅 cone 包模式下会填充）。
    entry_main_fqn: Option<String>,
}

impl BuildInput {
    fn main_source(&self) -> &scoopc::source::SourceFile {
        &self.sources[self.main_index]
    }

    #[cfg(feature = "llvm")]
    fn is_mir_request_source_index(&self, idx: usize) -> bool {
        if let Some(root) = self.cone_root.as_ref() {
            return self.sources[idx].path().starts_with(root);
        }
        idx == self.main_index
    }

    #[cfg(feature = "llvm")]
    fn mir_request_source_paths(&self) -> Vec<PathBuf> {
        self.sources
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.is_mir_request_source_index(*idx))
            .map(|(_, source)| source.path().to_path_buf())
            .collect()
    }

    #[allow(dead_code)]
    fn cone_anchor_main_source(&self) -> Option<&scoopc::source::SourceFile> {
        self.cone_anchor_main_index.map(|idx| &self.sources[idx])
    }
}

#[derive(Debug)]
struct FrontendOutput {
    input: BuildInput,
    #[cfg(feature = "llvm")]
    asts: Vec<scoopc::ast::File>,
    #[cfg(feature = "llvm")]
    index: scoopc::resolve::Index,
    /// T0127/T5000: typecheck 收集到的带 call-site 来源的实例请求种子。
    ///
    /// 当前 build/frontend 主路径会把它交给 MIR materializer 建立 `InstanceKey` 集，
    /// 而不是回到 HIR eager lowering 现场扫描并克隆具体实例。
    #[cfg(feature = "llvm")]
    monomorph_requests: Vec<scoopc::monomorph::MonomorphRequest>,
    /// T0130: typecheck 阶段的 `TypeStore`。
    ///
    /// 用途：
    /// - 供 MIR materializer 把请求里的 `TypeId` / effect row re-intern 到实例化用的类型表；
    /// - 供 HIR compatibility lowering 只按显式 `InstanceKey` 集恢复当前 LLVM codegen 仍需要的
    ///   monomorphic HIR fun/member。
    #[cfg(feature = "llvm")]
    typecheck_types: scoopc::ty::TypeStore,
    #[cfg(feature = "llvm")]
    type_env: scoopc::typecheck::TypeEnv,
}

impl FrontendOutput {
    fn main_source(&self) -> &scoopc::source::SourceFile {
        self.input.main_source()
    }

    #[cfg(feature = "llvm")]
    #[allow(dead_code)]
    fn main_ast(&self) -> &scoopc::ast::File {
        &self.asts[self.input.main_index]
    }
}

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
        .input
        .sources
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
            session_options: SessionOptions::default(),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("`--entry-package` 仅支持 cone 包目录输入：{input}")]
#[diagnostic(code(scoop::driver::entry_package_only_for_cone))]
pub(crate) struct EntryPackageOnlyForCone {
    input: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("入口包 `{entry_package}` 中找不到入口函数 `fun main`")]
#[diagnostic(code(scoop::driver::entry_package_missing_main))]
pub(crate) struct EntryPackageMissingMain {
    entry_package: String,
    #[label("该 package 没有 `main`")]
    span: miette::SourceSpan,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "入口包 `{entry_package}` 的 `fun main` 不属于 consumer cone（它声明在依赖/其它 cone：{decl_file}）"
)]
#[diagnostic(code(scoop::driver::entry_package_main_not_in_consumer_cone))]
pub(crate) struct EntryPackageMainNotInConsumerCone {
    entry_package: String,
    decl_file: String,
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

    let entry_package_for_fingerprint = entry_package.clone();

    let input = input
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位输入文件")?;

    let input = load_build_input(&input, entry_package)?;
    let opt_level = resolve_opt_level(
        opt_level_override,
        input
            .cone_manifest
            .as_ref()
            .and_then(|m| m.native_build.opt_level),
        profile,
    );
    let output =
        output.unwrap_or_else(|| default_output_path_for_input_and_emit(&input, emit, profile));
    ensure_output_parent_dir(&output)?;

    if output.exists() && output.is_dir() {
        return Err(miette::miette!("输出路径是目录：{}", output.display()));
    }

    // T1124：粗粒度增量构建（仅对 cone 项目 + 可执行产物生效）。
    //
    // 重要：为避免污染 run-pass fixtures 的 stdout，这里统一把“cache hit”信息输出到 stderr。
    let mut computed_fingerprint: Option<incremental::BuildFingerprint> = None;
    let incremental_ctx = (|| {
        if !incremental || !cfg!(feature = "llvm") || emit != BuildEmit::Executable {
            return None;
        }
        let (root, manifest) = match (input.cone_root.as_ref(), input.cone_manifest.as_ref()) {
            (Some(root), Some(manifest)) => (root, manifest),
            _ => return None,
        };
        let expected_out = layout::cone_exe_path(root, None, profile.as_str(), &manifest.cone.name);
        if output != expected_out {
            return None;
        }
        let build_json = layout::cone_build_json_path(root, None, profile.as_str());
        Some((root.clone(), build_json))
    })();

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

    let session = scoopc::session::Session::with_options(session_options)?;

    let deps = match (&input.cone_root, &input.cone_manifest) {
        (Some(root), Some(manifest)) => deps::load_dependency_graph(manifest, root)?,
        _ => Vec::new(),
    };
    let warning_capture = scoopc::warnings::begin_capture();
    let front = run_frontend(&session, input, &deps)?;
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
                let _extern_libs = emit_refactor_llvm_artifact_for_build(
                    &session,
                    &front,
                    &output,
                    opt_level,
                    scoopc::effect_refactor_pipeline::LlvmArtifactKind::LlvmIr,
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
                let _extern_libs = emit_refactor_llvm_artifact_for_build(
                    &session,
                    &front,
                    &output,
                    opt_level,
                    scoopc::effect_refactor_pipeline::LlvmArtifactKind::Object,
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
                let _extern_libs = emit_refactor_llvm_artifact_for_build(
                    &session,
                    &front,
                    &output,
                    opt_level,
                    scoopc::effect_refactor_pipeline::LlvmArtifactKind::Asm,
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

fn load_build_input(input: &Path, entry_package_override: Option<String>) -> Result<BuildInput> {
    let stdlib_sources = load_stdlib_sources()?;

    // 单文件模式：保持 `scoop build <file.scoop>` 的原有行为。
    if input.is_file() {
        if entry_package_override.is_some() {
            return Err(EntryPackageOnlyForCone {
                input: input.display().to_string(),
            }
            .into());
        }
        let mut sources = stdlib_sources;
        let main_index = sources.len();
        sources.push(scoopc::source::SourceFile::load(input)?);
        return Ok(BuildInput {
            sources,
            main_index,
            cone_anchor_main_index: None,
            cone_root: None,
            cone_manifest: None,
            entry_package_override: None,
            entry_main_fqn: None,
        });
    }

    // cone 包模式：`scoop build <cone-root>`（按 T1102 规则定位 `src/main.scoop`）。
    if input.is_dir() {
        let pkg = scoopc::cone::load_cone_source_package(input)?;
        let mut sources = stdlib_sources;
        let stdlib_len = sources.len();
        sources.reserve(pkg.sources.len());
        let mut main_index = None;
        for (idx, path) in pkg.sources.iter().enumerate() {
            let source = scoopc::source::SourceFile::load(path)?;
            if source.path() == pkg.main.as_path() {
                main_index = Some(stdlib_len + idx);
            }
            sources.push(source);
        }

        let main_index = main_index.ok_or_else(|| {
            miette::miette!(
                "cone package 的 main 未出现在 sources 列表中：{}",
                pkg.main.display()
            )
        })?;

        return Ok(BuildInput {
            sources,
            main_index,
            cone_anchor_main_index: Some(main_index),
            cone_root: Some(pkg.root),
            cone_manifest: Some(pkg.manifest),
            entry_package_override,
            entry_main_fqn: None,
        });
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

fn default_output_path_for_input_and_emit(
    input: &BuildInput,
    emit: BuildEmit,
    profile: BuildProfile,
) -> PathBuf {
    if emit == BuildEmit::Executable
        && let (Some(root), Some(manifest)) =
            (input.cone_root.as_ref(), input.cone_manifest.as_ref())
    {
        return layout::cone_exe_path(root, None, profile.as_str(), &manifest.cone.name);
    }
    default_output_path_for_emit(emit)
}

fn default_stdlib_path() -> PathBuf {
    // 开发期路径：相对于 `crates/scoop` 的 `../../stdlib`。
    // 后续可随“工具链安装/分发”演进为：
    // - `SCOOP_STDLIB` 环境变量
    // - 或可执行文件旁的资源目录
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn load_stdlib_sources() -> Result<Vec<scoopc::source::SourceFile>> {
    let root = default_stdlib_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 stdlib 目录（T1315a）")?;

    let mut paths = Vec::new();
    collect_scoop_files(&root, &mut paths)?;

    // T0143：加载 sysroot 中的"可编译"源文件（如 sysroot/string.scoop）。
    // 这些文件含有需要编译的函数体（纯 Scoop 实现），与 stdlib 一起参与完整管线。
    // Sysroot::load_from() 会将它们排除在签名索引之外，避免双重声明。
    let sysroot_root = scoopc::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（T0143）")?;
    scoopc::sysroot::collect_compilable_sysroot_files(&sysroot_root, &mut paths)?;

    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        out.push(scoopc::source::SourceFile::load(&path)?);
    }
    Ok(out)
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
    mut input: BuildInput,
    deps: &[scoopc::cone::ConeArchiveApi],
) -> Result<FrontendOutput> {
    if input.sources.is_empty() {
        return Err(miette::miette!("内部错误：build 输入 sources 为空"));
    }

    // 先 parse 所有文件（cone 包模式下：`src/**/*.scoop`）。
    let mut asts = Vec::with_capacity(input.sources.len());
    for source in &input.sources {
        let ast =
            scoopc::effect_refactor_pipeline::enter_ast_stage(session, || session.parse(source))
                .map_err(miette::Report::from)?;
        asts.push(ast);
    }
    {
        let source_refs = input.sources.iter().collect::<Vec<_>>();
        let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
        // T1220b：在 resolver/index 之前按整编译单元裁剪 package-level `comptime if`
        //（未选中分支不进入后续阶段），并让条件表达式复用 const/comptime 的 typechecked 调用绑定主线。
        scoopc::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )
        .map_err(miette::Report::from)?;
    }

    // 先运行不依赖 resolver/index 的 typecheck 预检查（与 fixtures/typecheck pipeline 对齐）。
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        scoopc::typecheck::check_file_headers(source, ast).map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_struct_decls(source, ast).map_err(miette::Report::from)?;
    }

    // 构建 Index：sysroot 作为 cone 0；当前被 build 的 cone 作为 cone 1。
    let mut indexed: Vec<scoopc::resolve::IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        indexed.push(scoopc::resolve::IndexedFile {
            cone: scoopc::resolve::ConeId::new(1),
            source,
            file: ast,
        });
    }

    let mut index =
        scoopc::resolve::Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // T0629b：program boundary 的“库导出入口 / host entry points”由 Cone.toml 指定，
    // 在 typecheck 阶段按 entry point 规则强制 `Pure!`。
    if let Some(manifest) = input.cone_manifest.as_ref() {
        index.set_export_entry_points(manifest.export_entry_points.clone());
    }

    // T1107：注入 `.cone` 依赖的 public API（用于 import/类型检查）。
    //
    // cone id 分配约定：
    // - 0：sysroot
    // - 1：当前被 build 的 cone（consumer）
    // - 2+：按依赖拓扑序分配（deps 由 build/deps.rs 负责解析为 DAG 顺序）
    let mut env = scoopc::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)
        .map_err(miette::Report::from)?;
    for (next_dep_cone, dep) in (2_u32..).zip(deps.iter()) {
        let dep_cone = scoopc::resolve::ConeId::new(next_dep_cone);
        scoopc::cone::inject_cone_dependency_public_api(&mut index, &mut env, dep_cone, dep)?;
    }

    // T1113：选择“入口包”的 `fun main`，并将其作为 runtime entry point。
    //
    // 说明：
    // - 该选择仅在 cone 包模式下生效；单文件模式保持现有行为。
    // - 该选择会影响：
    //   - typecheck：仅对选定 `main` 按 entry point 规则强制 `Pure!`；
    //   - HIR lowering / LLVM codegen：选定 `main` 所在文件作为 entry source（允许 source-backed literals）。
    if input.cone_manifest.is_some() {
        select_cone_entry_main(&mut input, &asts, &mut index)?;
    }

    // resolver phase：headers + bodies（逐文件运行，但共享同一个 index）。
    let mut headers = Vec::with_capacity(input.sources.len());
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        let h = scoopc::resolve::check_file_headers(source, ast, &index)
            .map_err(miette::Report::from)?;
        headers.push(h);
    }
    for ((source, ast), h) in input
        .sources
        .iter()
        .zip(asts.iter_mut())
        .zip(headers.iter())
    {
        scoopc::resolve::check_file_bodies(source, ast, &index, h).map_err(miette::Report::from)?;
    }

    // type env：sysroot + 依赖 cones（已注入）+ 当前 cone 全部文件（用于跨文件 TypeRef lowering）。
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    let mut types = scoopc::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    // T0127: 收集 typecheck 观察到的 generic/effect 实例请求，作为后续 MIR materialization 的种子。
    #[cfg(feature = "llvm")]
    let mut all_monomorph_requests: Vec<scoopc::monomorph::MonomorphRequest> = Vec::new();

    // typecheck phase：逐文件执行（共享 env/index/types）。
    #[cfg(feature = "llvm")]
    let file_iter = input
        .sources
        .iter()
        .zip(asts.iter())
        .zip(headers.iter())
        .enumerate();
    #[cfg(not(feature = "llvm"))]
    let file_iter = input.sources.iter().zip(asts.iter()).zip(headers.iter());

    for item in file_iter {
        #[cfg(feature = "llvm")]
        let (source_index, ((source, ast), h)) = item;
        #[cfg(not(feature = "llvm"))]
        let ((source, ast), h) = item;

        scoopc::typecheck::check_file_annotations(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_properties(source, ast, &index, &env)
            .map_err(|err| miette::Report::from(*err))?;
        scoopc::typecheck::check_file_inheritance(source, ast, &index)
            .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_interfaces(source, ast, &index, &env)
            .map_err(miette::Report::from)?;
        scoopc::typecheck::check_file_override_effects(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(|err| miette::Report::from(*err))?;

        scoopc::typecheck::check_file_type_refs(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_where_clauses(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        scoopc::typecheck::check_file_overload_conflicts(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        // T0127/T5000: 只有 request roots 贡献初始 monomorph seeds；stdlib/sysroot
        // support sources 仍完整 typecheck/lower，但不把内部泛型调用提升为实例根。
        #[cfg(feature = "llvm")]
        {
            if input.is_mir_request_source_index(source_index) {
                let requests = scoopc::typecheck::check_file_exprs_with_monomorph_requests(
                    source, ast, &index, &h.imports, &env, &mut types, builtins,
                )
                .map_err(miette::Report::from)?;
                all_monomorph_requests.extend(requests);
            } else {
                scoopc::typecheck::check_file_exprs(
                    source, ast, &index, &h.imports, &env, &mut types, builtins,
                )
                .map_err(miette::Report::from)?;
            }
        }
        #[cfg(not(feature = "llvm"))]
        {
            scoopc::typecheck::check_file_exprs(
                source, ast, &index, &h.imports, &env, &mut types, builtins,
            )
            .map_err(miette::Report::from)?;
        }
    }

    // 对整个编译单元中出现过的类型做一次 layout/metadata 计算（与 fixtures/typecheck_multi 对齐）。
    scoopc::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::from)?;

    Ok(FrontendOutput {
        input,
        #[cfg(feature = "llvm")]
        asts,
        #[cfg(feature = "llvm")]
        index,
        #[cfg(feature = "llvm")]
        monomorph_requests: all_monomorph_requests,
        #[cfg(feature = "llvm")]
        typecheck_types: types,
        #[cfg(feature = "llvm")]
        type_env: env,
    })
}

fn package_prefix(
    source: &scoopc::source::SourceFile,
    pkg: Option<&scoopc::ast::PackageDecl>,
) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

fn cone_entry_main_fqn(entry_package: &str) -> String {
    if entry_package.is_empty() {
        "main".to_string()
    } else {
        format!("{entry_package}.main")
    }
}

fn find_consumer_package_decl_span(
    input: &BuildInput,
    asts: &[scoopc::ast::File],
    entry_package: &str,
) -> miette::SourceSpan {
    let Some(root) = input.cone_root.as_ref() else {
        return scoopc::span::Span::new(0, 0).into();
    };

    for (source, file) in input.sources.iter().zip(asts.iter()) {
        if !source.path().starts_with(root) {
            continue;
        }
        let Some(pkg) = file.package.as_ref() else {
            continue;
        };
        if package_prefix(source, Some(pkg)) == entry_package {
            return pkg.span.into();
        }
    }

    // fallback：锚点 main 文件的 package（如果存在）。
    if let Some(anchor) = input.cone_anchor_main_index
        && let Some(pkg) = asts.get(anchor).and_then(|file| file.package.as_ref())
    {
        return pkg.span.into();
    }

    scoopc::span::Span::new(0, 0).into()
}

fn select_cone_entry_main(
    input: &mut BuildInput,
    asts: &[scoopc::ast::File],
    index: &mut scoopc::resolve::Index,
) -> Result<()> {
    let Some(manifest) = input.cone_manifest.as_ref() else {
        return Ok(());
    };

    let entry_package = if let Some(v) = input.entry_package_override.as_deref() {
        v.trim().to_string()
    } else if let Some(v) = manifest.native_build.entry_package.as_deref() {
        v.trim().to_string()
    } else {
        let anchor = input.cone_anchor_main_index.unwrap_or(input.main_index);
        let anchor_source = &input.sources[anchor];
        let anchor_file = &asts[anchor];
        package_prefix(anchor_source, anchor_file.package.as_ref())
    };

    let entry_main_fqn = cone_entry_main_fqn(&entry_package);
    index.set_runtime_entry_point(entry_main_fqn.clone());
    input.entry_main_fqn = Some(entry_main_fqn.clone());

    // 约定（与本模块 build pipeline 对齐）：
    // - cone 0：sysroot
    // - cone 1：consumer（当前被 build 的 cone）
    // - cone 2+：依赖 cones
    let consumer_cone = scoopc::resolve::ConeId::new(1);

    let overload_in_consumer = index.by_fqn.get(&entry_main_fqn).and_then(|syms| {
        syms.fun.iter().find(|o| {
            o.symbol.decl_cone == consumer_cone
                && o.sig.receiver.is_none()
                && o.sig.kind == scoopc::ast::FunDeclKind::Regular
        })
    });

    if let Some(overload) = overload_in_consumer {
        let decl_file = overload.symbol.decl_file.as_path();
        let Some((idx, _)) = input
            .sources
            .iter()
            .enumerate()
            .find(|(_idx, s)| s.path() == decl_file)
        else {
            return Err(miette::miette!(
                "内部错误：入口 main 源文件未在 sources 列表中：{}",
                decl_file.display()
            ));
        };

        input.main_index = idx;
        return Ok(());
    }

    if let Some(syms) = index.by_fqn.get(&entry_main_fqn)
        && let Some(overload) = syms.fun.first()
        && overload.symbol.decl_cone != consumer_cone
    {
        return Err(EntryPackageMainNotInConsumerCone {
            entry_package,
            decl_file: overload.symbol.decl_file.display().to_string(),
        }
        .into());
    }

    let span = find_consumer_package_decl_span(input, asts, &entry_package);
    Err(EntryPackageMissingMain {
        entry_package,
        span,
    }
    .into())
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
fn emit_refactor_llvm_artifact_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    output: &Path,
    opt_level: OptLevel,
    artifact: scoopc::effect_refactor_pipeline::LlvmArtifactKind,
) -> Result<Vec<String>> {
    // P6-T05 handoff：`build --emit-*`、`run`（通过 executable build）和 build fixtures
    // 都必须经由同一 refactor LLVM stage helper，避免为某个产物种类保留测试专用语义入口。
    let lowered = lower_main_hir_for_build(session, front, opt_level)?;
    let extern_libs = lowered.extern_libs.clone();
    let abi_visibility_lowered =
        refactor_abi_visibility_lowered_hir_for_build(session, front, opt_level)?;
    let (source_map, entry_source_id) = build_codegen_source_map(session, front);
    scoopc::effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
        session,
        &source_map,
        entry_source_id,
        lowered,
        abi_visibility_lowered,
        output,
        front.input.entry_main_fqn.as_deref(),
        opt_level,
        artifact,
    )?;
    Ok(extern_libs)
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
    let is_cone = front.input.cone_root.is_some() && front.input.cone_manifest.is_some();

    let (work_dir, keep_work_dir) = if is_cone {
        let root = front
            .input
            .cone_root
            .as_ref()
            .ok_or_else(|| miette::miette!("内部错误：cone build 缺少 cone_root"))?;
        let dir = layout::cone_obj_dir(root, None, profile.as_str());
        std::fs::create_dir_all(&dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("无法创建 build obj 目录：{}", dir.display()))?;
        (dir, true)
    } else {
        (super::temp::make_temp_dir("scoop_build")?, false)
    };

    let obj = work_dir.join(layout::obj_file_name("main"));

    let extern_libs = emit_refactor_llvm_artifact_for_build(
        session,
        front,
        &obj,
        opt_level,
        scoopc::effect_refactor_pipeline::LlvmArtifactKind::Object,
    )?;

    // T1115：cone native build 的 `c-sources/c-flags`：
    // - 额外把用户声明的 C 源文件编译成 `.o`；
    // - `c-flags` 仅作用于这些 user sources（不影响 runtime/c 的编译选项）。
    let mut extra_objs: Vec<PathBuf> = Vec::new();
    let mut use_cxx_linker_driver = false;
    if let (Some(root), Some(manifest)) = (
        front.input.cone_root.as_ref(),
        front.input.cone_manifest.as_ref(),
    ) {
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
        .input
        .cone_manifest
        .as_ref()
        .and_then(|m| m.native_build.linker.as_deref());
    if use_cxx_linker_driver && linker.is_none() {
        // 默认策略（v0）：仅在用户启用 `cxx-sources` 时才切换到 C++ driver，
        // 以避免在纯 C/纯 Scoop 场景引入额外工具链依赖。
        linker = Some("clang++");
    }
    let options = crate::toolchain::LinkOptions {
        linker,
        link_flags: front
            .input
            .cone_manifest
            .as_ref()
            .map(|m| m.native_build.link_flags.as_slice())
            .unwrap_or(&[]),
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

#[cfg(feature = "llvm")]
#[derive(Clone, Copy)]
enum BuildMirRequestRootMode {
    EntryMain,
    RequestSources,
}

#[cfg(feature = "llvm")]
fn lower_hir_for_build_with_request_root_mode(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
    request_root_mode: BuildMirRequestRootMode,
) -> Result<scoopc::hir::LoweredHir> {
    // 该返回值仍是当前 LLVM codegen 消费的 HIR 兼容输入，但在 via-MIR 主路径上会额外挂住
    // `LoweredHir::materialized_pass_view()`，把 canonical materialized body / summary /
    // 后续 MIR pass 产物视图显式保留在 build frontend 产物里，避免 production 入口只能停留在
    // dump/test 路径。
    // compilation unit：sysroot + 当前 cone 全部源文件（稳定顺序）。
    let mut unit: Vec<(&scoopc::source::SourceFile, &scoopc::ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        unit.push((&f.source, &f.ast));
    }
    for (source, ast) in front.input.sources.iter().zip(front.asts.iter()) {
        unit.push((source, ast));
    }

    // T1315a：stdlib 注入后需要 multi-file lowering（否则 stdlib 顶层函数不会出现在 fun_index 中）。
    let files_to_lower = front
        .input
        .sources
        .iter()
        .zip(front.asts.iter())
        .collect::<Vec<_>>();

    let request_source_paths = front.input.mir_request_source_paths();
    let entry_main_fqn = front.input.entry_main_fqn.clone().unwrap_or_else(|| {
        let source = front.input.main_source();
        let ast = &front.asts[front.input.main_index];
        cone_entry_main_fqn(&package_prefix(source, ast.package.as_ref()))
    });

    let request_root_mode = match request_root_mode {
        BuildMirRequestRootMode::EntryMain => scoopc::mir::MaterializeRequestRootMode::EntryMain {
            fqn: Some(entry_main_fqn.as_str()),
        },
        BuildMirRequestRootMode::RequestSources => {
            scoopc::mir::MaterializeRequestRootMode::RequestSources
        }
    };

    scoopc::hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
        &front.index,
        &unit,
        &files_to_lower,
        &front.monomorph_requests,
        Some(&front.type_env),
        &front.typecheck_types,
        scoopc::hir::MirInstanceCollectionOptions {
            request_source_paths: &request_source_paths,
            request_root_mode,
            opt_level,
        },
    )
    .map_err(|err| miette::Report::from(*err))
}

#[cfg(feature = "llvm")]
fn lower_main_hir_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
) -> Result<scoopc::hir::LoweredHir> {
    lower_hir_for_build_with_request_root_mode(
        session,
        front,
        opt_level,
        BuildMirRequestRootMode::EntryMain,
    )
}

#[cfg(feature = "llvm")]
fn refactor_abi_visibility_lowered_hir_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
) -> Result<Option<scoopc::hir::LoweredHir>> {
    if session.effect_pipeline_mode() != scoopc::session::EffectPipelineMode::Refactor {
        return Ok(None);
    }

    // 这条附加 handoff 只负责把 request-source 范围内的 callable ABI shell 暴露给 refactor
    // LLVM stage；真正的 reachable body lowering 仍由 entry-main rooted build lowering 决定。
    lower_hir_for_build_with_request_root_mode(
        session,
        front,
        opt_level,
        BuildMirRequestRootMode::RequestSources,
    )
    .map(Some)
}

#[cfg(feature = "llvm")]
fn build_codegen_source_map(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
) -> (scoopc::source::SourceMap, scoopc::source::SourceId) {
    let mut source_map = scoopc::source::SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }

    let mut entry_source_id = None;
    for (idx, source) in front.input.sources.iter().enumerate() {
        let source_id = source_map.add_source_clone(source);
        if idx == front.input.main_index {
            entry_source_id = Some(source_id);
        }
    }

    (
        source_map,
        entry_source_id.expect("main source should always be present in build source map"),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[cfg(feature = "llvm")]
    use scoopc::effect_refactor_pipeline::LlvmArtifactKind;
    use scoopc::opt::OptLevel;
    use tempfile::tempdir;

    #[cfg(feature = "llvm")]
    fn refactor_session() -> scoopc::session::Session {
        use scoopc::session::{EffectPipelineMode, SessionOptions};

        scoopc::session::Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor))
            .unwrap()
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
    fn build_frontend_single_file_request_roots_exclude_stdlib_support_sources() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        std::fs::write(&input, "fun main() {}\n").unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();

        assert_eq!(
            front.input.mir_request_source_paths(),
            vec![input.clone()],
            "单文件 build 只能让用户入口源贡献 MIR request roots"
        );
        assert!(
            front.monomorph_requests.is_empty(),
            "不含泛型调用的单文件入口不应因为 stdlib/sysroot support sources 产生初始 monomorph seeds: {:?}",
            front.monomorph_requests
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
        let build_input = super::load_build_input(&pkg, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let cone_root = pkg.canonicalize().unwrap();
        let roots = front.input.mir_request_source_paths();

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
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        assert!(
            front
                .monomorph_requests
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
        let build_input = super::load_build_input(&pkg, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        assert!(
            front.input.mir_request_source_paths().len() >= 2,
            "cone build 仍应把 consumer cone sources 作为 request-source 过滤集合"
        );
        assert!(
            front
                .monomorph_requests
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
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
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
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
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
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let (source_map, entry_source_id) = super::build_codegen_source_map(&session, &front);
        let ir = scoopc::llvm::emit_minimal_main_ir_from_production_lowered_hir(
            &source_map,
            entry_source_id,
            &lowered,
        )
        .expect("build frontend 的 production codegen 入口应显式消费 materialized pass view");

        for fqn in [
            "fixtures.t5000h0c.id::<Int>",
            "fixtures.t5000h0c.Box.memberId::<Int>",
        ] {
            assert!(
                ir.contains(fqn),
                "build production codegen 入口应继续保留实例身份 `{fqn}`，实际 IR:\n{ir}"
            );
        }
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn build_refactor_task_atomic_fixture_lowers_o0_without_legacy_mutex() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("task_atomic_claim_no_mutex_llvm.scoop");
        std::fs::write(
            &input,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/build/task_atomic_claim_no_mutex_llvm.scoop"
            )),
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let output = dir.path().join("task_atomic.ll");
        let _extern_libs = super::emit_refactor_llvm_artifact_for_build(
            &session,
            &front,
            &output,
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .unwrap();
        let ir = std::fs::read_to_string(output).unwrap();

        assert!(
            ir.contains("cmpxchg ptr addrspace(1)"),
            "Task.step() O0 build should preserve atomic claim acquisition\n{ir}"
        );
        assert!(
            ir.contains("store atomic i64 0, ptr addrspace(1)"),
            "Task.step() O0 build should preserve atomic claim release\n{ir}"
        );
        assert!(
            !ir.contains("scoop_sync_mutex"),
            "Task.step() O0 build must not fall back to legacy mutex transport\n{ir}"
        );
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
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
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
    fn refactor_build_publishes_request_source_abi_shells_for_unreachable_effectful_helpers() {
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

        let session = refactor_session();
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let abi_visibility_lowered =
            super::refactor_abi_visibility_lowered_hir_for_build(&session, &front, OptLevel::O0)
                .unwrap()
                .expect("refactor build 应额外构造 request-source ABI visibility handoff");
        let (source_map, entry_source_id) = super::build_codegen_source_map(&session, &front);

        scoopc::effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            Some(abi_visibility_lowered),
            &out,
            front.input.entry_main_fqn.as_deref(),
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .unwrap();

        let ir = std::fs::read_to_string(&out).unwrap();
        assert!(
            ir.contains(
                "__scoop_refactor_dynamic_invoke__fixtures_build_abi_visibility_hiddenWorker"
            ),
            "ABI visibility handoff 应让不可达 effectful helper 的 canonical invoke shell 出现在 refactor build IR 中：\n{ir}"
        );
        assert!(
            !ir.contains("scoop.effect.frame."),
            "纯 main 的 refactor build 不应为了 ABI shell 可见性而偷偷生成 legacy effect frame IR：\n{ir}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn refactor_build_lowers_reachable_self_contained_effect_body_without_legacy_frames() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        let out = dir.path().join("refactor.ll");

        write_reachable_legacy_effect_fixture(&input);

        let session = refactor_session();
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front, OptLevel::O0).unwrap();
        let abi_visibility_lowered =
            super::refactor_abi_visibility_lowered_hir_for_build(&session, &front, OptLevel::O0)
                .unwrap();
        let (source_map, entry_source_id) = super::build_codegen_source_map(&session, &front);

        scoopc::effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            abi_visibility_lowered,
            &out,
            front.input.entry_main_fqn.as_deref(),
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .expect("reachable self-contained handle 应由 refactor lowering 正常生成 IR");

        let ir = std::fs::read_to_string(&out).unwrap();
        assert!(
            ir.contains("__scoop_refactor"),
            "refactor IR 应包含 canonical refactor symbol，而不是空壳输出：\n{ir}"
        );
        assert!(
            !ir.contains("scoop_effect_handler_stack") && !ir.contains("scoop_effect_outcome"),
            "refactor IR 不应回落到 legacy handler-stack/outcome runtime：\n{ir}"
        );
    }

    #[cfg(feature = "llvm")]
    #[test]
    fn no_hidden_legacy_fallback_for_default_refactor_build_output() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("main.scoop");
        let out = dir.path().join("default_refactor.ll");

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
        .expect("default refactor build should lower without hidden legacy fallback");
        let ir = std::fs::read_to_string(&out).unwrap();

        assert!(
            ir.contains("__scoop_refactor"),
            "default build should emit refactor-owned symbols:\n{ir}"
        );
        assert!(
            !ir.contains("scoop_effect_handler_stack") && !ir.contains("scoop_effect_outcome"),
            "default build must not retry or embed legacy handler-stack/outcome lowering:\n{ir}"
        );
    }
}
