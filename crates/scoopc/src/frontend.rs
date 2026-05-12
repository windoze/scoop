use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use thiserror::Error;

use crate::ast;
use crate::cone::{ConeArchiveApi, ConeManifest, ConeNativeBuildConfig, ConeSection};
use crate::opt::OptLevel;
use crate::resolve::{ConeId, Index, IndexedFile};
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::ty::TypeStore;
use crate::typecheck::TypeEnv;

#[cfg(feature = "llvm")]
use crate::hir;
#[cfg(feature = "llvm")]
use crate::monomorph::MonomorphRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConeProjectKind {
    Explicit,
    Virtual,
}

#[derive(Debug, Clone)]
pub struct ProjectInput {
    /// 当前编译单元的全部源文件：support sources + 当前 project sources。
    sources: Vec<SourceFile>,
    /// 当前 project（consumer cone）自身的源文件在 `sources` 中的下标。
    project_source_indices: Vec<usize>,
    /// 当前运行入口 `fun main` 所在源文件在 `sources` 中的下标。
    main_index: usize,
    /// 当前 project 的锚点 main 文件下标。
    ///
    /// - 显式 cone：`src/main.scoop`
    /// - virtual cone：唯一的用户源文件
    cone_anchor_main_index: usize,
    project_kind: ConeProjectKind,
    cone_root: PathBuf,
    cone_manifest: ConeManifest,
    entry_package_override: Option<String>,
    entry_main_fqn: Option<String>,
}

impl ProjectInput {
    fn new_explicit(
        sources: Vec<SourceFile>,
        project_source_indices: Vec<usize>,
        main_index: usize,
        cone_root: PathBuf,
        cone_manifest: ConeManifest,
        entry_package_override: Option<String>,
    ) -> Self {
        Self {
            sources,
            project_source_indices,
            main_index,
            cone_anchor_main_index: main_index,
            project_kind: ConeProjectKind::Explicit,
            cone_root,
            cone_manifest,
            entry_package_override,
            entry_main_fqn: None,
        }
    }

    fn new_virtual(
        sources: Vec<SourceFile>,
        main_index: usize,
        cone_root: PathBuf,
        cone_manifest: ConeManifest,
    ) -> Self {
        Self {
            sources,
            project_source_indices: vec![main_index],
            main_index,
            cone_anchor_main_index: main_index,
            project_kind: ConeProjectKind::Virtual,
            cone_root,
            cone_manifest,
            entry_package_override: None,
            entry_main_fqn: None,
        }
    }

    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    pub fn main_source(&self) -> &SourceFile {
        &self.sources[self.main_index]
    }

    pub fn main_index(&self) -> usize {
        self.main_index
    }

    pub fn cone_anchor_main_index(&self) -> usize {
        self.cone_anchor_main_index
    }

    pub fn cone_root(&self) -> &Path {
        &self.cone_root
    }

    pub fn cone_manifest(&self) -> &ConeManifest {
        &self.cone_manifest
    }

    pub fn entry_main_fqn(&self) -> Option<&str> {
        self.entry_main_fqn.as_deref()
    }

    pub fn is_explicit_cone(&self) -> bool {
        self.project_kind == ConeProjectKind::Explicit
    }

    pub fn is_virtual_cone(&self) -> bool {
        self.project_kind == ConeProjectKind::Virtual
    }

    #[cfg(feature = "llvm")]
    pub fn mir_request_source_paths(&self) -> Vec<PathBuf> {
        self.project_source_indices
            .iter()
            .copied()
            .map(|idx| self.sources[idx].path().to_path_buf())
            .collect()
    }
}

