use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result};
use thiserror::Error;

use crate::ast;
#[cfg(test)]
use crate::cone::{
    CONSUMER_CONE_ID, SourceConeDependencyEdge, SourceConeDependencyKind, SourceConeNode,
    SourceConeTrust,
};
use crate::cone::{
    ConeId, ConeInfo, ConeKind, ConeManifest, ConeNativeBuildConfig, ConeSection,
    SourceConeCompilationUnit, SourceConeGraph, SourceConeInfo, SourceConeRole,
    build_cached_cone_import_from_artifact, load_cone_source_package_for_platform,
    load_source_cone_graph_for_consumer_package_for_platform,
    load_source_cone_graph_for_virtual_consumer_for_platform,
};
#[cfg(feature = "llvm")]
use crate::opt::OptLevel;
use crate::resolve::{Index, IndexedFile};
use crate::session::{Session, SessionOptions};
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::ty::TypeStore;
use crate::typecheck::TypeEnv;
use scoopc_hir::cone_import::CachedConeImport;

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
        let entry_main = consumer.entry_main.clone();

        // Bin cones must declare an entry anchor (`fun main` location); lib cones
        // (subprocess single-cone mode) may not have one — fall back to the first
        // consumer source so the build closure stays well-formed.
        let consumer_kind = consumer.manifest.cone.kind;
        if consumer_kind == ConeKind::Bin && entry_main.is_none() {
            return Err(miette::miette!(
                "consumer cone `{}` 缺少入口锚点",
                consumer.manifest.cone.name
            ));
        }

        let mut build_closure_sources = Vec::new();
        let mut source_cone_ids = Vec::new();
        let mut source_cone_infos = Vec::new();
        let mut consumer_source_indices = Vec::new();
        let mut explicit_main_index = None;
        let mut first_consumer_index: Option<usize> = None;

        for node in graph.nodes() {
            let cone_info = SourceConeInfo::from_node(node);
            for source in &node.sources {
                let idx = build_closure_sources.len();
                if node.id == consumer_cone_id {
                    consumer_source_indices.push(idx);
                    if first_consumer_index.is_none() {
                        first_consumer_index = Some(idx);
                    }
                    if let Some(entry) = entry_main.as_ref()
                        && source.path() == entry.as_path()
                    {
                        explicit_main_index = Some(idx);
                    }
                }
                build_closure_sources.push(source.clone());
                source_cone_ids.push(node.id);
                source_cone_infos.push(cone_info.clone());
            }
        }

        let main_index = if let Some(idx) = explicit_main_index {
            idx
        } else if let Some(entry) = entry_main.as_ref() {
            return Err(miette::miette!(
                "consumer cone 的入口锚点未出现在 graph sources 中：{}",
                entry.display()
            ));
        } else {
            // Lib consumer (subprocess single-cone mode): anchor on the first consumer
            // source. Downstream `select_cone_entry_main` skips `fun main` lookup for
            // non-Bin consumers, so this anchor only seeds the build closure shape.
            first_consumer_index.ok_or_else(|| {
                miette::miette!(
                    "consumer cone `{}` 没有任何源文件",
                    consumer.manifest.cone.name
                )
            })?
        };

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
    /// consumer 的所有 dep cone 注入 payload，按 DAG 顺序保留。
    ///
    /// 这些 payload 只在 frontend 构造 `Index`/`TypeEnv` 时注入；后续 stage 通过 HIR
    /// semantic artifact 直接消费注入后的环境，不再重放注入。
    cached_cone_imports: Vec<CachedConeImport>,
    /// cache-hit dependency artifacts decoded for LLVM ABI/layout handoff.
    #[cfg(feature = "llvm")]
    cached_dep_artifacts: Vec<crate::llvm::CachedDepArtifactHandoff>,
    /// subprocess single-cone artifact 模式（consumer 是 artifact target）下，frontend
    /// 在 typecheck 完成后构造的 skeleton artifact（含 frontend_import，但 stage products
    /// 仍为空）。subprocess 调用方会跑完后端 pipeline，把非空 LIR/MIR/.o 装回去再写盘。
    consumer_artifact_skeleton: Option<crate::cone::ConeArtifact>,
}

impl FrontendOutput {
    #[allow(clippy::too_many_arguments)]
    fn new(
        input: ProjectInput,
        #[cfg(feature = "llvm")] asts: Vec<ast::File>,
        #[cfg(feature = "llvm")] index: Index,
        #[cfg(feature = "llvm")] monomorph_requests: Vec<MonomorphRequest>,
        #[cfg(feature = "llvm")] typecheck_types: TypeStore,
        #[cfg(feature = "llvm")] type_env: TypeEnv,
        cached_cone_imports: Vec<CachedConeImport>,
        #[cfg(feature = "llvm")] cached_dep_artifacts: Vec<crate::llvm::CachedDepArtifactHandoff>,
        consumer_artifact_skeleton: Option<crate::cone::ConeArtifact>,
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
            cached_cone_imports,
            #[cfg(feature = "llvm")]
            cached_dep_artifacts,
            consumer_artifact_skeleton,
        }
    }

    pub fn take_consumer_artifact_skeleton(&mut self) -> Option<crate::cone::ConeArtifact> {
        self.consumer_artifact_skeleton.take()
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

    pub fn cached_cone_imports(&self) -> &[CachedConeImport] {
        &self.cached_cone_imports
    }

    #[cfg(feature = "llvm")]
    pub fn cached_dep_artifacts(&self) -> &[crate::llvm::CachedDepArtifactHandoff] {
        &self.cached_dep_artifacts
    }
}

