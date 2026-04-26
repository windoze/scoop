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
    /// T0127: typecheck 收集到的实例请求种子。
    ///
    /// 当前 build/frontend 主路径会把它交给 MIR materializer 建立 `InstanceKey` 集，
    /// 而不是回到 HIR eager lowering 现场扫描并克隆具体实例。
    #[cfg(feature = "llvm")]
    monomorph_keys: Vec<scoopc::monomorph::MonomorphKey>,
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
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            emit: BuildEmit::Executable,
            entry_package: None,
            profile: BuildProfile::Debug,
            opt_level: None,
            incremental: true,
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

    let session = scoopc::session::Session::new()?;

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
                let lowered = lower_main_hir_for_build(&session, &front)?;
                let (source_map, entry_source_id) = build_codegen_source_map(&session, &front);
                scoopc::llvm::emit_minimal_main_ir_to_file_from_lowered_hir_with_entry_with_opt_level(
                    &source_map,
                    entry_source_id,
                    &lowered,
                    &output,
                    front.input.entry_main_fqn.as_deref(),
                    opt_level,
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
                let lowered = lower_main_hir_for_build(&session, &front)?;
                let (source_map, entry_source_id) = build_codegen_source_map(&session, &front);
                scoopc::llvm::emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level(
                    &source_map,
                    entry_source_id,
                    &lowered,
                    &output,
                    front.input.entry_main_fqn.as_deref(),
                    opt_level,
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
                let lowered = lower_main_hir_for_build(&session, &front)?;
                let (source_map, entry_source_id) = build_codegen_source_map(&session, &front);
                scoopc::llvm::emit_minimal_main_asm_to_file_from_lowered_hir_with_entry_with_opt_level(
                    &source_map,
                    entry_source_id,
                    &lowered,
                    &output,
                    front.input.entry_main_fqn.as_deref(),
                    opt_level,
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
        let ast = scoopc::parser::parse_file(source).map_err(miette::Report::from)?;
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
    let mut next_dep_cone: u32 = 2;
    for dep in deps {
        let dep_cone = scoopc::resolve::ConeId::new(next_dep_cone);
        next_dep_cone += 1;
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
    let mut all_monomorph_keys: Vec<scoopc::monomorph::MonomorphKey> = Vec::new();

    // typecheck phase：逐文件执行（共享 env/index/types）。
    for ((source, ast), h) in input.sources.iter().zip(asts.iter()).zip(headers.iter()) {
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

        // T0127: 使用 check_file_exprs_with_monomorph_keys 收集泛型函数实例化信息。
        #[cfg(feature = "llvm")]
        {
            let keys = scoopc::typecheck::check_file_exprs_with_monomorph_keys(
                source, ast, &index, &h.imports, &env, &mut types, builtins,
            )
            .map_err(miette::Report::from)?;
            all_monomorph_keys.extend(keys);
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
        monomorph_keys: all_monomorph_keys,
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
                && o.sig.params.is_empty()
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

    let lowered = lower_main_hir_for_build(session, front)?;
    let (source_map, entry_source_id) = build_codegen_source_map(session, front);
    scoopc::llvm::emit_minimal_main_obj_to_file_from_lowered_hir_with_entry_with_opt_level(
        &source_map,
        entry_source_id,
        &lowered,
        &obj,
        front.input.entry_main_fqn.as_deref(),
        opt_level,
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
        crate::toolchain::link_objs(&objs, output, &lowered.extern_libs, options)?;
    } else {
        crate::toolchain::link_objs_with_runtime(&objs, output, &lowered.extern_libs, options)?;
    }

    if !keep_work_dir {
        let _ = std::fs::remove_dir_all(&work_dir);
    }
    Ok(())
}

#[cfg(feature = "llvm")]
fn lower_main_hir_for_build(
    session: &scoopc::session::Session,
    front: &FrontendOutput,
) -> Result<scoopc::hir::LoweredHir> {
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

    scoopc::hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection(
        &front.index,
        &unit,
        &files_to_lower,
        &front.monomorph_keys,
        Some(&front.type_env),
        &front.typecheck_types,
    )
    .map_err(|err| miette::Report::from(*err))
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

    use scoopc::opt::OptLevel;
    use tempfile::tempdir;

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

fun <T, eff E> id(x: T): T / E {
    return x
}

fun <T, eff E> wrap(x: T): T / E {
    return id<T, eff E>(x)
}

private fun entry(): Int / (Boom + Zap) {
    val a = wrap<Int, eff Boom>(1)
    val b = wrap<Int, eff Zap>(2)
    return a + b
}

fun main(): Int / Pure! {
    val thunk: () -> Int / (Boom + Zap) = entry
    return 0
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front).unwrap();
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
    val thunk: () -> Int = entry
    return 0
}
"#,
        )
        .unwrap();

        let session = scoopc::session::Session::new().unwrap();
        let build_input = super::load_build_input(&input, None).unwrap();
        let front = super::run_frontend(&session, build_input, &[]).unwrap();
        let lowered = super::lower_main_hir_for_build(&session, &front).unwrap();
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
}