/// `scoop` -> `scoopc` 的 authoritative project context。
///
/// 约定：
/// - 裸 `SourceFile` / `<file>` 只表达 single-source virtual cone；
/// - 若目标语义是显式 cone / 多源 project，则上层驱动必须先构造完整的 project context，
///   再把它交给 `run_project_frontend`；
/// - `scoopc` 不应在末端通过工作目录、相邻 `Cone.toml` 或其它环境线索，从单个源码路径
///   隐式恢复 explicit-cone 语义。
#[derive(Debug, Clone)]
pub struct ProjectContext {
    input: ProjectInput,
    deps: Vec<ConeArchiveApi>,
}

impl ProjectContext {
    pub fn new(input: ProjectInput, deps: Vec<ConeArchiveApi>) -> Self {
        Self { input, deps }
    }

    pub fn input(&self) -> &ProjectInput {
        &self.input
    }

    pub fn deps(&self) -> &[ConeArchiveApi] {
        &self.deps
    }

    pub fn into_parts(self) -> (ProjectInput, Vec<ConeArchiveApi>) {
        (self.input, self.deps)
    }
}

#[derive(Debug)]
pub struct FrontendOutput {
    input: ProjectInput,
    #[cfg(feature = "llvm")]
    asts: Vec<ast::File>,
    #[cfg(feature = "llvm")]
    index: Index,
    #[cfg(feature = "llvm")]
    monomorph_requests: Vec<MonomorphRequest>,
    #[cfg(feature = "llvm")]
    typecheck_types: TypeStore,
    #[cfg(feature = "llvm")]
    type_env: TypeEnv,
}

impl FrontendOutput {
    fn new(
        input: ProjectInput,
        #[cfg(feature = "llvm")] asts: Vec<ast::File>,
        #[cfg(feature = "llvm")] index: Index,
        #[cfg(feature = "llvm")] monomorph_requests: Vec<MonomorphRequest>,
        #[cfg(feature = "llvm")] typecheck_types: TypeStore,
        #[cfg(feature = "llvm")] type_env: TypeEnv,
    ) -> Self {
        Self {
            input,
            #[cfg(feature = "llvm")]
            asts,
            #[cfg(feature = "llvm")]
            index,
            #[cfg(feature = "llvm")]
            monomorph_requests,
            #[cfg(feature = "llvm")]
            typecheck_types,
            #[cfg(feature = "llvm")]
            type_env,
        }
    }

    pub fn input(&self) -> &ProjectInput {
        &self.input
    }

    pub fn main_source(&self) -> &SourceFile {
        self.input.main_source()
    }

    #[cfg(feature = "llvm")]
    pub fn asts(&self) -> &[ast::File] {
        &self.asts
    }

    #[cfg(feature = "llvm")]
    pub fn index(&self) -> &Index {
        &self.index
    }

    #[cfg(feature = "llvm")]
    pub fn monomorph_requests(&self) -> &[MonomorphRequest] {
        &self.monomorph_requests
    }

    #[cfg(feature = "llvm")]
    pub fn typecheck_types(&self) -> &TypeStore {
        &self.typecheck_types
    }

    #[cfg(feature = "llvm")]
    pub fn type_env(&self) -> &TypeEnv {
        &self.type_env
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("入口包 `{entry_package}` 中找不到入口函数 `fun main`")]
#[diagnostic(code(scoop::driver::entry_package_missing_main))]
pub struct EntryPackageMissingMain {
    pub entry_package: String,
    #[label("该 package 没有 `main`")]
    pub span: miette::SourceSpan,
}

#[derive(Debug, Error, Diagnostic)]
#[error(
    "入口包 `{entry_package}` 的 `fun main` 不属于 consumer cone（它声明在依赖/其它 cone：{decl_file}）"
)]
#[diagnostic(code(scoop::driver::entry_package_main_not_in_consumer_cone))]
pub struct EntryPackageMainNotInConsumerCone {
    pub entry_package: String,
    pub decl_file: String,
}

#[cfg(feature = "llvm")]
#[derive(Clone, Copy)]
pub enum MirRequestRootMode {
    EntryMain,
    RequestSources,
}