fn target_platform_id(session_options: &SessionOptions) -> String {
    session_options
        .target_platform()
        .map(|platform| platform.id().to_string())
        .unwrap_or_else(|| crate::target::TargetPlatform::host().id().to_string())
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
    pub frontend_index: Index,
    pub type_env: TypeEnv,
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

    pub fn frontend_index(&self) -> &Index {
        &self.frontend_index
    }

    pub fn type_env(&self) -> &TypeEnv {
        &self.type_env
    }

    pub fn into_parts(self) -> (hir::LoweredHir, crate::mir::MaterializedMir, Index, TypeEnv) {
        (
            self.lowered_hir,
            self.materialized_mir,
            self.frontend_index,
            self.type_env,
        )
    }
}

pub fn load_project_input_from_path(
    input: &Path,
    entry_package_override: Option<String>,
    session_options: &SessionOptions,
) -> Result<ProjectInput> {
    let target_platform = target_platform_id(session_options);
    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（source cone graph）")?;

    if input.is_file() {
        let source = SourceFile::load(input)?;
        let virtual_root = source.path().to_path_buf();
        let manifest = default_virtual_cone_manifest(&source);
        let graph = load_source_cone_graph_for_virtual_consumer_for_platform(
            source,
            virtual_root,
            manifest,
            &sysroot_root,
            session_options.sysroot_overlay(),
            session_options.extra_sysroot_dependencies(),
            &target_platform,
        )?;
        return ProjectInput::from_graph(graph, ConeProjectKind::Virtual, None);
    }

    if input.is_dir() {
        let pkg = load_cone_source_package_for_platform(input, &target_platform)?;
        if pkg.manifest.cone.kind != ConeKind::Bin {
            return Err(miette::miette!(
                "只有 `bin` cone 可作为 executable consumer 输入；`{}` 声明为 `{}` cone",
                pkg.manifest.cone.name,
                pkg.manifest.cone.kind
            ));
        }
        let graph = load_source_cone_graph_for_consumer_package_for_platform(
            pkg,
            &sysroot_root,
            session_options.sysroot_overlay(),
            &[],
            session_options.extra_sysroot_dependencies(),
            &target_platform,
        )?;
        return ProjectInput::from_graph(graph, ConeProjectKind::Explicit, entry_package_override);
    }

    Err(miette::miette!(
        "输入既不是文件也不是目录：{}",
        input.display()
    ))
}

/// Load any cone (Bin / Lib / Syslib) as a project input rooted at itself.
///
/// Mirrors [`load_project_input_from_path`] but skips the "consumer must be a
/// Bin cone" guard, so subprocess single-cone mode can build the cone graph for
/// a Lib cone (treating it as the consumer of its own subgraph). The caller
/// owns the responsibility of seeding [`FrontendArtifactCache`] with upstream
/// dep cones — without that seed, the subprocess would re-compile every dep.
pub fn load_single_cone_project_input_from_path(
    cone_root: &Path,
    session_options: &SessionOptions,
) -> Result<ProjectInput> {
    if !cone_root.is_dir() {
        return Err(miette::miette!(
            "single-cone 编译输入必须是 cone 根目录：{}",
            cone_root.display()
        ));
    }

    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（source cone graph）")?;

    let target_platform = target_platform_id(session_options);
    let pkg = load_cone_source_package_for_platform(cone_root, &target_platform)?;
    let graph = load_source_cone_graph_for_consumer_package_for_platform(
        pkg,
        &sysroot_root,
        session_options.sysroot_overlay(),
        &[],
        session_options.extra_sysroot_dependencies(),
        &target_platform,
    )?;
    let marker_path = cone_root.join(".scoop-virtual-cone");
    let is_virtual_cone = marker_path.is_file();
    let graph = if is_virtual_cone {
        graph_with_virtual_source_identity(graph, &marker_path)?
    } else {
        graph
    };
    let project_kind = if is_virtual_cone {
        ConeProjectKind::Virtual
    } else {
        ConeProjectKind::Explicit
    };
    ProjectInput::from_graph(graph, project_kind, None)
}

