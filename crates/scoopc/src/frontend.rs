use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use thiserror::Error;

use crate::ast;
#[cfg(test)]
use crate::cone::{
    CONSUMER_CONE_ID, SourceConeDependencyEdge, SourceConeDependencyKind, SourceConeNode,
    SourceConeRole, SourceConeTrust,
};
use crate::cone::{
    ConeId, ConeInfo, ConeKind, ConeManifest, ConeNativeBuildConfig, ConeSection,
    SourceConeCompilationUnit, SourceConeGraph, SourceConeInfo,
    load_source_cone_graph_for_consumer_package, load_source_cone_graph_for_virtual_consumer,
};
#[cfg(feature = "llvm")]
use crate::opt::OptLevel;
use crate::resolve::{Index, IndexedFile};
use crate::session::{Session, SessionOptions};
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
    graph: SourceConeGraph,
    /// Build-closure source view flattened from the source-cone graph in DAG order.
    ///
    /// This is not a compilation unit. Use `compilation_units()` or
    /// `consumer_compilation_unit()` when a caller needs cone-level semantics.
    build_closure_sources: Vec<SourceFile>,
    /// `build_closure_sources` 中每个 source 的 owning cone id，与下标一一对应。
    source_cone_ids: Vec<ConeId>,
    /// `build_closure_sources` 中每个 source 的 authoritative cone metadata，与下标一一对应。
    source_cone_infos: Vec<SourceConeInfo>,
    /// consumer cone 自身的源文件在 `build_closure_sources` 中的下标。
    #[allow(dead_code)]
    consumer_source_indices: Vec<usize>,
    consumer_cone_id: ConeId,
    /// 当前运行入口 `fun main` 所在源文件在 `build_closure_sources` 中的下标。
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
    fn from_graph(
        graph: SourceConeGraph,
        project_kind: ConeProjectKind,
        entry_package_override: Option<String>,
    ) -> Result<Self> {
        let consumer = graph.consumer();
        let consumer_cone_id = graph.consumer_id();
        let cone_root = consumer.root.clone();
        let cone_manifest = consumer.manifest.clone();
        let entry_main = consumer.entry_main.as_ref().ok_or_else(|| {
            miette::miette!(
                "consumer cone `{}` 缺少入口锚点",
                consumer.manifest.cone.name
            )
        })?;

        let mut build_closure_sources = Vec::new();
        let mut source_cone_ids = Vec::new();
        let mut source_cone_infos = Vec::new();
        let mut consumer_source_indices = Vec::new();
        let mut main_index = None;

        for node in graph.nodes() {
            let cone_info = SourceConeInfo::from_node(node);
            for source in &node.sources {
                let idx = build_closure_sources.len();
                if node.id == consumer_cone_id {
                    consumer_source_indices.push(idx);
                    if source.path() == entry_main.as_path() {
                        main_index = Some(idx);
                    }
                }
                build_closure_sources.push(source.clone());
                source_cone_ids.push(node.id);
                source_cone_infos.push(cone_info.clone());
            }
        }

        let main_index = main_index.ok_or_else(|| {
            miette::miette!(
                "consumer cone 的入口锚点未出现在 graph sources 中：{}",
                entry_main.display()
            )
        })?;

        Ok(Self {
            graph,
            build_closure_sources,
            source_cone_ids,
            source_cone_infos,
            consumer_source_indices,
            main_index,
            cone_anchor_main_index: main_index,
            consumer_cone_id,
            project_kind,
            cone_root,
            cone_manifest,
            entry_package_override,
            entry_main_fqn: None,
        })
    }

    #[cfg(test)]
    fn new_explicit(
        build_closure_sources: Vec<SourceFile>,
        consumer_source_indices: Vec<usize>,
        main_index: usize,
        cone_root: PathBuf,
        cone_manifest: ConeManifest,
        entry_package_override: Option<String>,
    ) -> Self {
        let mut source_cone_ids = Vec::with_capacity(build_closure_sources.len());
        let mut source_cone_infos = Vec::with_capacity(build_closure_sources.len());
        let mut consumer_sources = Vec::new();
        let mut graph_nodes = Vec::new();
        let mut next_dep_id = 2;
        for (idx, source) in build_closure_sources.iter().enumerate() {
            if consumer_source_indices.contains(&idx) {
                source_cone_ids.push(CONSUMER_CONE_ID);
                source_cone_infos.push(SourceConeInfo {
                    id: CONSUMER_CONE_ID,
                    kind: cone_manifest.cone.kind,
                    stable_key: crate::stable_id::StableConeKey::from_manifest(&cone_manifest),
                    trust: SourceConeTrust::Untrusted,
                });
                consumer_sources.push(source.clone());
            } else {
                let dep_id = ConeId::new(next_dep_id);
                next_dep_id += 1;
                source_cone_ids.push(dep_id);
                let manifest = synthetic_manifest_for_source(source, ConeKind::Lib);
                source_cone_infos.push(SourceConeInfo {
                    id: dep_id,
                    kind: manifest.cone.kind,
                    stable_key: crate::stable_id::StableConeKey::from_manifest(&manifest),
                    trust: SourceConeTrust::Untrusted,
                });
                graph_nodes.push(SourceConeNode {
                    id: dep_id,
                    role: SourceConeRole::LocalDependency,
                    root: source
                        .path()
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| source.path().to_path_buf()),
                    manifest_path: PathBuf::new(),
                    kind: manifest.cone.kind,
                    native_build: manifest.native_build.clone(),
                    manifest,
                    trust: SourceConeTrust::Untrusted,
                    sources: vec![source.clone()],
                    entry_main: None,
                    dependencies: Vec::<SourceConeDependencyEdge>::new(),
                });
            }
        }
        graph_nodes.push(SourceConeNode {
            id: CONSUMER_CONE_ID,
            role: SourceConeRole::Consumer,
            root: cone_root.clone(),
            manifest_path: cone_root.join(crate::cone::CONE_TOML_FILE_NAME),
            kind: cone_manifest.cone.kind,
            native_build: cone_manifest.native_build.clone(),
            manifest: cone_manifest.clone(),
            trust: SourceConeTrust::Untrusted,
            sources: consumer_sources,
            entry_main: build_closure_sources
                .get(main_index)
                .map(|source| source.path().to_path_buf()),
            dependencies: Vec::new(),
        });
        let graph = SourceConeGraph::from_nodes(graph_nodes, CONSUMER_CONE_ID)
            .expect("synthetic ProjectInput graph should be valid");
        Self {
            graph,
            build_closure_sources,
            source_cone_ids,
            source_cone_infos,
            consumer_source_indices,
            main_index,
            cone_anchor_main_index: main_index,
            consumer_cone_id: CONSUMER_CONE_ID,
            project_kind: ConeProjectKind::Explicit,
            cone_root,
            cone_manifest,
            entry_package_override,
            entry_main_fqn: None,
        }
    }

    pub fn build_closure_sources(&self) -> &[SourceFile] {
        &self.build_closure_sources
    }

    /// Legacy alias for the build-closure source view.
    ///
    /// This slice can span multiple source cones. It must not be interpreted as
    /// one compilation unit; use `compilation_units()` for cone-level work.
    pub fn sources(&self) -> &[SourceFile] {
        self.build_closure_sources()
    }

    pub fn graph(&self) -> &SourceConeGraph {
        &self.graph
    }

    pub fn compilation_units(&self) -> impl Iterator<Item = SourceConeCompilationUnit<'_>> {
        self.graph.compilation_units()
    }

    pub fn consumer_compilation_unit(&self) -> SourceConeCompilationUnit<'_> {
        self.graph.consumer_compilation_unit()
    }

    pub fn source_cone_id(&self, source_index: usize) -> ConeId {
        self.source_cone_ids[source_index]
    }

    pub fn source_cone_kind(&self, source_index: usize) -> ConeKind {
        self.source_cone_infos[source_index].kind
    }

    pub fn source_cone_info(&self, source_index: usize) -> &SourceConeInfo {
        &self.source_cone_infos[source_index]
    }

    pub fn source_cone_info_map(&self) -> HashMap<PathBuf, SourceConeInfo> {
        let mut out = HashMap::new();
        for unit in self.compilation_units() {
            let info = unit.source_cone_info();
            for source in unit.sources() {
                out.insert(source.path().to_path_buf(), info.clone());
            }
        }
        out
    }

    pub fn source_resolver_cone_info(&self, source_index: usize) -> ConeInfo {
        self.source_cone_info(source_index).resolver_info()
    }

    pub fn consumer_cone_id(&self) -> ConeId {
        self.consumer_cone_id
    }

    pub fn main_source(&self) -> &SourceFile {
        &self.build_closure_sources[self.main_index]
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
        self.consumer_source_paths()
    }

    pub fn consumer_source_paths(&self) -> Vec<PathBuf> {
        self.consumer_compilation_unit()
            .sources()
            .iter()
            .map(|source| source.path().to_path_buf())
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
}