pub fn load_project_input_from_path(
    input: &Path,
    entry_package_override: Option<String>,
) -> Result<ProjectInput> {
    let support_sources = load_default_support_sources()?;

    if input.is_file() {
        let mut sources = support_sources;
        let main_index = sources.len();
        let source = SourceFile::load(input)?;
        let virtual_root = source.path().to_path_buf();
        let manifest = default_virtual_cone_manifest(&source);
        sources.push(source);
        return Ok(ProjectInput::new_virtual(
            sources,
            main_index,
            virtual_root,
            manifest,
        ));
    }

    if input.is_dir() {
        let pkg = crate::cone::load_cone_source_package(input)?;
        let mut sources = support_sources;
        let support_len = sources.len();
        sources.reserve(pkg.sources.len());
        let mut main_index = None;
        for (idx, path) in pkg.sources.iter().enumerate() {
            let source = SourceFile::load(path)?;
            if source.path() == pkg.main.as_path() {
                main_index = Some(support_len + idx);
            }
            sources.push(source);
        }

        let main_index = main_index.ok_or_else(|| {
            miette::miette!(
                "cone package 的 main 未出现在 sources 列表中：{}",
                pkg.main.display()
            )
        })?;
        let project_source_indices = (support_len..sources.len()).collect::<Vec<_>>();
        return Ok(ProjectInput::new_explicit(
            sources,
            project_source_indices,
            main_index,
            pkg.root,
            pkg.manifest,
            entry_package_override,
        ));
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

pub fn prepare_virtual_cone_input(source: SourceFile) -> Result<ProjectInput> {
    let mut sources = load_default_support_sources()?;
    let main_index = sources.len();
    let virtual_root = source.path().to_path_buf();
    let manifest = default_virtual_cone_manifest(&source);
    sources.push(source);
    Ok(ProjectInput::new_virtual(
        sources,
        main_index,
        virtual_root,
        manifest,
    ))
}

pub fn prepare_virtual_cone_context(source: SourceFile) -> Result<ProjectContext> {
    prepare_virtual_cone_input(source).map(|input| ProjectContext::new(input, Vec::new()))
}

pub fn run_project_frontend(session: &Session, context: ProjectContext) -> Result<FrontendOutput> {
    let (input, deps) = context.into_parts();
    run_frontend(session, input, &deps)
}

pub fn run_frontend(
    session: &Session,
    mut input: ProjectInput,
    deps: &[ConeArchiveApi],
) -> Result<FrontendOutput> {
    if input.sources.is_empty() {
        return Err(miette::miette!("内部错误：frontend 输入 sources 为空"));
    }

    let mut asts = Vec::with_capacity(input.sources.len());
    for source in &input.sources {
        let ast = crate::pipeline::load_ast_stage_output_for_dump(session, source)
            .map(crate::pipeline::AstStageOutput::into_ast)
            .map_err(miette::Report::from)?;
        asts.push(ast);
    }

    {
        let source_refs = input.sources.iter().collect::<Vec<_>>();
        let mut ast_refs = asts.iter_mut().collect::<Vec<_>>();
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )
        .map_err(miette::Report::from)?;
    }

    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        crate::typecheck::check_file_headers(source, ast).map_err(miette::Report::from)?;
        crate::typecheck::check_file_struct_decls(source, ast).map_err(miette::Report::from)?;
    }

    let mut indexed: Vec<IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        indexed.push(IndexedFile {
            cone: ConeId::new(0),
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        indexed.push(IndexedFile {
            cone: ConeId::new(1),
            source,
            file: ast,
        });
    }

    let mut index = Index::build_with_cones(&indexed).map_err(miette::Report::from)?;
    index.set_export_entry_points(input.cone_manifest.export_entry_points.clone());

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).map_err(miette::Report::from)?;
    for (next_dep_cone, dep) in (2_u32..).zip(deps.iter()) {
        let dep_cone = ConeId::new(next_dep_cone);
        crate::cone::inject_cone_dependency_public_api(&mut index, &mut env, dep_cone, dep)?;
    }

    select_cone_entry_main(&mut input, &asts, &mut index)?;

    let mut headers = Vec::with_capacity(input.sources.len());
    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        let h = crate::resolve::check_file_headers(source, ast, &index)
            .map_err(miette::Report::from)?;
        headers.push(h);
    }
    for ((source, ast), h) in input
        .sources
        .iter()
        .zip(asts.iter_mut())
        .zip(headers.iter())
    {
        crate::resolve::check_file_bodies(source, ast, &index, h).map_err(miette::Report::from)?;
    }

    for (source, ast) in input.sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    #[cfg(feature = "llvm")]
    let mut all_monomorph_requests: Vec<MonomorphRequest> = Vec::new();

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

        crate::typecheck::check_file_annotations(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;
        crate::typecheck::check_file_properties(source, ast, &index, &env)
            .map_err(|err| miette::Report::from(*err))?;
        crate::typecheck::check_file_inheritance(source, ast, &index)
            .map_err(miette::Report::from)?;
        crate::typecheck::check_file_interfaces(source, ast, &index, &env)
            .map_err(miette::Report::from)?;
        crate::typecheck::check_file_override_effects(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(|err| miette::Report::from(*err))?;
        crate::typecheck::check_file_type_refs(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;
        crate::typecheck::check_file_where_clauses(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;
        crate::typecheck::check_file_overload_conflicts(
            source, ast, &index, &h.imports, &env, &mut types, builtins,
        )
        .map_err(miette::Report::from)?;

        #[cfg(feature = "llvm")]
        {
            if input.project_source_indices.contains(&source_index) {
                let requests = crate::typecheck::check_file_exprs_with_monomorph_requests(
                    source, ast, &index, &h.imports, &env, &mut types, builtins,
                )
                .map_err(miette::Report::from)?;
                all_monomorph_requests.extend(requests);
            } else {
                crate::typecheck::check_file_exprs(
                    source, ast, &index, &h.imports, &env, &mut types, builtins,
                )
                .map_err(miette::Report::from)?;
            }
        }
        #[cfg(not(feature = "llvm"))]
        {
            crate::typecheck::check_file_exprs(
                source, ast, &index, &h.imports, &env, &mut types, builtins,
            )
            .map_err(miette::Report::from)?;
        }
    }

    crate::typecheck::check_file_type_layouts(&index, &env, &mut types, builtins)
        .map_err(miette::Report::from)?;

    Ok(FrontendOutput::new(
        input,
        #[cfg(feature = "llvm")]
        asts,
        #[cfg(feature = "llvm")]
        index,
        #[cfg(feature = "llvm")]
        all_monomorph_requests,
        #[cfg(feature = "llvm")]
        types,
        #[cfg(feature = "llvm")]
        env,
    ))
}

#[cfg(feature = "llvm")]
pub fn lower_hir_for_codegen_with_request_root_mode(
    session: &Session,
    front: &FrontendOutput,
    opt_level: OptLevel,
    request_root_mode: MirRequestRootMode,
) -> Result<hir::LoweredHir> {
    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        unit.push((&f.source, &f.ast));
    }
    for (source, ast) in front.input.sources.iter().zip(front.asts.iter()) {
        unit.push((source, ast));
    }

    let files_to_lower = front
        .input
        .sources
        .iter()
        .zip(front.asts.iter())
        .collect::<Vec<_>>();
    let request_source_paths = front.input.mir_request_source_paths();
    let stable_cone_key =
        crate::stable_id::StableConeKey::from_manifest(front.input.cone_manifest());
    let entry_main_fqn = front.input.entry_main_fqn.clone().unwrap_or_else(|| {
        let source = front.input.main_source();
        let ast = &front.asts[front.input.main_index];
        cone_entry_main_fqn(&package_prefix(source, ast.package.as_ref()))
    });
    let request_root_mode = match request_root_mode {
        MirRequestRootMode::EntryMain => crate::mir::MaterializeRequestRootMode::EntryMain {
            fqn: Some(entry_main_fqn.as_str()),
        },
        MirRequestRootMode::RequestSources => {
            crate::mir::MaterializeRequestRootMode::RequestSources
        }
    };

    hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
        &front.index,
        &unit,
        &files_to_lower,
        &front.monomorph_requests,
        Some(&front.type_env),
        &front.typecheck_types,
        hir::MirInstanceCollectionOptions {
            stable_cone_key,
            request_source_paths: &request_source_paths,
            request_root_mode,
            opt_level,
        },
    )
    .map_err(|err| miette::Report::from(*err))
}