fn graph_with_virtual_source_identity(
    graph: SourceConeGraph,
    marker_path: &Path,
) -> Result<SourceConeGraph> {
    let raw = std::fs::read_to_string(marker_path).into_diagnostic()?;
    let original_path = PathBuf::from(raw.trim());
    if !original_path.is_file() {
        return Ok(graph);
    }
    let original_source = SourceFile::load(&original_path)?;
    let consumer_id = graph.consumer_id();
    let mut nodes = graph.nodes().to_vec();
    for node in &mut nodes {
        if node.id != consumer_id {
            continue;
        }
        node.sources = vec![original_source.clone()];
        node.entry_main = Some(original_source.path().to_path_buf());
    }
    SourceConeGraph::from_nodes(nodes, consumer_id)
}

#[cfg(test)]
pub(crate) fn prepare_virtual_cone_input(source: SourceFile) -> Result<ProjectInput> {
    prepare_virtual_cone_input_with_options(source, &SessionOptions::new())
}

pub(crate) fn prepare_virtual_cone_input_with_options(
    source: SourceFile,
    session_options: &SessionOptions,
) -> Result<ProjectInput> {
    let virtual_root = source.path().to_path_buf();
    let manifest = default_virtual_cone_manifest(&source);
    let target_platform = target_platform_id(session_options);
    let sysroot_root = crate::sysroot::Sysroot::default_path()
        .canonicalize()
        .into_diagnostic()
        .wrap_err("无法定位 sysroot 目录（source cone graph）")?;
    let graph = load_source_cone_graph_for_virtual_consumer_for_platform(
        source,
        virtual_root,
        manifest,
        &sysroot_root,
        session_options.sysroot_overlay(),
        session_options.extra_sysroot_dependencies(),
        &target_platform,
    )?;
    ProjectInput::from_graph(graph, ConeProjectKind::Virtual, None)
}

pub(crate) fn prepare_virtual_cone_context_with_options(
    source: SourceFile,
    session_options: &SessionOptions,
) -> Result<ProjectContext> {
    prepare_virtual_cone_input_with_options(source, session_options).map(ProjectContext::new)
}

pub fn run_project_frontend(session: &Session, context: ProjectContext) -> Result<FrontendOutput> {
    run_frontend(session, context.into_input())
}

#[derive(Debug, Clone, Default)]
pub struct FrontendArtifactCache {
    entries: HashMap<ConeId, FrontendArtifactCacheEntry>,
}

impl FrontendArtifactCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, cone_id: ConeId, entry: FrontendArtifactCacheEntry) {
        self.entries.insert(cone_id, entry);
    }

    fn entry(&self, cone_id: ConeId) -> Option<&FrontendArtifactCacheEntry> {
        self.entries.get(&cone_id)
    }
}

#[derive(Debug, Clone)]
pub struct FrontendArtifactCacheEntry {
    pub artifact_dir: PathBuf,
    pub expected_inputs_fingerprint: Vec<u8>,
    pub direct_dependency_outputs_fingerprints: Vec<(ConeId, Vec<u8>)>,
    pub write_on_cache_miss: bool,
    /// 子进程 single-cone 模式：把该 cone 当作 artifact 输出对象（即使它是 graph
    /// consumer）。默认 false 表示遵守"consumer 不写 artifact"的语义。
    pub is_artifact_target: bool,
}

impl FrontendArtifactCacheEntry {
    pub fn new(artifact_dir: PathBuf, expected_inputs_fingerprint: Vec<u8>) -> Self {
        Self {
            artifact_dir,
            expected_inputs_fingerprint,
            direct_dependency_outputs_fingerprints: Vec::new(),
            write_on_cache_miss: true,
            is_artifact_target: false,
        }
    }

    pub fn with_dependency_outputs_fingerprints(
        mut self,
        fingerprints: Vec<(ConeId, Vec<u8>)>,
    ) -> Self {
        self.direct_dependency_outputs_fingerprints = fingerprints;
        self
    }

    pub fn with_write_on_cache_miss(mut self, write: bool) -> Self {
        self.write_on_cache_miss = write;
        self
    }

    pub fn with_artifact_target(mut self, is_target: bool) -> Self {
        self.is_artifact_target = is_target;
        self
    }
}