impl ProjectContext {
    pub fn new(input: ProjectInput) -> Self {
        Self { input }
    }

    pub fn input(&self) -> &ProjectInput {
        &self.input
    }

    pub fn into_input(self) -> ProjectInput {
        self.input
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

#[cfg(feature = "llvm")]
/// Production codegen handoff: HIR compatibility scaffold plus MIR-owned canonical snapshot.
///
/// The snapshot stays outside `LoweredHir` so later stages must attach it to `MirStageOutput`
/// explicitly before effect facts or LLVM emit can consume a pass view.
#[derive(Debug)]
pub struct CodegenLoweringOutput {
    pub lowered_hir: hir::LoweredHir,
    pub materialized_mir: crate::mir::MaterializedMir,
}

#[cfg(feature = "llvm")]
impl CodegenLoweringOutput {
    pub fn lowered_hir(&self) -> &hir::LoweredHir {
        &self.lowered_hir
    }

    pub fn materialized_mir(&self) -> &crate::mir::MaterializedMir {
        &self.materialized_mir
    }

    pub fn materialized_callable_view(&self) -> crate::mir::MaterializedCallableView<'_> {
        self.materialized_mir.callable_view()
    }

    pub fn into_parts(self) -> (hir::LoweredHir, crate::mir::MaterializedMir) {
        (self.lowered_hir, self.materialized_mir)
    }
}

pub fn load_project_input_from_path(
    input: &Path,
    entry_package_override: Option<String>,
    session_options: &SessionOptions,
) -> Result<ProjectInput> {
    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（source cone graph）")?;

    if input.is_file() {
        let source = SourceFile::load(input)?;
        let virtual_root = source.path().to_path_buf();
        let manifest = default_virtual_cone_manifest(&source);
        let graph = load_source_cone_graph_for_virtual_consumer(
            source,
            virtual_root,
            manifest,
            &sysroot_root,
            session_options.sysroot_overlay(),
            session_options.extra_sysroot_dependencies(),
        )?;
        return ProjectInput::from_graph(graph, ConeProjectKind::Virtual, None);
    }

    if input.is_dir() {
        let pkg = crate::cone::load_cone_source_package(input)?;
        if pkg.manifest.cone.kind != ConeKind::Bin {
            return Err(miette::miette!(
                "只有 `bin` cone 可作为 executable consumer 输入；`{}` 声明为 `{}` cone",
                pkg.manifest.cone.name,
                pkg.manifest.cone.kind
            ));
        }
        let graph = load_source_cone_graph_for_consumer_package(
            pkg,
            &sysroot_root,
            session_options.sysroot_overlay(),
            &[],
            session_options.extra_sysroot_dependencies(),
        )?;
        return ProjectInput::from_graph(graph, ConeProjectKind::Explicit, entry_package_override);
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

pub fn prepare_virtual_cone_input(source: SourceFile) -> Result<ProjectInput> {
    prepare_virtual_cone_input_with_options(source, &SessionOptions::new())
}

pub fn prepare_virtual_cone_input_with_options(
    source: SourceFile,
    session_options: &SessionOptions,
) -> Result<ProjectInput> {
    let virtual_root = source.path().to_path_buf();
    let manifest = default_virtual_cone_manifest(&source);
    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（source cone graph）")?;
    let graph = load_source_cone_graph_for_virtual_consumer(
        source,
        virtual_root,
        manifest,
        &sysroot_root,
        session_options.sysroot_overlay(),
        session_options.extra_sysroot_dependencies(),
    )?;
    ProjectInput::from_graph(graph, ConeProjectKind::Virtual, None)
}

pub fn prepare_virtual_cone_context(source: SourceFile) -> Result<ProjectContext> {
    prepare_virtual_cone_context_with_options(source, &SessionOptions::new())
}

pub fn prepare_virtual_cone_context_with_options(
    source: SourceFile,
    session_options: &SessionOptions,
) -> Result<ProjectContext> {
    prepare_virtual_cone_input_with_options(source, session_options).map(ProjectContext::new)
}

pub fn run_project_frontend(session: &Session, context: ProjectContext) -> Result<FrontendOutput> {
    run_frontend(session, context.into_input())
}

pub fn run_frontend(session: &Session, mut input: ProjectInput) -> Result<FrontendOutput> {
    if input.build_closure_sources.is_empty() {
        return Err(miette::miette!(
            "内部错误：frontend 输入 build closure sources 为空"
        ));
    }

    let mut asts = Vec::with_capacity(input.build_closure_sources.len());
    for unit in input.compilation_units() {
        let unit_output = crate::pipeline::load_ast_compilation_unit_stage_output(session, unit)
            .map_err(miette::Report::from)?;
        asts.extend(unit_output.into_asts());
    }
    debug_assert_eq!(asts.len(), input.build_closure_sources.len());

    for (source, ast) in input.build_closure_sources.iter().zip(asts.iter()) {
        crate::typecheck::check_file_headers(source, ast).map_err(miette::Report::from)?;
        crate::typecheck::check_file_struct_decls(source, ast).map_err(miette::Report::from)?;
    }

    let mut indexed: Vec<IndexedFile<'_>> = Vec::new();
    for f in &session.sysroot().files {
        if input
            .build_closure_sources
            .iter()
            .any(|source| source.path() == f.source.path())
        {
            continue;
        }
        indexed.push(IndexedFile {
            cone: ConeId::new(0),
            cone_kind: if f.source.is_trusted_syslib() {
                ConeKind::Syslib
            } else {
                ConeKind::Lib
            },
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source_index, (source, ast)) in input
        .build_closure_sources
        .iter()
        .zip(asts.iter())
        .enumerate()
    {
        indexed.push(IndexedFile {
            cone: input.source_cone_id(source_index),
            cone_kind: input.source_cone_kind(source_index),
            source,
            file: ast,
        });
    }

    let mut index = Index::build_with_cones(&indexed).map_err(miette::Report::from)?;
    index.set_export_entry_points(input.cone_manifest.export_entry_points.clone());

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).map_err(miette::Report::from)?;

    select_cone_entry_main(&mut input, &asts, &mut index)?;

    let mut headers = Vec::with_capacity(input.build_closure_sources.len());
    for (source, ast) in input.build_closure_sources.iter().zip(asts.iter()) {
        let h = crate::resolve::check_file_headers(source, ast, &index)
            .map_err(miette::Report::from)?;
        headers.push(h);
    }
    for ((source, ast), h) in input
        .build_closure_sources
        .iter()
        .zip(asts.iter_mut())
        .zip(headers.iter())
    {
        crate::resolve::check_file_bodies(source, ast, &index, h).map_err(miette::Report::from)?;
    }

    for (source, ast) in input.build_closure_sources.iter().zip(asts.iter()) {
        env.extend_from_file(source, ast, &index)
            .map_err(miette::Report::from)?;
    }

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    #[cfg(feature = "llvm")]
    let mut all_monomorph_requests: Vec<MonomorphRequest> = Vec::new();

    #[cfg(feature = "llvm")]
    let file_iter = input
        .build_closure_sources
        .iter()
        .zip(asts.iter())
        .zip(headers.iter())
        .enumerate();
    #[cfg(not(feature = "llvm"))]
    let file_iter = input
        .build_closure_sources
        .iter()
        .zip(asts.iter())
        .zip(headers.iter());

    for item in file_iter {
        #[cfg(feature = "llvm")]
        let (_source_index, ((source, ast), h)) = item;
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
            // Support sources can contain generic calls inside reachable non-generic bodies
            // (for example sysroot class initializers).  Collect their call-site bindings too;
            // the MIR materializer still filters initial roots by consumer cone sources.
            let requests = crate::typecheck::check_file_exprs_with_monomorph_requests(
                source, ast, &index, &h.imports, &env, &mut types, builtins,
            )
            .map_err(miette::Report::from)?;
            all_monomorph_requests.extend(requests);
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
) -> Result<CodegenLoweringOutput> {
    let mut lowering_context_files: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        if front
            .input
            .build_closure_sources
            .iter()
            .any(|source| source.path() == f.source.path())
        {
            continue;
        }
        lowering_context_files.push((&f.source, &f.ast));
    }
    for (source, ast) in front
        .input
        .build_closure_sources
        .iter()
        .zip(front.asts.iter())
    {
        lowering_context_files.push((source, ast));
    }

    let files_to_lower = front
        .input
        .build_closure_sources
        .iter()
        .zip(front.asts.iter())
        .collect::<Vec<_>>();
    let request_source_paths = front.input.mir_request_source_paths();
    let stable_cone_key = front
        .input
        .consumer_compilation_unit()
        .source_cone_info()
        .stable_key;
    let source_cones = front.input.source_cone_info_map();
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

    let materialized_mir =
        crate::mir::materialize_compilation_unit_from_typechecked_inputs_with_options(
            &lowering_context_files,
            &front.index,
            Some(&front.type_env),
            &front.typecheck_types,
            &front.monomorph_requests,
            crate::mir::MaterializeCompilationUnitOptions {
                stable_cone_key: stable_cone_key.clone(),
                source_cones: &source_cones,
                request_source_paths: &request_source_paths,
                request_root_mode,
                opt_level,
            },
        )
        .map_err(|err| miette::Report::from(*err))?;
    let lowered_hir = hir::lower_for_compilation_unit_multi_files_with_explicit_mir_instances(
        &front.index,
        &lowering_context_files,
        &files_to_lower,
        Some(&front.type_env),
        &front.typecheck_types,
        hir::ExplicitMirInstanceLoweringOptions {
            stable_cone_key,
            source_cones: &source_cones,
            instance_keys: &materialized_mir.instance_keys,
            instance_types: &materialized_mir.types,
            devirtualize_dispatch_calls: true,
        },
    )
    .map_err(miette::Report::from)?;
    Ok(CodegenLoweringOutput {
        lowered_hir,
        materialized_mir,
    })
}

pub fn build_source_map(session: &Session, input: &ProjectInput) -> (SourceMap, SourceId) {
    let mut source_map = SourceMap::new();
    for file in &session.sysroot().files {
        if input
            .build_closure_sources
            .iter()
            .any(|source| source.path() == file.source.path())
        {
            continue;
        }
        let _ = source_map.add_source_clone(&file.source);
    }

    let mut entry_source_id = None;
    for (idx, source) in input.build_closure_sources.iter().enumerate() {
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
    synthetic_manifest_for_source(source, ConeKind::Bin)
}

fn synthetic_manifest_for_source(source: &SourceFile, kind: ConeKind) -> ConeManifest {
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
            kind,
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
    for (source_idx, (source, file)) in input
        .build_closure_sources
        .iter()
        .zip(asts.iter())
        .enumerate()
    {
        if input.source_cone_id(source_idx) != input.consumer_cone_id {
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
        let anchor_source = &input.build_closure_sources[input.cone_anchor_main_index];
        let anchor_file = &asts[input.cone_anchor_main_index];
        package_prefix(anchor_source, anchor_file.package.as_ref())
    };

    let entry_main_fqn = cone_entry_main_fqn(&entry_package);
    index.set_runtime_entry_point(entry_main_fqn.clone());
    input.entry_main_fqn = Some(entry_main_fqn.clone());

    let consumer_cone = input.consumer_cone_id();

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
            .build_closure_sources
            .iter()
            .enumerate()
            .find(|(_idx, s)| s.path() == decl_file)
        else {
            return Err(miette::miette!(
                "内部错误：入口 main 源文件未在 build closure source view 中：{}",
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

pub(crate) fn load_default_support_sources(
    session_options: &SessionOptions,
) -> Result<Vec<SourceFile>> {
    let mut support_paths: Vec<(PathBuf, bool)> = Vec::new();
    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（T0143）")?;
    let sysroot_entries = crate::sysroot::collect_auto_sysroot_source_entries(
        &sysroot_root,
        session_options.sysroot_overlay(),
        session_options.extra_sysroot_dependencies(),
    )?;

    support_paths.extend(
        sysroot_entries
            .into_iter()
            .map(|entry| (entry.path, entry.trusted_syslib)),
    );

    support_paths.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

    let mut out = Vec::with_capacity(support_paths.len());
    for (path, trusted_syslib) in support_paths {
        out.push(if trusted_syslib {
            SourceFile::load_trusted_syslib(&path)?
        } else {
            SourceFile::load_sysroot(&path)?
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::resolve::IndexedFile;

    fn bin_manifest(name: &str) -> ConeManifest {
        ConeManifest {
            cone: ConeSection {
                name: name.to_string(),
                version: "0.0.0".to_string(),
                kind: ConeKind::Bin,
            },
            dependencies: Default::default(),
            pre_specialize_functions: Vec::new(),
            pre_specialize_types: Vec::new(),
            export_entry_points: Vec::new(),
            selectors: Vec::new(),
            native_build: ConeNativeBuildConfig::default(),
        }
    }

    fn graph_node(
        id: ConeId,
        role: SourceConeRole,
        manifest: ConeManifest,
        root: &str,
        sources: Vec<SourceFile>,
        entry_main: Option<PathBuf>,
        dependencies: Vec<SourceConeDependencyEdge>,
    ) -> SourceConeNode {
        SourceConeNode {
            id,
            role,
            root: PathBuf::from(root),
            manifest_path: PathBuf::new(),
            kind: manifest.cone.kind,
            native_build: manifest.native_build.clone(),
            manifest,
            trust: SourceConeTrust::Untrusted,
            sources,
            entry_main,
            dependencies,
        }
    }

    #[test]
    fn project_input_is_derived_from_source_cone_graph() {
        let source = SourceFile::new_virtual(
            "/tmp/scoop-source-graph-main.scoop",
            "package fixtures.source_graph\nfun main() {}\n",
        );

        let input = prepare_virtual_cone_input(source).unwrap();
        let graph = input.graph();

        assert_eq!(graph.consumer_id(), input.consumer_cone_id());
        assert_eq!(graph.consumer().role, SourceConeRole::Consumer);
        assert!(
            graph
                .nodes()
                .iter()
                .any(|node| node.role == SourceConeRole::SysrootAuto),
            "virtual project input should include sysroot auto cones in the source graph"
        );
        assert!(
            input
                .consumer_source_indices
                .iter()
                .all(|&idx| input.source_cone_id(idx) == input.consumer_cone_id()),
            "consumer sources must keep the graph consumer cone id"
        );
        assert!(
            input
                .source_cone_ids
                .iter()
                .any(|&id| id != input.consumer_cone_id()),
            "sysroot graph sources must not be flattened into the consumer cone id"
        );
        let source_cone_info_map = input.source_cone_info_map();
        for (idx, source) in input.build_closure_sources().iter().enumerate() {
            let info = input.source_cone_info(idx);
            assert_eq!(source_cone_info_map[source.path()].id, info.id);
            assert_eq!(input.source_cone_kind(idx), info.kind);
        }
    }

    #[test]
    fn virtual_file_input_is_synthetic_consumer_compilation_unit() {
        let source = SourceFile::new_virtual(
            "/tmp/scoop-virtual-consumer-main.scoop",
            "package fixtures.virtual_consumer\nfun main() {}\n",
        );

        let input = prepare_virtual_cone_input(source).unwrap();
        let consumer_unit = input.consumer_compilation_unit();

        assert_eq!(consumer_unit.id(), input.consumer_cone_id());
        assert_eq!(consumer_unit.role(), SourceConeRole::Consumer);
        assert_eq!(consumer_unit.sources().len(), 1);
        assert_eq!(
            input.consumer_source_paths(),
            vec![PathBuf::from("/tmp/scoop-virtual-consumer-main.scoop")]
        );
        assert!(
            input
                .compilation_units()
                .any(|unit| unit.id() != input.consumer_cone_id()),
            "virtual single-file input must still be a synthetic consumer cone inside the source graph"
        );
    }

    #[test]
    fn explicit_multi_file_cone_is_one_compilation_unit_with_stable_source_order() {
        let first = SourceFile::new_virtual(
            "/tmp/scoop-explicit-unit/src/a.scoop",
            "package fixtures.explicit_unit\nfun a() {}\n",
        );
        let second = SourceFile::new_virtual(
            "/tmp/scoop-explicit-unit/src/b.scoop",
            "package fixtures.explicit_unit\nfun main() {}\n",
        );
        let graph = SourceConeGraph::from_nodes(
            vec![graph_node(
                CONSUMER_CONE_ID,
                SourceConeRole::Consumer,
                bin_manifest("fixture-explicit-unit"),
                "/tmp/scoop-explicit-unit",
                vec![first, second],
                Some(PathBuf::from("/tmp/scoop-explicit-unit/src/b.scoop")),
                Vec::new(),
            )],
            CONSUMER_CONE_ID,
        )
        .unwrap();

        let input = ProjectInput::from_graph(graph, ConeProjectKind::Explicit, None).unwrap();
        let consumer_unit = input.consumer_compilation_unit();
        let unit_paths = consumer_unit
            .sources()
            .iter()
            .map(|source| source.path().to_path_buf())
            .collect::<Vec<_>>();

        assert_eq!(input.compilation_units().count(), 1);
        assert_eq!(
            unit_paths,
            vec![
                PathBuf::from("/tmp/scoop-explicit-unit/src/a.scoop"),
                PathBuf::from("/tmp/scoop-explicit-unit/src/b.scoop"),
            ]
        );
        assert_eq!(input.consumer_source_paths(), unit_paths);
        assert_eq!(
            input.main_source().path(),
            Path::new("/tmp/scoop-explicit-unit/src/b.scoop")
        );
        assert!(
            input
                .consumer_source_indices
                .iter()
                .all(|&idx| input.source_cone_id(idx) == CONSUMER_CONE_ID)
        );
    }

    #[test]
    fn compilation_units_follow_dependency_before_consumer_order() {
        let dep_id = ConeId::new(2);
        let dep = SourceFile::new_virtual(
            "/tmp/scoop-unit-dep/src/lib.scoop",
            "package fixtures.unit_dep\nfun dep() {}\n",
        );
        let consumer = SourceFile::new_virtual(
            "/tmp/scoop-unit-app/src/main.scoop",
            "package fixtures.unit_app\nfun main() {}\n",
        );
        let graph = SourceConeGraph::from_nodes(
            vec![
                graph_node(
                    CONSUMER_CONE_ID,
                    SourceConeRole::Consumer,
                    bin_manifest("fixture-unit-app"),
                    "/tmp/scoop-unit-app",
                    vec![consumer],
                    Some(PathBuf::from("/tmp/scoop-unit-app/src/main.scoop")),
                    vec![SourceConeDependencyEdge {
                        target: dep_id,
                        kind: SourceConeDependencyKind::LocalSource,
                    }],
                ),
                graph_node(
                    dep_id,
                    SourceConeRole::LocalDependency,
                    bin_manifest("fixture-unit-dep"),
                    "/tmp/scoop-unit-dep",
                    vec![dep],
                    None,
                    Vec::new(),
                ),
            ],
            CONSUMER_CONE_ID,
        )
        .unwrap();

        let input = ProjectInput::from_graph(graph, ConeProjectKind::Explicit, None).unwrap();
        let units = input.compilation_units().collect::<Vec<_>>();

        assert_eq!(
            units
                .iter()
                .map(|unit| unit.node().manifest.cone.name.as_str())
                .collect::<Vec<_>>(),
            vec!["fixture-unit-dep", "fixture-unit-app"]
        );
        assert_eq!(
            input
                .consumer_compilation_unit()
                .dependency_cone_ids()
                .collect::<Vec<_>>(),
            vec![dep_id]
        );
        assert_eq!(
            input
                .build_closure_sources()
                .iter()
                .map(|source| source.path().to_path_buf())
                .collect::<Vec<_>>(),
            vec![
                PathBuf::from("/tmp/scoop-unit-dep/src/lib.scoop"),
                PathBuf::from("/tmp/scoop-unit-app/src/main.scoop"),
            ]
        );
    }

    #[test]
    fn entry_selection_rejects_dependency_main_without_consumer_main() {
        let consumer = SourceFile::new_virtual(
            "/tmp/scoop-entry-consumer/src/anchor.scoop",
            "package fixtures.entry.consumer\nfun anchor() {}\n",
        );
        let dep = SourceFile::new_virtual(
            "/tmp/scoop-entry-lib/src/main.scoop",
            "package fixtures.entry.lib\nfun main() {}\n",
        );
        let asts = vec![parse_file(&consumer).unwrap(), parse_file(&dep).unwrap()];
        let mut index = Index::build_with_cones(&[
            IndexedFile {
                cone: ConeId::new(1),
                cone_kind: ConeKind::Bin,
                source: &consumer,
                file: &asts[0],
            },
            IndexedFile {
                cone: ConeId::new(2),
                cone_kind: ConeKind::Lib,
                source: &dep,
                file: &asts[1],
            },
        ])
        .unwrap();
        let mut input = ProjectInput::new_explicit(
            vec![consumer, dep],
            vec![0],
            0,
            PathBuf::from("/tmp/scoop-entry-consumer"),
            bin_manifest("fixture-entry-consumer"),
            Some("fixtures.entry.lib".to_string()),
        );

        let err = select_cone_entry_main(&mut input, &asts, &mut index).unwrap_err();
        let diag = err.downcast::<EntryPackageMainNotInConsumerCone>().unwrap();

        assert_eq!(diag.entry_package, "fixtures.entry.lib");
        assert!(
            diag.decl_file
                .ends_with("/tmp/scoop-entry-lib/src/main.scoop")
        );
    }

    #[test]
    fn entry_selection_prefers_consumer_main_over_dependency_same_fqn() {
        let anchor = SourceFile::new_virtual(
            "/tmp/scoop-entry-consumer/src/main.scoop",
            "package fixtures.entry.anchor\nfun anchor() {}\n",
        );
        let consumer = SourceFile::new_virtual(
            "/tmp/scoop-entry-consumer/src/selected.scoop",
            "package fixtures.entry.app\nfun main() {}\n",
        );
        let dep = SourceFile::new_virtual(
            "/tmp/scoop-entry-lib/src/main.scoop",
            "package fixtures.entry.app\nfun main() {}\n",
        );
        let asts = vec![
            parse_file(&anchor).unwrap(),
            parse_file(&consumer).unwrap(),
            parse_file(&dep).unwrap(),
        ];
        let mut index = Index::build_with_cones(&[
            IndexedFile {
                cone: ConeId::new(2),
                cone_kind: ConeKind::Lib,
                source: &dep,
                file: &asts[2],
            },
            IndexedFile {
                cone: ConeId::new(1),
                cone_kind: ConeKind::Bin,
                source: &anchor,
                file: &asts[0],
            },
            IndexedFile {
                cone: ConeId::new(1),
                cone_kind: ConeKind::Bin,
                source: &consumer,
                file: &asts[1],
            },
        ])
        .unwrap();
        let mut input = ProjectInput::new_explicit(
            vec![anchor, consumer, dep],
            vec![0, 1],
            0,
            PathBuf::from("/tmp/scoop-entry-consumer"),
            bin_manifest("fixture-entry-consumer"),
            Some("fixtures.entry.app".to_string()),
        );

        select_cone_entry_main(&mut input, &asts, &mut index).unwrap();

        assert_eq!(input.main_index(), 1);
        assert_eq!(index.runtime_entry_point(), Some("fixtures.entry.app.main"));
    }
}