pub fn build_source_map(session: &Session, input: &ProjectInput) -> (SourceMap, SourceId) {
    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        let _ = source_map.add_source_clone(&file.source);
    }

    let mut entry_source_id = None;
    for (idx, source) in input.sources.iter().enumerate() {
        let source_id = source_map.add_source_clone(source);
        if idx == input.main_index {
            entry_source_id = Some(source_id);
        }
    }

    (
        source_map,
        entry_source_id.expect("main source should always be present in source map"),
    )
}

fn default_virtual_cone_manifest(source: &SourceFile) -> ConeManifest {
    let name = source
        .path()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("virtual-cone")
        .to_string();
    ConeManifest {
        cone: ConeSection {
            name,
            version: "0.0.0".to_string(),
        },
        dependencies: Default::default(),
        pre_specialize_functions: Vec::new(),
        pre_specialize_types: Vec::new(),
        export_entry_points: Vec::new(),
        selectors: Vec::new(),
        native_build: ConeNativeBuildConfig::default(),
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
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
    input: &ProjectInput,
    asts: &[ast::File],
    entry_package: &str,
) -> miette::SourceSpan {
    for (source, file) in input.sources.iter().zip(asts.iter()) {
        if !source.path().starts_with(&input.cone_root) {
            continue;
        }
        let Some(pkg) = file.package.as_ref() else {
            continue;
        };
        if package_prefix(source, Some(pkg)) == entry_package {
            return pkg.span.into();
        }
    }

    if let Some(pkg) = asts
        .get(input.cone_anchor_main_index)
        .and_then(|file| file.package.as_ref())
    {
        return pkg.span.into();
    }

    crate::span::Span::new(0, 0).into()
}

fn select_cone_entry_main(
    input: &mut ProjectInput,
    asts: &[ast::File],
    index: &mut Index,
) -> Result<()> {
    let entry_package = if let Some(v) = input.entry_package_override.as_deref() {
        v.trim().to_string()
    } else if let Some(v) = input.cone_manifest.native_build.entry_package.as_deref() {
        v.trim().to_string()
    } else {
        let anchor_source = &input.sources[input.cone_anchor_main_index];
        let anchor_file = &asts[input.cone_anchor_main_index];
        package_prefix(anchor_source, anchor_file.package.as_ref())
    };

    let entry_main_fqn = cone_entry_main_fqn(&entry_package);
    index.set_runtime_entry_point(entry_main_fqn.clone());
    input.entry_main_fqn = Some(entry_main_fqn.clone());

    let consumer_cone = ConeId::new(1);

    let overload_in_consumer = index.by_fqn.get(&entry_main_fqn).and_then(|syms| {
        syms.fun.iter().find(|o| {
            o.symbol.decl_cone == consumer_cone
                && o.sig.receiver.is_none()
                && o.sig.kind == ast::FunDeclKind::Regular
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

fn default_stdlib_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn load_default_support_sources() -> Result<Vec<SourceFile>> {
    let root = default_stdlib_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 stdlib 目录（T1315a）")?;

    let mut paths = Vec::new();
    collect_scoop_files(&root, &mut paths)?;

    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（T0143）")?;
    crate::sysroot::collect_compilable_sysroot_files(&sysroot_root, &mut paths)?;

    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        out.push(SourceFile::load(&path)?);
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