#[cfg(feature = "llvm")]
fn build_cached_dep_artifact_handoff(
    dep_id: ConeId,
    artifact_dir: &Path,
    artifact: &crate::cone::ConeArtifact,
) -> Result<crate::llvm::CachedDepArtifactHandoff> {
    let object_files = artifact
        .manifest
        .object_files
        .iter()
        .map(|file_name| {
            artifact_dir
                .join(crate::cone::CONE_ARTIFACT_OBJS_DIR_NAME)
                .join(file_name)
                .canonicalize()
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "cached dep cone {} artifact 缺少 object `{}`",
                        dep_id.as_u32(),
                        file_name
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(crate::llvm::CachedDepArtifactHandoff::new(
        dep_id,
        artifact.manifest.stable_cone_key(),
        artifact.lir_program.clone(),
        artifact.lir_facts.clone(),
        artifact.type_store.clone(),
        object_files,
    ))
}

pub fn run_project_frontend_with_artifact_cache(
    session: &Session,
    context: ProjectContext,
    cache: &FrontendArtifactCache,
) -> Result<FrontendOutput> {
    run_frontend_with_artifact_cache(session, context.into_input(), Some(cache))
}

pub fn run_frontend(session: &Session, input: ProjectInput) -> Result<FrontendOutput> {
    run_frontend_with_artifact_cache(session, input, None)
}

pub fn run_frontend_with_artifact_cache(
    session: &Session,
    mut input: ProjectInput,
    artifact_cache: Option<&FrontendArtifactCache>,
) -> Result<FrontendOutput> {
    if input.build_closure_sources.is_empty() {
        return Err(miette::miette!(
            "内部错误：frontend 输入 build closure sources 为空"
        ));
    }

    let original_build_closure_sources = input.build_closure_sources.clone();
    let original_source_cone_ids = input.source_cone_ids.clone();
    let original_source_cone_infos = input.source_cone_infos.clone();
    let session_sysroot_paths = session
        .sysroot()
        .files
        .iter()
        .map(|file| file.source.path().to_path_buf())
        .collect::<HashSet<_>>();
    let default_sysroot_cone_ids = input
        .graph()
        .nodes()
        .iter()
        .filter(|node| {
            node.role == SourceConeRole::SysrootAuto
                && node
                    .sources
                    .iter()
                    .all(|source| session_sysroot_paths.contains(source.path()))
        })
        .map(|node| node.id)
        .collect::<HashSet<_>>();
    let mut asts_by_source_path: HashMap<PathBuf, ast::File> = HashMap::new();
    let mut published_artifacts: HashMap<ConeId, crate::cone::ConeArtifact> = HashMap::new();
    let mut final_index = None;
    let mut final_env = None;
    // consumer 在 frontend 阶段消费的所有 dep cone 注入 payload，按 dep DAG 顺序排列。
    // 下游 stage 通过 HIR semantic artifact 消费注入后的环境，不再重放这些 payload。
    let mut consumer_cached_cone_imports: Vec<CachedConeImport> = Vec::new();
    #[cfg(feature = "llvm")]
    let mut consumer_cached_dep_artifacts: Vec<crate::llvm::CachedDepArtifactHandoff> = Vec::new();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    #[cfg(feature = "llvm")]
    let mut all_monomorph_requests: Vec<MonomorphRequest> = Vec::new();

    // P10-T04-c-3：fingerprint 命中的 dependency cone 已经有 authoritative artifact。
    // 这里直接发布 artifact 并跳过该 cone 的 frontend pipeline；consumer 后续只通过
    // cached frontend import 与 LLVM dep handoff 消费它，不能再把 dep AST 混入 active view。
    #[cfg(feature = "llvm")]
    let mut cache_hit_artifacts: HashMap<ConeId, (PathBuf, crate::cone::ConeArtifact)> =
        HashMap::new();
    for unit in input.compilation_units() {
        let cache_entry = artifact_cache.and_then(|cache| cache.entry(unit.id()));
        if let Some(entry) = cache_entry
            && !unit.is_consumer()
            && !default_sysroot_cone_ids.contains(&unit.id())
            && entry.artifact_dir.is_dir()
        {
            match crate::cone::ConeArtifact::read_with_inputs_fingerprint(
                &entry.artifact_dir,
                &entry.expected_inputs_fingerprint,
            ) {
                Ok(artifact) => {
                    #[cfg(feature = "llvm")]
                    cache_hit_artifacts
                        .insert(unit.id(), (entry.artifact_dir.clone(), artifact.clone()));
                    published_artifacts.insert(unit.id(), artifact);
                    continue;
                }
                Err(crate::cone::ConeArtifactError::InputsFingerprintMismatch { .. }) => {}
                Err(crate::cone::ConeArtifactError::IncompatibleCompilerVersion { .. })
                | Err(crate::cone::ConeArtifactError::IncompatibleSchemaVersions { .. })
                | Err(crate::cone::ConeArtifactError::MissingFrontendImportPayload { .. })
                | Err(crate::cone::ConeArtifactError::ManifestEncode(_)) => {}
                Err(crate::cone::ConeArtifactError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(miette::miette!("{err}")),
            }
        }

        let unit_output = crate::pipeline::load_ast_compilation_unit_stage_output(session, unit)
            .map_err(miette::Report::from)?;
        let mut unit_asts = unit_output.into_asts();
        debug_assert_eq!(unit_asts.len(), unit.sources().len());

        for (source, ast) in unit.sources().iter().zip(unit_asts.iter()) {
            crate::typecheck::check_file_headers(source, ast).map_err(miette::Report::from)?;
            crate::typecheck::check_file_struct_decls(source, ast).map_err(miette::Report::from)?;
        }

        let mut indexed: Vec<IndexedFile<'_>> = Vec::new();
        for f in &session.sysroot().files {
            if unit
                .sources()
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
        for (source, ast) in unit.sources().iter().zip(unit_asts.iter()) {
            indexed.push(IndexedFile {
                cone: unit.id(),
                cone_kind: unit.kind(),
                source,
                file: ast,
            });
        }

        let mut index = Index::build_with_cones(&indexed).map_err(miette::Report::from)?;
        index.set_export_entry_points(unit.node().manifest.export_entry_points.clone());
        let mut env =
            TypeEnv::from_sysroot(session.sysroot(), &index).map_err(miette::Report::from)?;
        if let Some(target_platform) = session.options().target_platform().cloned() {
            env.set_target_platform(target_platform);
        }

        for dep_id in unit.dependency_cone_ids() {
            let Some(artifact) = published_artifacts.get(&dep_id) else {
                if default_sysroot_cone_ids.contains(&dep_id) {
                    continue;
                }
                return Err(miette::miette!(
                    "内部错误：cone {} 在依赖 {} 的 frontend 之前尚未发布 artifact",
                    dep_id.as_u32(),
                    unit.id().as_u32()
                ));
            };
            crate::cone::inject_cone_artifact_frontend_import(
                &mut index, &mut env, dep_id, artifact,
            )?;
            if unit.is_consumer() {
                consumer_cached_cone_imports
                    .push(build_cached_cone_import_from_artifact(dep_id, artifact));
                #[cfg(feature = "llvm")]
                if let Some((artifact_dir, cached_artifact)) = cache_hit_artifacts.get(&dep_id) {
                    consumer_cached_dep_artifacts.push(build_cached_dep_artifact_handoff(
                        dep_id,
                        artifact_dir,
                        cached_artifact,
                    )?);
                }
            }
        }

        let mut headers = Vec::with_capacity(unit.sources().len());
        for (source, ast) in unit.sources().iter().zip(unit_asts.iter()) {
            let h = crate::resolve::check_file_headers(source, ast, &index)
                .map_err(miette::Report::from)?;
            headers.push(h);
        }
        for ((source, ast), h) in unit
            .sources()
            .iter()
            .zip(unit_asts.iter_mut())
            .zip(headers.iter())
        {
            crate::resolve::check_file_bodies(source, ast, &index, h)
                .map_err(miette::Report::from)?;
        }

        for (source, ast) in unit.sources().iter().zip(unit_asts.iter()) {
            env.extend_from_file(source, ast, &index)
                .map_err(miette::Report::from)?;
        }

        #[cfg(feature = "llvm")]
        let file_iter = unit
            .sources()
            .iter()
            .zip(unit_asts.iter())
            .zip(headers.iter())
            .enumerate();
        #[cfg(not(feature = "llvm"))]
        let file_iter = unit
            .sources()
            .iter()
            .zip(unit_asts.iter())
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
            crate::typecheck::check_file_inheritance(
                source, ast, &index, &h.imports, &env, &mut types, builtins,
            )
            .map_err(miette::Report::from)?;
            crate::typecheck::check_file_interfaces(
                source, ast, &index, &h.imports, &env, &mut types, builtins,
            )
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

        let mut lowering_context_files: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            if unit
                .sources()
                .iter()
                .any(|source| source.path() == f.source.path())
            {
                continue;
            }
            lowering_context_files.push((&f.source, &f.ast));
        }
        for (source, ast) in unit.sources().iter().zip(unit_asts.iter()) {
            lowering_context_files.push((source, ast));
        }
        let unit_is_artifact_target = cache_entry
            .map(|entry| entry.is_artifact_target)
            .unwrap_or(false);
        let artifact = if (unit.is_consumer() && !unit_is_artifact_target)
            || default_sysroot_cone_ids.contains(&unit.id())
        {
            None
        } else {
            let mut export_lowering_context_files: Vec<(&SourceFile, &ast::File)> =
                Vec::with_capacity(lowering_context_files.len());
            let unit_path_to_export_ast = unit
                .sources()
                .iter()
                .zip(unit_asts.iter())
                .map(|(source, ast)| (source.path(), ast))
                .collect::<HashMap<_, _>>();
            for (source, ast) in &lowering_context_files {
                let export_ast = unit_path_to_export_ast
                    .get(source.path())
                    .copied()
                    .unwrap_or(*ast);
                export_lowering_context_files.push((*source, export_ast));
            }
            let frontend_import =
                if unit.is_consumer() && unit.node().manifest.cone.kind == ConeKind::Bin {
                    crate::cone::ConeArtifactFrontendImport::empty()
                } else {
                    crate::cone::build_frontend_import_for_typechecked_cone(
                        session,
                        unit.sources(),
                        &unit_asts,
                        &unit.node().manifest,
                        &index,
                        &env,
                        &export_lowering_context_files,
                    )?
                };
            Some(crate::cone::ConeArtifact::new(
                unit.source_cone_info().stable_key,
                unit.source_cone_info().kind,
                scoopc_hir_facts::HirFacts::new(),
                scoopc_mir_facts::MirFacts::new(),
                scoopc_effect_facts::EffectFacts::new(),
                scoopc_lir_facts::LirFacts::new(crate::opt::OptLevel::O0),
                scoopc_lir::LateLoweredProgram::new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                TypeStore::new(),
                frontend_import,
            ))
        };

        for (source, ast) in unit.sources().iter().zip(unit_asts) {
            asts_by_source_path.insert(source.path().to_path_buf(), ast);
        }

        if unit.is_consumer() {
            final_index = Some(index);
            final_env = Some(env);
        }
        if let Some(artifact) = artifact {
            if let Some(entry) = cache_entry {
                let mut artifact = artifact;
                artifact.inputs_fingerprint = entry.expected_inputs_fingerprint.clone();
                // 走到这里说明当前 unit 没有 artifact cache hit；按调用方策略写入新的
                // artifact。cache-hit 路径已经在 loop 顶部发布旧 artifact 并 `continue`。
                if entry.write_on_cache_miss {
                    artifact
                        .write_with_computed_outputs_fingerprint(&entry.artifact_dir)
                        .map_err(|err| miette::miette!("{err}"))?;
                }
                published_artifacts.insert(unit.id(), artifact);
                continue;
            }
            published_artifacts.insert(unit.id(), artifact);
        }
    }

    let mut active_sources = Vec::new();
    let mut active_source_cone_ids = Vec::new();
    let mut active_source_cone_infos = Vec::new();
    let mut active_asts = Vec::new();
    let mut main_index = None;
    let mut cone_anchor_main_index = None;
    let mut consumer_source_indices = Vec::new();
    for (idx, source) in original_build_closure_sources.iter().enumerate() {
        let Some(ast) = asts_by_source_path.remove(source.path()) else {
            continue;
        };
        let active_idx = active_sources.len();
        if idx == input.main_index {
            main_index = Some(active_idx);
        }
        if idx == input.cone_anchor_main_index {
            cone_anchor_main_index = Some(active_idx);
        }
        if original_source_cone_ids[idx] == input.consumer_cone_id {
            consumer_source_indices.push(active_idx);
        }
        active_sources.push(source.clone());
        active_source_cone_ids.push(original_source_cone_ids[idx]);
        active_source_cone_infos.push(original_source_cone_infos[idx].clone());
        active_asts.push(ast);
    }
    input.build_closure_sources = active_sources;
    input.source_cone_ids = active_source_cone_ids;
    input.source_cone_infos = active_source_cone_infos;
    input.consumer_source_indices = consumer_source_indices;
    input.main_index =
        main_index.ok_or_else(|| miette::miette!("内部错误：frontend 未生成入口 source AST"))?;
    input.cone_anchor_main_index = cone_anchor_main_index
        .ok_or_else(|| miette::miette!("内部错误：frontend 未生成 cone anchor source AST"))?;
    let asts = active_asts;
    let mut index =
        final_index.ok_or_else(|| miette::miette!("内部错误：consumer cone 未运行 frontend"))?;
    select_cone_entry_main(&mut input, &asts, &mut index)?;
    let env = final_env.ok_or_else(|| miette::miette!("内部错误：consumer cone 缺少 TypeEnv"))?;

    // P10-T04-c：subprocess single-cone artifact 模式下，consumer 自己就是 artifact target。
    // 把 consumer 的 skeleton 从 published_artifacts 摘出来交还给调用方，由 subprocess 跑完
    // 后端 pipeline 后再把非空 LIR/MIR/.o 装回去并写盘——避免在 frontend 阶段把空 stage
    // products 写进磁盘。
    let consumer_artifact_skeleton = if let Some(cache) = artifact_cache
        && let Some(entry) = cache.entry(input.consumer_cone_id)
        && entry.is_artifact_target
    {
        published_artifacts.remove(&input.consumer_cone_id)
    } else {
        None
    };

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
        consumer_cached_cone_imports,
        #[cfg(feature = "llvm")]
        consumer_cached_dep_artifacts,
        consumer_artifact_skeleton,
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
        },
    )
    .map_err(miette::Report::from)?;
    Ok(CodegenLoweringOutput {
        lowered_hir,
        materialized_mir,
        frontend_index: front.index.clone(),
        type_env: front.type_env.clone(),
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
    // Lib / Syslib consumer cones (subprocess single-cone mode) have no `fun main`;
    // skip the entry-package lookup so we don't surface a spurious
    // `EntryPackageMissingMain` diagnostic. Downstream stages that need a runtime
    // entry point (codegen + link) only run for Bin projects.
    if input.cone_manifest.cone.kind != ConeKind::Bin {
        return Ok(());
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;
    use crate::resolve::IndexedFile;
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn downstream_frontend_uses_dependency_artifact_imports() {
        let dep_id = ConeId::new(2);
        let dep = SourceFile::new_virtual(
            "/tmp/scoop-artifact-dep/src/lib.scoop",
            "package fixtures.artifact.dep\npublic fun dep(): Int { return 42 }\n",
        );
        let consumer = SourceFile::new_virtual(
            "/tmp/scoop-artifact-app/src/main.scoop",
            "package fixtures.artifact.app\nimport fixtures.artifact.dep.*\nfun main() { dep() }\n",
        );
        let mut dep_manifest = bin_manifest("fixture-artifact-dep");
        dep_manifest.cone.kind = ConeKind::Lib;
        let graph = SourceConeGraph::from_nodes(
            vec![
                graph_node(
                    CONSUMER_CONE_ID,
                    SourceConeRole::Consumer,
                    bin_manifest("fixture-artifact-app"),
                    "/tmp/scoop-artifact-app",
                    vec![consumer],
                    Some(PathBuf::from("/tmp/scoop-artifact-app/src/main.scoop")),
                    vec![SourceConeDependencyEdge {
                        target: dep_id,
                        kind: SourceConeDependencyKind::LocalSource,
                    }],
                ),
                graph_node(
                    dep_id,
                    SourceConeRole::LocalDependency,
                    dep_manifest,
                    "/tmp/scoop-artifact-dep",
                    vec![dep],
                    None,
                    Vec::new(),
                ),
            ],
            CONSUMER_CONE_ID,
        )
        .unwrap();
        let input = ProjectInput::from_graph(graph, ConeProjectKind::Explicit, None).unwrap();
        let session = Session::new().unwrap();

        let output = run_frontend(&session, input).unwrap();
        let overload = output.index().by_fqn["fixtures.artifact.dep.dep"]
            .fun
            .first()
            .expect("dependency public fun should be imported");

        assert_eq!(overload.symbol.decl_cone, dep_id);
        assert!(!overload.has_body);
        assert_eq!(
            overload.symbol.decl_file.display().to_string(),
            "<cone:fixture-artifact-dep@0.0.0>"
        );
    }

    #[test]
    fn dependency_frontend_cache_hit_uses_artifact_without_reading_source() {
        // P10-T04-c-3：fingerprint 命中时 dep cone 必须完全由 artifact 供给，不能再 parse
        // 或 typecheck dep 源，也不能把 dep AST 留在 consumer active source view 中。
        // fingerprint mismatch 时仍必须回退到完整 frontend pipeline。
        let dep_id = ConeId::new(2);
        let cache_dir = unique_temp_dir("scoop-frontend-cache-hit");
        let artifact_dir = cache_dir.join("cones").join("fixture-cache-dep@0.0.0");
        let expected_inputs = b"cache-hit-inputs".to_vec();
        let session = Session::new().unwrap();

        let mut first_cache = FrontendArtifactCache::new();
        first_cache.insert(
            dep_id,
            FrontendArtifactCacheEntry::new(artifact_dir.clone(), expected_inputs.clone()),
        );
        let dep_source = "package fixtures.cache.dep\npublic fun dep(): Int { return 42 }\n";
        let first = ProjectInput::from_graph(
            cached_dependency_graph(dep_id, dep_source),
            ConeProjectKind::Explicit,
            None,
        )
        .unwrap();
        run_frontend_with_artifact_cache(&session, first, Some(&first_cache))
            .expect("cache miss should parse dependency and write artifact");
        assert!(
            artifact_dir
                .join(crate::cone::CONE_ARTIFACT_MANIFEST_FILE_NAME)
                .is_file(),
            "cache miss should publish a dependency artifact"
        );

        // 第二轮：fingerprint 命中。故意传入语法损坏的 dep 源；如果 cache-hit 路径仍然
        // 读取 dep source，这里会在 parser/typecheck 阶段失败。
        let mut hit_cache = FrontendArtifactCache::new();
        hit_cache.insert(
            dep_id,
            FrontendArtifactCacheEntry::new(artifact_dir.clone(), expected_inputs)
                .with_write_on_cache_miss(false),
        );
        let broken_but_cached = ProjectInput::from_graph(
            cached_dependency_graph(
                dep_id,
                "package fixtures.cache.dep\npublic fun dep(): Int {",
            ),
            ConeProjectKind::Explicit,
            None,
        )
        .unwrap();
        let output =
            run_frontend_with_artifact_cache(&session, broken_but_cached, Some(&hit_cache))
                .expect("matching cache hit should succeed without reading dependency source");
        assert!(
            output
                .input()
                .build_closure_sources()
                .iter()
                .all(|source| !source.path().ends_with("lib.scoop")),
            "cache-hit 后 dep source 不应出现在 consumer build closure source view"
        );
        let overload = output.index().by_fqn["fixtures.cache.dep.dep"]
            .fun
            .first()
            .expect("cached dependency public fun should be imported");
        assert_eq!(
            overload.symbol.decl_file.display().to_string(),
            "<cone:fixture-cache-dep@0.0.0>"
        );

        // P10-T04-b: 验证 frontend 仍记录 cache-hit dep 的 import payload；下游通过
        // HIR semantic artifact 消费注入后的环境，不再自行重放注入。
        assert_eq!(
            output.cached_cone_imports().len(),
            1,
            "consumer cache-hit 输出应聚合一个 cached cone import payload"
        );
        let cached = &output.cached_cone_imports()[0];
        assert_eq!(cached.decl_cone, dep_id);
        assert_eq!(cached.cone_name, "fixture-cache-dep");
        assert_eq!(cached.cone_version, "0.0.0");
        assert_eq!(cached.cone_kind, ConeKind::Lib);
        assert_eq!(
            cached.decl_file.display().to_string(),
            "<cone:fixture-cache-dep@0.0.0>"
        );
        assert!(
            cached
                .funs
                .iter()
                .any(|fun| fun.fqn == "fixtures.cache.dep.dep"),
            "cached cone import 应携带 dep public fun"
        );

        // fingerprint mismatch 时 dep cache 失效，cache-miss 路径需要把 broken source
        // 当作真实输入跑完整 pipeline，从而把语法错误抛出来。
        let mut miss_cache = FrontendArtifactCache::new();
        miss_cache.insert(
            dep_id,
            FrontendArtifactCacheEntry::new(artifact_dir, b"different-inputs".to_vec())
                .with_write_on_cache_miss(false),
        );
        let broken_source = ProjectInput::from_graph(
            cached_dependency_graph(
                dep_id,
                "package fixtures.cache.dep\npublic fun dep(): Int {",
            ),
            ConeProjectKind::Explicit,
            None,
        )
        .unwrap();
        run_frontend_with_artifact_cache(&session, broken_source, Some(&miss_cache))
            .expect_err("fingerprint mismatch should miss and re-read broken dependency source");

        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[test]
    fn dependency_frontend_uses_its_own_export_entry_points() {
        let dep_id = ConeId::new(2);
        let dep = SourceFile::new_virtual(
            "/tmp/scoop-export-dep/src/lib.scoop",
            "package fixtures.export.dep\npublic fun exported() { () }\n",
        );
        let consumer = SourceFile::new_virtual(
            "/tmp/scoop-export-app/src/main.scoop",
            "package fixtures.export.app\npublic fun main() / Pure! { () }\n",
        );
        let mut dep_manifest = bin_manifest("fixture-export-dep");
        dep_manifest.cone.kind = ConeKind::Lib;
        dep_manifest.export_entry_points = vec!["fixtures.export.dep.exported".to_owned()];
        let graph = SourceConeGraph::from_nodes(
            vec![
                graph_node(
                    CONSUMER_CONE_ID,
                    SourceConeRole::Consumer,
                    bin_manifest("fixture-export-app"),
                    "/tmp/scoop-export-app",
                    vec![consumer],
                    Some(PathBuf::from("/tmp/scoop-export-app/src/main.scoop")),
                    vec![SourceConeDependencyEdge {
                        target: dep_id,
                        kind: SourceConeDependencyKind::LocalSource,
                    }],
                ),
                graph_node(
                    dep_id,
                    SourceConeRole::LocalDependency,
                    dep_manifest,
                    "/tmp/scoop-export-dep",
                    vec![dep],
                    None,
                    Vec::new(),
                ),
            ],
            CONSUMER_CONE_ID,
        )
        .unwrap();
        let input = ProjectInput::from_graph(graph, ConeProjectKind::Explicit, None).unwrap();
        let session = Session::new().unwrap();

        let err =
            run_frontend(&session, input).expect_err("dependency export entry must be checked");
        assert_eq!(
            err.code().map(|code| code.to_string()).as_deref(),
            Some("scoop::typecheck::export_entry_point_must_declare_closed_pure")
        );
    }

    fn cached_dependency_graph(dep_id: ConeId, dep_text: &str) -> SourceConeGraph {
        let dep = SourceFile::new_virtual("/tmp/scoop-cache-dep/src/lib.scoop", dep_text);
        let consumer = SourceFile::new_virtual(
            "/tmp/scoop-cache-app/src/main.scoop",
            "package fixtures.cache.app\nimport fixtures.cache.dep.*\nfun main() { dep() }\n",
        );
        let mut dep_manifest = bin_manifest("fixture-cache-dep");
        dep_manifest.cone.kind = ConeKind::Lib;
        SourceConeGraph::from_nodes(
            vec![
                graph_node(
                    CONSUMER_CONE_ID,
                    SourceConeRole::Consumer,
                    bin_manifest("fixture-cache-app"),
                    "/tmp/scoop-cache-app",
                    vec![consumer],
                    Some(PathBuf::from("/tmp/scoop-cache-app/src/main.scoop")),
                    vec![SourceConeDependencyEdge {
                        target: dep_id,
                        kind: SourceConeDependencyKind::LocalSource,
                    }],
                ),
                graph_node(
                    dep_id,
                    SourceConeRole::LocalDependency,
                    dep_manifest,
                    "/tmp/scoop-cache-dep",
                    vec![dep],
                    None,
                    Vec::new(),
                ),
            ],
            CONSUMER_CONE_ID,
        )
        .unwrap()
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
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
