//! generic MIR template -> monomorphic MIR instance materialization（当前先服务 dump-ir）。
//!
//! 当前阶段的目标边界：
//! - 在 MIR 层定义稳定的 `TemplateKey` / `InstanceKey`；
//! - 用 typecheck 收集到的“实例请求”作为初始种子；
//! - 基于 generic MIR template 做单态实例物化，而不是对每个实例重新回到 HIR lowering；
//! - 先覆盖 dump/调试路径需要的最小闭环：standalone direct-call fixed-point、nested closure family
//!   的 FQN/fn_ptr 重写，以及 per-instance cache。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::monomorph::MonomorphRequest;
use crate::opt::OptLevel;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};
use crate::typecheck;
use crate::typecheck::{
    AnnotationError, ExprTypeError, StructDeclError, TypeEnv, TypeEnvError, TypeHeaderError,
    TypeLowerError,
};

use super::{
    Body, CallArg, CallKind, ConstValue, DeclOnlySummaryInput, File, FunDecl, HandleMetadata,
    HandlerArm, InstanceRootSummaryInput, Item, LocalDecl, MaterializedCallableFamilies,
    MaterializedCallableFamilyInput, MaterializedMirPassArtifacts, MaterializedMirSummaries,
    MemberAccessMetadata, MemberTarget, Operand, Pattern, PerformMetadata, Rvalue, Statement,
    StatementKind, Terminator, TerminatorKind, TopLevelRef, build_materialized_summary_table,
};

/// 一个 generic MIR template 的稳定标识。
///
/// 说明：
/// - `fqn` 给出语言级声明身份；
/// - `source_path + decl_span` 用于区分同名 overload / 多文件重复 span；
/// - 后续编译单元主路径也应复用这一层语义，而不是退回 mangled symbol name。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TemplateKey {
    pub fqn: String,
    pub source_path: PathBuf,
    pub decl_span: Span,
}

impl fmt::Debug for TemplateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{}:{:?}",
            self.fqn,
            self.source_path.display(),
            self.decl_span
        )
    }
}

/// 一个 monomorphic MIR instance 的稳定身份。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub template: TemplateKey,
    pub type_args: Vec<TypeId>,
    pub eff_args: Vec<EffectRow>,
}

impl fmt::Debug for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstanceKey")
            .field("template", &self.template)
            .field("type_args", &TypeIdList(&self.type_args))
            .field("eff_args", &EffectRowList(&self.eff_args))
            .finish()
    }
}

struct TypeIdList<'a>(&'a [TypeId]);

impl fmt::Debug for TypeIdList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

struct TypeIdRepr(TypeId);

impl fmt::Debug for TypeIdRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0.as_u32())
    }
}

struct EffectRowList<'a>(&'a [EffectRow]);

impl fmt::Debug for EffectRowList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(EffectRowRepr))
            .finish()
    }
}

struct EffectRowRepr<'a>(&'a EffectRow);

impl fmt::Debug for EffectRowRepr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_pure() {
            return write!(f, "Pure");
        }
        f.debug_list()
            .entries(self.0.terms.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

/// `dump-ir` / tests 使用的 monomorphic MIR 输出。
#[derive(Debug)]
pub struct MaterializedMir {
    pub file: File,
    pub types: TypeStore,
    pub instance_keys: Vec<InstanceKey>,
    pub summaries: MaterializedMirSummaries,
    opt_level: OptLevel,
    callable_families: MaterializedCallableFamilies,
    pass_artifacts: MaterializedMirPassArtifacts,
    caller_side_pass_candidates: Vec<FunDecl>,
}

impl MaterializedMir {
    /// 返回当前 materialized MIR 上 canonical 的 callable body / summary 查询视图。
    pub fn callable_view(&self) -> super::MaterializedCallableView<'_> {
        super::MaterializedCallableView::new(self, &self.callable_families)
    }

    /// 返回当前 materialized MIR 在 production/codegen 主路径上使用的 canonical pass 视图。
    pub fn pass_view(&self) -> super::MaterializedMirPassView<'_> {
        super::MaterializedMirPassView::new(self)
    }

    /// 返回当前 materialized MIR 上挂载的 canonical pass 产物 side table。
    pub fn pass_artifacts(&self) -> &super::MaterializedMirPassArtifacts {
        &self.pass_artifacts
    }

    /// 返回当前 canonical materialized MIR snapshot 对应的优化等级。
    pub fn opt_level(&self) -> OptLevel {
        self.opt_level
    }

    /// 返回当前 materialized MIR 上挂载的 canonical pass 产物 side table 的可变引用。
    ///
    /// 说明：
    /// - 后续 MIR pass 应优先通过这层写入 rewritten callable body / summary / family 映射；
    /// - raw `file` / `summaries` 保留为 materialization 原始产物，不应再被 pass rewrite 隐式覆盖。
    pub fn pass_artifacts_mut(&mut self) -> &mut super::MaterializedMirPassArtifacts {
        &mut self.pass_artifacts
    }

    /// 返回 request-root 可达的 non-generic callable body，供 caller-side MIR pass 作为候选输入。
    ///
    /// 这些 body 现在会在 materialization 结束时同步发布到 canonical `MaterializedMirPassView`
    /// 的 ordinary callable family 映射里；原始候选列表仍保留，供尚未完全切到 pass-view query
    /// 面的 caller-side pass / 调试路径复用。
    pub(crate) fn caller_side_pass_candidate_bodies(&self) -> &[FunDecl] {
        &self.caller_side_pass_candidates
    }
}

/// MIR 实例化错误。
#[derive(Debug, Error, Diagnostic)]
pub enum MirMaterializeError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Hir(#[from] crate::hir::HirLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Comptime(#[from] crate::comptime::ConstEvalError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeHeader(#[from] TypeHeaderError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    StructDecl(#[from] StructDeclError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeEnv(#[from] TypeEnvError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Annotation(#[from] AnnotationError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ExprType(#[from] ExprTypeError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    VtableLayout(#[from] crate::vtable::VtableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ItableLayout(#[from] crate::itable::ItableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    MirLower(#[from] super::MirLowerError),

    #[error("{message}")]
    Frontend { message: String },

    #[error("实例请求找不到对应的 generic template：{fqn}@{file}:{span:?}")]
    #[diagnostic(code(scoop::mir::materialize::missing_generic_template))]
    MissingGenericTemplate {
        fqn: String,
        file: String,
        span: Span,
    },

    #[error("generic template 没有匹配的 MIR 根函数：{fqn}@{file}:{span:?}")]
    #[diagnostic(code(scoop::mir::materialize::missing_mir_root_for_template))]
    MissingMirRootForTemplate {
        fqn: String,
        file: String,
        span: Span,
    },

    #[error("实例化的 type args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::mir::materialize::type_arg_arity_mismatch))]
    TypeArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
        #[label("模板声明在这里")]
        decl_span: miette::SourceSpan,
    },

    #[error("实例化的 effect args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::mir::materialize::effect_arg_arity_mismatch))]
    EffectArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
        #[label("模板声明在这里")]
        decl_span: miette::SourceSpan,
    },
}

type MaterializeResult<T> = Result<T, Box<MirMaterializeError>>;

fn materialize_err(error: MirMaterializeError) -> Box<MirMaterializeError> {
    Box::new(error)
}

impl From<crate::hir::HirLowerError> for Box<MirMaterializeError> {
    fn from(error: crate::hir::HirLowerError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<ParseError> for Box<MirMaterializeError> {
    fn from(error: ParseError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<crate::comptime::ConstEvalError> for Box<MirMaterializeError> {
    fn from(error: crate::comptime::ConstEvalError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<ResolveError> for Box<MirMaterializeError> {
    fn from(error: ResolveError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<TypeHeaderError> for Box<MirMaterializeError> {
    fn from(error: TypeHeaderError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<StructDeclError> for Box<MirMaterializeError> {
    fn from(error: StructDeclError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<TypeEnvError> for Box<MirMaterializeError> {
    fn from(error: TypeEnvError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<TypeLowerError> for Box<MirMaterializeError> {
    fn from(error: TypeLowerError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<AnnotationError> for Box<MirMaterializeError> {
    fn from(error: AnnotationError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<ExprTypeError> for Box<MirMaterializeError> {
    fn from(error: ExprTypeError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<crate::vtable::VtableLayoutError> for Box<MirMaterializeError> {
    fn from(error: crate::vtable::VtableLayoutError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<crate::itable::ItableLayoutError> for Box<MirMaterializeError> {
    fn from(error: crate::itable::ItableLayoutError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<super::MirLowerError> for Box<MirMaterializeError> {
    fn from(error: super::MirLowerError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

fn frontend_err(message: impl Into<String>) -> Box<MirMaterializeError> {
    materialize_err(MirMaterializeError::Frontend {
        message: message.into(),
    })
}

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances。
pub fn materialize_for_dump(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<MaterializedMir> {
    materialize_for_dump_with_opt_level(session, source, OptLevel::O2)
}

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances，并显式指定 MIR pass 优化等级。
pub fn materialize_for_dump_with_opt_level(
    session: &Session,
    source: &SourceFile,
    opt_level: OptLevel,
) -> MaterializeResult<MaterializedMir> {
    let DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types,
        monomorph_requests,
    } = collect_dump_materialization_inputs(session, source)?;
    let compilation_unit = prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    super::materialize_compilation_unit_from_typechecked_inputs_with_opt_level(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &index,
        Some(&env),
        &typecheck_types,
        &monomorph_requests,
        opt_level,
    )
}

/// 基于既有 typechecked compilation-unit facts 执行 generic MIR template -> monomorphic
/// instance materialization。
///
/// 说明：
/// - 该入口直接复用调用方已经准备好的 `Index` / `TypeEnv` / `TypeStore` /
///   `MonomorphRequest` 与 AST side tables，不重新跑 parse/resolve/typecheck；
/// - dump/debug 路径目前通过它做包装，后续 build/frontend 主路径也将复用同一层。
pub(crate) fn materialize_compilation_unit_from_typechecked_inputs(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    type_env: Option<&TypeEnv>,
    typecheck_types: &TypeStore,
    monomorph_requests: &[MonomorphRequest],
    options: super::MaterializeCompilationUnitOptions<'_>,
) -> MaterializeResult<MaterializedMir> {
    let super::MaterializeCompilationUnitOptions {
        request_source_paths,
        request_root_mode,
        opt_level,
    } = options;
    let template_catalog = collect_generic_template_infos(compilation_unit);
    let callable_body_infos = collect_callable_body_infos(compilation_unit);
    // materialized callee 可能定义在 helper/sysroot 等“非请求源文件”中，因此 generic
    // template lowering 与 site binding 收集都必须覆盖完整 compilation unit；调用方只需通过
    // `monomorph_requests` 决定初始请求种子，而不是把 template 提供者排除在外。
    let (top_level_fun_value_refs, top_level_fun_call_bindings) =
        collect_site_instance_bindings(compilation_unit);
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        index,
        compilation_unit,
        compilation_unit,
        type_env,
        typecheck_types,
    )?;
    let request_root_fun_keys =
        collect_request_root_fun_keys(&lowered_hir, request_source_paths, index, request_root_mode);
    let request_sources = request_source_paths.iter().cloned().collect::<HashSet<_>>();
    let callable_signatures = collect_callable_signature_infos(&lowered_hir);
    let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
    let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
        &mut lowered_hir,
        typecheck_types,
        &top_level_fun_call_bindings,
    );
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
    let class_vtables = lowered_hir.class_vtables.clone();
    let interfaces = lowered_hir.interfaces.clone();
    let class_itables = lowered_hir.class_itables.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let facts = super::MirLoweringFacts::from_lowered_hir(&lowered_hir);
    let generic_file = super::lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = lowered_hir.types;

    materialize_generic_mir(
        generic_file,
        types,
        builtins,
        MaterializeRequestSet {
            monomorph_requests,
            hir_direct_instance_keys_by_fun,
            construction_inputs: MaterializerConstructionInputs {
                typecheck_types,
                template_infos: template_catalog,
                callable_body_infos,
                callable_signatures,
                known_receiver_subclasses,
                class_vtables,
                interfaces,
                class_itables,
                top_level_fun_value_refs,
                top_level_fun_call_bindings,
                top_level_immutable_values,
                request_sources,
                request_root_mode,
                request_root_fun_keys,
            },
        },
        opt_level,
    )
}

#[derive(Clone)]
struct PreparedDumpFile {
    source: SourceFile,
    ast: ast::File,
    extend_type_env: bool,
    collect_monomorph_keys: bool,
}

struct DumpMaterializationInputs {
    prepared_files: Vec<PreparedDumpFile>,
    index: Index,
    env: TypeEnv,
    typecheck_types: TypeStore,
    monomorph_requests: Vec<MonomorphRequest>,
}

type SourceSiteKey = (PathBuf, Span);

#[derive(Clone)]
struct RequestRootFunKey {
    source_path: PathBuf,
    fqn: String,
    span: Span,
}

#[derive(Clone)]
struct CallableBodyInfo {
    request_lookup_key: RequestTemplateKey,
    source_path: PathBuf,
    fqn: String,
    body_span: Span,
}

#[derive(Clone)]
struct CallableSignatureParam {
    name: String,
    ty: TypeId,
}

#[derive(Clone)]
struct CallableSignatureInfo {
    template: TemplateKey,
    fun_ty: TypeId,
    return_ty: TypeId,
    params: Vec<CallableSignatureParam>,
}

struct MaterializerConstructionInputs<'a> {
    typecheck_types: &'a TypeStore,
    template_infos: Vec<GenericTemplateInfo>,
    callable_body_infos: Vec<CallableBodyInfo>,
    callable_signatures: Vec<CallableSignatureInfo>,
    known_receiver_subclasses: crate::devirtualize::KnownReceiverSubclassIndex,
    class_vtables: crate::vtable::ClassVtableIndex,
    interfaces: crate::itable::InterfaceIndex,
    class_itables: crate::itable::ClassItableIndex,
    top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
    top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    top_level_immutable_values: crate::hir::TopLevelImmutableValueIndex,
    request_sources: HashSet<PathBuf>,
    request_root_mode: super::MaterializeRequestRootMode<'a>,
    request_root_fun_keys: Vec<RequestRootFunKey>,
}

struct MaterializeRequestSet<'a> {
    monomorph_requests: &'a [MonomorphRequest],
    hir_direct_instance_keys_by_fun: HashMap<(PathBuf, Span), Vec<InstanceKey>>,
    construction_inputs: MaterializerConstructionInputs<'a>,
}

fn collect_request_root_fun_keys(
    lowered_hir: &crate::hir::LoweredHir,
    request_source_paths: &[PathBuf],
    index: &Index,
    request_root_mode: super::MaterializeRequestRootMode<'_>,
) -> Vec<RequestRootFunKey> {
    let request_sources = request_source_paths
        .iter()
        .cloned()
        .collect::<HashSet<PathBuf>>();
    let mut out = Vec::new();

    match request_root_mode {
        super::MaterializeRequestRootMode::RequestSources => {
            for item in &lowered_hir.file.items {
                let crate::hir::Item::Fun(fun) = item else {
                    continue;
                };
                if request_sources.contains(&fun.source_path) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }

            for fun in &lowered_hir.member_funs {
                if request_sources.contains(&fun.source_path) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }
        }
        super::MaterializeRequestRootMode::EntryMain { fqn } => {
            for item in &lowered_hir.file.items {
                let crate::hir::Item::Fun(fun) = item else {
                    continue;
                };
                if !request_sources.contains(&fun.source_path) {
                    continue;
                }
                let is_entry_main = fqn.map_or(fun.name == "main", |entry| fun.fqn == entry);
                if is_entry_main || index.is_export_entry_point(&fun.fqn) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }
        }
    }

    out
}

fn collect_callable_signature_infos(
    lowered_hir: &crate::hir::LoweredHir,
) -> Vec<CallableSignatureInfo> {
    lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered_hir.member_funs.iter())
        .map(|fun| CallableSignatureInfo {
            template: TemplateKey {
                fqn: fun.fqn.clone(),
                source_path: fun.source_path.clone(),
                decl_span: fun.span,
            },
            fun_ty: fun.ty,
            return_ty: fun.return_ty,
            params: fun
                .params
                .iter()
                .map(|param| CallableSignatureParam {
                    name: param.name.clone(),
                    ty: param.ty,
                })
                .collect(),
        })
        .collect()
}

#[derive(Clone)]
struct HirDirectCallTemplateInfo {
    template: TemplateKey,
    type_param_names: Vec<String>,
    params: Vec<CallableSignatureParam>,
    has_effect_param: bool,
    has_body: bool,
}

fn collect_hir_direct_call_instance_requests(
    lowered_hir: &mut crate::hir::LoweredHir,
    typecheck_types: &TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
) -> HashMap<(PathBuf, Span), Vec<InstanceKey>> {
    let file_items = &lowered_hir.file.items;
    let member_funs = &lowered_hir.member_funs;
    let types = &mut lowered_hir.types;
    let mut templates_by_fqn: HashMap<String, Vec<HirDirectCallTemplateInfo>> = HashMap::new();
    for fun in file_items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(member_funs.iter())
    {
        let mut type_param_names = Vec::new();
        for param in &fun.params {
            collect_type_param_names_in_type(&*types, param.ty, &mut type_param_names);
        }
        collect_type_param_names_in_type(&*types, fun.return_ty, &mut type_param_names);
        let has_effect_param = function_type_has_effect_param(&*types, fun.ty);
        if type_param_names.is_empty() && !has_effect_param {
            continue;
        }
        let template = TemplateKey {
            fqn: fun.fqn.clone(),
            source_path: fun.source_path.clone(),
            decl_span: fun.span,
        };
        let entry = templates_by_fqn.entry(fun.fqn.clone()).or_default();
        if entry.iter().any(|existing| existing.template == template) {
            continue;
        }
        entry.push(HirDirectCallTemplateInfo {
            template,
            type_param_names,
            params: fun
                .params
                .iter()
                .map(|param| CallableSignatureParam {
                    name: param.name.clone(),
                    ty: param.ty,
                })
                .collect(),
            has_effect_param,
            has_body: fun.body.is_some(),
        });
    }

    let mut out = HashMap::new();
    for fun in file_items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(member_funs.iter())
    {
        let Some(body) = &fun.body else {
            continue;
        };
        let mut fun_instances = HashSet::new();
        collect_hir_direct_call_instances_in_block(
            body,
            &fun.source_path,
            &templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            &mut fun_instances,
        );
        if !fun_instances.is_empty() {
            out.insert(
                (fun.source_path.clone(), fun.span),
                fun_instances.into_iter().collect(),
            );
        }
    }

    out
}

fn collect_hir_direct_call_instances_in_block(
    block: &crate::hir::Block,
    source_path: &Path,
    templates_by_fqn: &HashMap<String, Vec<HirDirectCallTemplateInfo>>,
    typecheck_types: &TypeStore,
    types: &mut TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    out: &mut HashSet<InstanceKey>,
) {
    for stmt in &block.stmts {
        collect_hir_direct_call_instances_in_stmt(
            stmt,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        );
    }
}

fn collect_hir_direct_call_instances_in_stmt(
    stmt: &crate::hir::Stmt,
    source_path: &Path,
    templates_by_fqn: &HashMap<String, Vec<HirDirectCallTemplateInfo>>,
    typecheck_types: &TypeStore,
    types: &mut TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    out: &mut HashSet<InstanceKey>,
) {
    match &stmt.kind {
        crate::hir::StmtKind::Expr(expr) => collect_hir_direct_call_instances_in_expr(
            expr,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                collect_hir_direct_call_instances_in_expr(
                    init,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_hir_direct_call_instances_in_expr(
                lhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_expr(
                rhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::StmtKind::While { cond, body } => {
            collect_hir_direct_call_instances_in_expr(
                cond,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_block(
                body,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_hir_direct_call_instances_in_expr(
                    value,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::StmtKind::Empty
        | crate::hir::StmtKind::Break { .. }
        | crate::hir::StmtKind::Continue { .. }
        | crate::hir::StmtKind::Todo(_) => {}
    }
}

fn collect_hir_direct_call_instances_in_expr(
    expr: &crate::hir::Expr,
    source_path: &Path,
    templates_by_fqn: &HashMap<String, Vec<HirDirectCallTemplateInfo>>,
    typecheck_types: &TypeStore,
    types: &mut TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    out: &mut HashSet<InstanceKey>,
) {
    match &expr.kind {
        crate::hir::ExprKind::Call { callee, args } => {
            collect_hir_direct_call_instances_in_expr(
                callee,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            for arg in args {
                match arg {
                    crate::hir::CallArg::Positional(value) => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                    crate::hir::CallArg::Named { value, .. } => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                }
            }

            let crate::hir::ExprKind::VarRef(crate::hir::ValueRef::TopLevel { fqn, .. }) =
                &callee.kind
            else {
                return;
            };
            let binding = top_level_fun_call_bindings.get(&(source_path.to_path_buf(), expr.span));
            let Some(candidates) = binding
                .and_then(|binding| templates_by_fqn.get(&binding.fqn))
                .or_else(|| templates_by_fqn.get(fqn))
            else {
                return;
            };
            if let Some(binding) = binding {
                let candidate =
                    choose_hir_direct_call_template_for_binding(candidates, binding, &*types)
                        .or_else(|| choose_hir_direct_call_template(candidates, &*types));
                if let Some(candidate) = candidate {
                    let type_args = binding
                        .type_args
                        .iter()
                        .map(|&ty| types.re_intern_from(typecheck_types, ty))
                        .collect::<Vec<_>>();
                    let eff_args = binding
                        .eff_args
                        .iter()
                        .map(|row| re_intern_effect_row_from(types, typecheck_types, row))
                        .collect::<Vec<_>>();
                    if !type_args.is_empty() || !eff_args.is_empty() {
                        let instance = InstanceKey {
                            template: candidate.template.clone(),
                            type_args,
                            eff_args,
                        };
                        if instance_request_is_concrete(
                            types,
                            &instance.type_args,
                            &instance.eff_args,
                        ) {
                            out.insert(instance);
                        }
                    }
                    return;
                }
            }

            let Some(candidate) = choose_hir_direct_call_template(candidates, &*types) else {
                return;
            };
            if candidate.has_effect_param || candidate.type_param_names.is_empty() {
                return;
            }

            let Some(arg_to_param) = map_hir_call_args_to_signature_params(&candidate.params, args)
            else {
                return;
            };
            let mut bindings = HashMap::new();
            for (arg_idx, param_idx) in arg_to_param.into_iter().enumerate() {
                let Some(param) = candidate.params.get(param_idx) else {
                    return;
                };
                if !type_contains_param(types, param.ty) {
                    continue;
                }
                let arg_ty = match args.get(arg_idx) {
                    Some(crate::hir::CallArg::Positional(value)) => value.ty,
                    Some(crate::hir::CallArg::Named { value, .. }) => value.ty,
                    None => return,
                };
                if type_contains_param(types, arg_ty) {
                    return;
                }
                collect_type_param_bindings(types, param.ty, arg_ty, &mut bindings);
            }

            let mut ordered = Vec::with_capacity(candidate.type_param_names.len());
            for name in &candidate.type_param_names {
                let Some(ty) = bindings.get(name).copied() else {
                    return;
                };
                if type_contains_param(types, ty) {
                    return;
                }
                ordered.push(ty);
            }
            if ordered.is_empty() {
                return;
            }

            let instance = InstanceKey {
                template: candidate.template.clone(),
                type_args: ordered,
                eff_args: Vec::new(),
            };
            if instance_request_is_concrete(types, &instance.type_args, &instance.eff_args) {
                out.insert(instance);
            }
        }
        crate::hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_hir_direct_call_instances_in_expr(
                    &field.value,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_hir_direct_call_instances_in_expr(
                    element,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_hir_direct_call_instances_in_expr(
                        expr,
                        source_path,
                        templates_by_fqn,
                        typecheck_types,
                        types,
                        top_level_fun_call_bindings,
                        out,
                    );
                }
            }
        }
        crate::hir::ExprKind::Unary { expr, .. } => collect_hir_direct_call_instances_in_expr(
            expr,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_hir_direct_call_instances_in_expr(
                lhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_expr(
                rhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::ExprKind::TypeCheck { expr, .. } | crate::hir::ExprKind::Cast { expr, .. } => {
            collect_hir_direct_call_instances_in_expr(
                expr,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::ExprKind::Block(block) => collect_hir_direct_call_instances_in_block(
            block,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::ExprKind::Closure(closure) => collect_hir_direct_call_instances_in_expr(
            &closure.body,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_hir_direct_call_instances_in_expr(
                cond,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_expr(
                then_branch,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            if let Some(else_branch) = else_branch {
                collect_hir_direct_call_instances_in_expr(
                    else_branch,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::When { subject, arms } => {
            collect_hir_direct_call_instances_in_expr(
                subject,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_hir_direct_call_instances_in_expr(
                        guard,
                        source_path,
                        templates_by_fqn,
                        typecheck_types,
                        types,
                        top_level_fun_call_bindings,
                        out,
                    );
                }
                collect_hir_direct_call_instances_in_expr(
                    &arm.body,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_hir_direct_call_instances_in_expr(
                receiver,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            )
        }
        crate::hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    crate::hir::CallArg::Positional(value) => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                    crate::hir::CallArg::Named { value, .. } => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                }
            }
        }
        crate::hir::ExprKind::Handle(handle) => {
            collect_hir_direct_call_instances_in_block(
                &handle.body,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            for arm in &handle.arms {
                collect_hir_direct_call_instances_in_expr(
                    &arm.body,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
            if let Some(finally) = &handle.finally {
                collect_hir_direct_call_instances_in_block(
                    finally,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::Missing
        | crate::hir::ExprKind::Literal(_)
        | crate::hir::ExprKind::VarRef(_)
        | crate::hir::ExprKind::UnresolvedIdent { .. }
        | crate::hir::ExprKind::Todo(_) => {}
    }
}

fn collect_top_level_value_refs_in_block(block: &crate::hir::Block, out: &mut Vec<String>) {
    for stmt in &block.stmts {
        collect_top_level_value_refs_in_stmt(stmt, out);
    }
}

fn collect_top_level_value_refs_in_stmt(stmt: &crate::hir::Stmt, out: &mut Vec<String>) {
    match &stmt.kind {
        crate::hir::StmtKind::Expr(expr) => collect_top_level_value_refs_in_expr(expr, out),
        crate::hir::StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                collect_top_level_value_refs_in_expr(init, out);
            }
        }
        crate::hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_top_level_value_refs_in_expr(lhs, out);
            collect_top_level_value_refs_in_expr(rhs, out);
        }
        crate::hir::StmtKind::While { cond, body } => {
            collect_top_level_value_refs_in_expr(cond, out);
            collect_top_level_value_refs_in_block(body, out);
        }
        crate::hir::StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_top_level_value_refs_in_expr(value, out);
            }
        }
        crate::hir::StmtKind::Empty
        | crate::hir::StmtKind::Break { .. }
        | crate::hir::StmtKind::Continue { .. }
        | crate::hir::StmtKind::Todo(_) => {}
    }
}

fn collect_top_level_value_refs_in_args(args: &[crate::hir::CallArg], out: &mut Vec<String>) {
    for arg in args {
        match arg {
            crate::hir::CallArg::Positional(value) => {
                collect_top_level_value_refs_in_expr(value, out);
            }
            crate::hir::CallArg::Named { value, .. } => {
                collect_top_level_value_refs_in_expr(value, out);
            }
        }
    }
}

fn collect_top_level_value_refs_in_expr(expr: &crate::hir::Expr, out: &mut Vec<String>) {
    match &expr.kind {
        crate::hir::ExprKind::VarRef(crate::hir::ValueRef::TopLevel { fqn, .. }) => {
            out.push(fqn.clone());
        }
        crate::hir::ExprKind::Call { callee, args } => {
            collect_top_level_value_refs_in_expr(callee, out);
            collect_top_level_value_refs_in_args(args, out);
        }
        crate::hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_top_level_value_refs_in_expr(&field.value, out);
            }
        }
        crate::hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_top_level_value_refs_in_expr(element, out);
            }
        }
        crate::hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_top_level_value_refs_in_expr(expr, out);
                }
            }
        }
        crate::hir::ExprKind::Unary { expr, .. } => {
            collect_top_level_value_refs_in_expr(expr, out);
        }
        crate::hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_top_level_value_refs_in_expr(lhs, out);
            collect_top_level_value_refs_in_expr(rhs, out);
        }
        crate::hir::ExprKind::TypeCheck { expr, .. } | crate::hir::ExprKind::Cast { expr, .. } => {
            collect_top_level_value_refs_in_expr(expr, out);
        }
        crate::hir::ExprKind::Block(block) => collect_top_level_value_refs_in_block(block, out),
        crate::hir::ExprKind::Closure(closure) => {
            collect_top_level_value_refs_in_expr(&closure.body, out);
        }
        crate::hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_top_level_value_refs_in_expr(cond, out);
            collect_top_level_value_refs_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_top_level_value_refs_in_expr(else_branch, out);
            }
        }
        crate::hir::ExprKind::When { subject, arms } => {
            collect_top_level_value_refs_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_top_level_value_refs_in_expr(guard, out);
                }
                collect_top_level_value_refs_in_expr(&arm.body, out);
            }
        }
        crate::hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_top_level_value_refs_in_expr(receiver, out);
        }
        crate::hir::ExprKind::Perform { args, .. } => {
            collect_top_level_value_refs_in_args(args, out);
        }
        crate::hir::ExprKind::Handle(handle) => {
            collect_top_level_value_refs_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_top_level_value_refs_in_expr(&arm.body, out);
            }
            if let Some(finally) = &handle.finally {
                collect_top_level_value_refs_in_block(finally, out);
            }
        }
        crate::hir::ExprKind::Missing
        | crate::hir::ExprKind::Literal(_)
        | crate::hir::ExprKind::VarRef(_)
        | crate::hir::ExprKind::UnresolvedIdent { .. }
        | crate::hir::ExprKind::Todo(_) => {}
    }
}

fn choose_hir_direct_call_template_for_binding<'a>(
    candidates: &'a [HirDirectCallTemplateInfo],
    binding: &ast::TopLevelFunCallBinding,
    types: &TypeStore,
) -> Option<&'a HirDirectCallTemplateInfo> {
    let chosen = candidates.iter().find(|candidate| {
        candidate.template.source_path == binding.decl_file
            && candidate.template.decl_span == binding.decl_span
    })?;
    let mut preferred = candidates
        .iter()
        .filter(|candidate| {
            hir_direct_call_templates_have_same_signature(candidate, chosen, types)
                && candidate.has_body
        })
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter().filter(|candidate| {
            hir_direct_call_templates_have_same_signature(candidate, chosen, types)
        }));
    }
    preferred.into_iter().min_by(|lhs, rhs| {
        lhs.template
            .source_path
            .cmp(&rhs.template.source_path)
            .then_with(|| {
                lhs.template
                    .decl_span
                    .start
                    .cmp(&rhs.template.decl_span.start)
            })
            .then_with(|| lhs.template.decl_span.end.cmp(&rhs.template.decl_span.end))
    })
}

fn hir_direct_call_templates_have_same_signature(
    lhs: &HirDirectCallTemplateInfo,
    rhs: &HirDirectCallTemplateInfo,
    types: &TypeStore,
) -> bool {
    lhs.type_param_names == rhs.type_param_names
        && lhs.has_effect_param == rhs.has_effect_param
        && lhs.params.len() == rhs.params.len()
        && lhs.params.iter().zip(rhs.params.iter()).all(|(lhs, rhs)| {
            lhs.name == rhs.name
                && types.display(lhs.ty).to_string() == types.display(rhs.ty).to_string()
        })
}

fn map_hir_call_args_to_signature_params(
    params: &[CallableSignatureParam],
    args: &[crate::hir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg {
            crate::hir::CallArg::Named { name, .. } => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == *name).then_some(idx))?,
            crate::hir::CallArg::Positional(_) => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    Some(out)
}

fn choose_hir_direct_call_template<'a>(
    candidates: &'a [HirDirectCallTemplateInfo],
    types: &TypeStore,
) -> Option<&'a HirDirectCallTemplateInfo> {
    let first = candidates.first()?;
    let same_signature = candidates
        .iter()
        .skip(1)
        .all(|candidate| hir_direct_call_templates_have_same_signature(candidate, first, types));
    if !same_signature {
        return None;
    }

    let mut preferred = candidates
        .iter()
        .filter(|candidate| candidate.has_body)
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter());
    }
    preferred.into_iter().min_by(|lhs, rhs| {
        lhs.template
            .source_path
            .cmp(&rhs.template.source_path)
            .then_with(|| {
                lhs.template
                    .decl_span
                    .start
                    .cmp(&rhs.template.decl_span.start)
            })
            .then_with(|| lhs.template.decl_span.end.cmp(&rhs.template.decl_span.end))
    })
}

fn collect_dump_materialization_inputs(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<DumpMaterializationInputs> {
    let mut prepared_files = Vec::with_capacity(session.sysroot().files.len() + 8);
    for file in &session.sysroot().files {
        prepared_files.push(PreparedDumpFile {
            source: file.source.clone(),
            ast: file.ast.clone(),
            extend_type_env: false,
            collect_monomorph_keys: false,
        });
    }

    for support_source in load_dump_support_sources(session)? {
        let ast = parse_file(&support_source)?;
        prepared_files.push(PreparedDumpFile {
            source: support_source,
            ast,
            extend_type_env: true,
            collect_monomorph_keys: false,
        });
    }

    let entry_source = source.clone();
    let entry_ast = parse_file(&entry_source)?;
    prepared_files.push(PreparedDumpFile {
        source: entry_source,
        ast: entry_ast,
        extend_type_env: true,
        collect_monomorph_keys: true,
    });

    {
        let trim_sources = prepared_files
            .iter()
            .filter(|file| file.extend_type_env)
            .map(|file| file.source.clone())
            .collect::<Vec<_>>();
        let sources = trim_sources.iter().collect::<Vec<_>>();
        let mut files = prepared_files
            .iter_mut()
            .filter(|file| file.extend_type_env)
            .map(|file| &mut file.ast)
            .collect::<Vec<_>>();
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }

    for file in &prepared_files {
        typecheck::check_file_headers(&file.source, &file.ast)?;
        typecheck::check_file_struct_decls(&file.source, &file.ast)?;
    }

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::with_capacity(prepared_files.len());
        for file in &prepared_files {
            pairs.push((&file.source, &file.ast));
        }
        Index::build(&pairs)?
    };

    let mut resolved_headers = Vec::with_capacity(prepared_files.len());
    for file in &prepared_files {
        resolved_headers.push(crate::resolve::check_file_headers(
            &file.source,
            &file.ast,
            &index,
        )?);
    }
    for (file, headers) in prepared_files.iter_mut().zip(resolved_headers.iter()) {
        crate::resolve::check_file_bodies(&file.source, &mut file.ast, &index, headers)?;
    }

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)?;
    for file in &prepared_files {
        if file.extend_type_env {
            env.extend_from_file(&file.source, &file.ast, &index)?;
        }
    }

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut monomorph_requests = Vec::new();
    for (file, headers) in prepared_files.iter().zip(resolved_headers.iter()) {
        typecheck::check_file_annotations(
            &file.source,
            &file.ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )?;
        typecheck::check_file_type_refs(
            &file.source,
            &file.ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )?;

        if file.collect_monomorph_keys {
            monomorph_requests.extend(typecheck::check_file_exprs_with_monomorph_requests(
                &file.source,
                &file.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )?);
        } else {
            typecheck::check_file_exprs(
                &file.source,
                &file.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )?;
        }
    }

    Ok(DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types: types,
        monomorph_requests,
    })
}

fn collect_site_instance_bindings(
    files_to_lower: &[(&SourceFile, &ast::File)],
) -> (
    HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
    HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
) {
    let mut top_level_fun_value_refs = HashMap::new();
    let mut top_level_fun_call_bindings = HashMap::new();
    for (source, file) in files_to_lower {
        let source_path = source.path().to_path_buf();
        for (span, binding) in file.top_level_fun_value_refs() {
            top_level_fun_value_refs.insert((source_path.clone(), span), binding);
        }
        for (span, binding) in file.top_level_fun_call_bindings() {
            top_level_fun_call_bindings.insert((source_path.clone(), span), binding);
        }
    }
    (top_level_fun_value_refs, top_level_fun_call_bindings)
}

type RequestTemplateKey = (String, PathBuf, Span);

#[derive(Clone)]
struct GenericTemplateInfo {
    request_lookup_key: RequestTemplateKey,
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    signature_key: String,
    has_body: bool,
}

fn normalize_sig_piece(s: &str) -> String {
    s.split_whitespace().collect()
}

fn generic_template_signature_key_with_owner_params(
    source: &SourceFile,
    owner_type_param_names: &[String],
    fun: &ast::FunDecl,
) -> String {
    let mut out = String::new();
    out.push_str(match fun.kind {
        ast::FunDeclKind::Regular => "fun",
        ast::FunDeclKind::EffectOp => "effect-op",
    });
    out.push('|');
    for param in owner_type_param_names {
        out.push_str(param);
        out.push(',');
    }
    out.push('|');
    for param in &fun.type_params {
        out.push_str(param.name.text(source));
        out.push(',');
    }
    out.push('|');
    if let Some(eff) = &fun.eff_param {
        out.push_str(&normalize_sig_piece(source.slice(eff.span)));
    }
    out.push('|');
    if let Some(receiver) = &fun.receiver {
        out.push_str(&normalize_sig_piece(source.slice(receiver.span())));
    }
    out.push('|');
    for param in &fun.params {
        if let Some(ty) = &param.ty {
            out.push_str(&normalize_sig_piece(source.slice(ty.span())));
        } else {
            out.push('_');
        }
        out.push(';');
    }
    out.push('|');
    match &fun.return_ty {
        Some(ret) => out.push_str(&normalize_sig_piece(source.slice(ret.span()))),
        None => out.push_str("Unit"),
    }
    out.push('|');
    if let Some(effects) = &fun.effects {
        out.push_str(&normalize_sig_piece(source.slice(effects.span)));
    }
    out
}

fn push_generic_template_info(
    out: &mut Vec<GenericTemplateInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    owner_type_param_names: &[String],
    fun: &ast::FunDecl,
) {
    if owner_type_param_names.is_empty() && fun.type_params.is_empty() && fun.eff_param.is_none() {
        return;
    }

    let local_name = source.slice(fun.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(GenericTemplateInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), fun.name.span),
        template: TemplateKey {
            fqn,
            source_path: source.path().to_path_buf(),
            decl_span: fun.span,
        },
        type_param_names: owner_type_param_names
            .iter()
            .cloned()
            .chain(
                fun.type_params
                    .iter()
                    .map(|param| param.name.text(source).to_string()),
            )
            .collect(),
        eff_param_name: fun
            .eff_param
            .as_ref()
            .map(|param| param.name.text(source).to_string()),
        signature_key: generic_template_signature_key_with_owner_params(
            source,
            owner_type_param_names,
            fun,
        ),
        has_body: matches!(fun.body, ast::FunBody::Block(_)),
    });
}

fn generic_value_property_getter_signature_key(
    source: &SourceFile,
    owner_type_param_names: &[String],
    property: &ast::PropertyDecl,
) -> String {
    let mut out = String::from("value-getter|");
    for param in owner_type_param_names {
        out.push_str(param);
        out.push(',');
    }
    out.push('|');
    match &property.ty {
        Some(ret) => out.push_str(&normalize_sig_piece(source.slice(ret.span()))),
        None => out.push_str("Any"),
    }
    out
}

fn push_generic_value_property_getter_template_info(
    out: &mut Vec<GenericTemplateInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    owner_type_param_names: &[String],
    property: &ast::PropertyDecl,
) {
    if owner_type_param_names.is_empty() || property.getter.is_none() {
        return;
    }

    let local_name = source.slice(property.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(GenericTemplateInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), property.name.span),
        template: TemplateKey {
            fqn,
            source_path: source.path().to_path_buf(),
            decl_span: property.span,
        },
        type_param_names: owner_type_param_names.to_vec(),
        eff_param_name: None,
        signature_key: generic_value_property_getter_signature_key(
            source,
            owner_type_param_names,
            property,
        ),
        has_body: property
            .getter
            .as_ref()
            .is_some_and(|getter| !matches!(getter.body, ast::AccessorBody::Missing)),
    });
}

fn collect_generic_templates_from_type_body(
    out: &mut Vec<GenericTemplateInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    owner_type_param_names: &[String],
    owner_kind: Option<ast::TypeKind>,
    body: Option<&ast::TypeBody>,
) {
    let Some(body) = body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => {
                push_generic_template_info(out, source, owner_fqn, owner_type_param_names, fun)
            }
            ast::TypeMember::Property(property)
                if matches!(
                    owner_kind,
                    Some(ast::TypeKind::Struct | ast::TypeKind::Enum)
                ) =>
            {
                push_generic_value_property_getter_template_info(
                    out,
                    source,
                    owner_fqn,
                    owner_type_param_names,
                    property,
                );
            }
            ast::TypeMember::Type(ty) => {
                let nested_owner = format!("{owner_fqn}.{}", ty.name.text(source));
                let nested_owner_type_param_names = ty
                    .type_params
                    .iter()
                    .map(|param| param.name.text(source).to_string())
                    .collect::<Vec<_>>();
                collect_generic_templates_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    &nested_owner_type_param_names,
                    Some(ty.kind),
                    ty.body.as_ref(),
                );
            }
            ast::TypeMember::Object(obj) => {
                let object_name = obj
                    .name
                    .as_ref()
                    .map(|name| name.text(source).to_string())
                    .or_else(|| {
                        matches!(obj.kind, ast::ObjectKind::Companion)
                            .then(|| "Companion".to_string())
                    });
                let Some(object_name) = object_name else {
                    continue;
                };
                let nested_owner = format!("{owner_fqn}.{object_name}");
                collect_generic_templates_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    &[],
                    None,
                    obj.body.as_ref(),
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Property(_) => {}
        }
    }
}

fn collect_generic_template_infos(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Vec<GenericTemplateInfo> {
    let mut out = Vec::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    push_generic_template_info(&mut out, source, &pkg_prefix, &[], fun);
                }
                ast::Item::Type(ty) => {
                    let owner_fqn = if pkg_prefix.is_empty() {
                        ty.name.text(source).to_string()
                    } else {
                        format!("{pkg_prefix}.{}", ty.name.text(source))
                    };
                    let owner_type_param_names = ty
                        .type_params
                        .iter()
                        .map(|param| param.name.text(source).to_string())
                        .collect::<Vec<_>>();
                    collect_generic_templates_from_type_body(
                        &mut out,
                        source,
                        &owner_fqn,
                        &owner_type_param_names,
                        Some(ty.kind),
                        ty.body.as_ref(),
                    );
                }
                ast::Item::Object(obj) => {
                    let object_name = obj
                        .name
                        .as_ref()
                        .map(|name| name.text(source).to_string())
                        .or_else(|| {
                            matches!(obj.kind, ast::ObjectKind::Companion)
                                .then(|| "Companion".to_string())
                        });
                    let Some(object_name) = object_name else {
                        continue;
                    };
                    let owner_fqn = if pkg_prefix.is_empty() {
                        object_name
                    } else {
                        format!("{pkg_prefix}.{object_name}")
                    };
                    collect_generic_templates_from_type_body(
                        &mut out,
                        source,
                        &owner_fqn,
                        &[],
                        None,
                        obj.body.as_ref(),
                    );
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Val(_) => {}
            }
        }
    }
    out
}

fn push_callable_fun_body_info(
    out: &mut Vec<CallableBodyInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    fun: &ast::FunDecl,
) {
    if !matches!(fun.body, ast::FunBody::Block(_)) {
        return;
    }

    let local_name = source.slice(fun.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(CallableBodyInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), fun.name.span),
        source_path: source.path().to_path_buf(),
        fqn,
        body_span: fun.span,
    });
}

fn push_callable_property_getter_body_info(
    out: &mut Vec<CallableBodyInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    property: &ast::PropertyDecl,
) {
    let Some(getter) = property.getter.as_ref() else {
        return;
    };
    if matches!(getter.body, ast::AccessorBody::Missing) {
        return;
    }

    let local_name = source.slice(property.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(CallableBodyInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), property.name.span),
        source_path: source.path().to_path_buf(),
        fqn,
        body_span: property.span,
    });
}

fn collect_callable_body_infos_from_type_body(
    out: &mut Vec<CallableBodyInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    body: Option<&ast::TypeBody>,
) {
    let Some(body) = body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => push_callable_fun_body_info(out, source, owner_fqn, fun),
            ast::TypeMember::Property(property) => {
                push_callable_property_getter_body_info(out, source, owner_fqn, property);
            }
            ast::TypeMember::Type(ty) => {
                let nested_owner = format!("{owner_fqn}.{}", ty.name.text(source));
                collect_callable_body_infos_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    ty.body.as_ref(),
                );
            }
            ast::TypeMember::Object(obj) => {
                let object_name = obj
                    .name
                    .as_ref()
                    .map(|name| name.text(source).to_string())
                    .or_else(|| {
                        matches!(obj.kind, ast::ObjectKind::Companion)
                            .then(|| "Companion".to_string())
                    });
                let Some(object_name) = object_name else {
                    continue;
                };
                let nested_owner = format!("{owner_fqn}.{object_name}");
                collect_callable_body_infos_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    obj.body.as_ref(),
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

fn collect_callable_body_infos(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Vec<CallableBodyInfo> {
    let mut out = Vec::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    push_callable_fun_body_info(&mut out, source, &pkg_prefix, fun);
                }
                ast::Item::Type(ty) => {
                    let owner_fqn = if pkg_prefix.is_empty() {
                        ty.name.text(source).to_string()
                    } else {
                        format!("{pkg_prefix}.{}", ty.name.text(source))
                    };
                    collect_callable_body_infos_from_type_body(
                        &mut out,
                        source,
                        &owner_fqn,
                        ty.body.as_ref(),
                    );
                }
                ast::Item::Object(obj) => {
                    let object_name = obj
                        .name
                        .as_ref()
                        .map(|name| name.text(source).to_string())
                        .or_else(|| {
                            matches!(obj.kind, ast::ObjectKind::Companion)
                                .then(|| "Companion".to_string())
                        });
                    let Some(object_name) = object_name else {
                        continue;
                    };
                    let owner_fqn = if pkg_prefix.is_empty() {
                        object_name
                    } else {
                        format!("{pkg_prefix}.{object_name}")
                    };
                    collect_callable_body_infos_from_type_body(
                        &mut out,
                        source,
                        &owner_fqn,
                        obj.body.as_ref(),
                    );
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Val(_) => {}
            }
        }
    }
    out
}

fn load_dump_support_sources(session: &Session) -> MaterializeResult<Vec<SourceFile>> {
    let stdlib_root = default_stdlib_path();
    let stdlib_root = stdlib_root.canonicalize().map_err(|error| {
        frontend_err(format!(
            "dump-ir 无法定位 stdlib 目录：{}: {error}",
            stdlib_root.display()
        ))
    })?;

    let mut paths = Vec::new();
    collect_scoop_files(&stdlib_root, &mut paths)?;
    paths.extend(session.sysroot().compilable_source_paths.iter().cloned());
    paths.sort();

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let source = SourceFile::load(&path).map_err(|error| {
            frontend_err(format!(
                "dump-ir 无法读取 sysroot support source：{}: {error}",
                path.display()
            ))
        })?;
        sources.push(source);
    }
    Ok(sources)
}

fn default_stdlib_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> MaterializeResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|error| {
        frontend_err(format!("dump-ir 无法读取目录：{}: {error}", dir.display()))
    })? {
        let entry = entry.map_err(|error| frontend_err(error.to_string()))?;
        let path = entry.path();
        let ty = entry
            .file_type()
            .map_err(|error| frontend_err(error.to_string()))?;
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

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(package) = package else {
        return String::new();
    };
    package
        .path
        .iter()
        .map(|seg| seg.text(source))
        .collect::<Vec<_>>()
        .join(".")
}

fn materialize_generic_mir(
    generic_file: File,
    types: TypeStore,
    builtins: BuiltinTypes,
    requests: MaterializeRequestSet<'_>,
    opt_level: OptLevel,
) -> MaterializeResult<MaterializedMir> {
    let MaterializeRequestSet {
        monomorph_requests,
        hir_direct_instance_keys_by_fun,
        construction_inputs,
    } = requests;
    let typecheck_types = construction_inputs.typecheck_types;
    let mut materializer = MirInstanceMaterializer::new(
        generic_file,
        types,
        builtins,
        construction_inputs,
        opt_level,
        opt_level.enables_summary_driven_mir_inlining(),
        opt_level.enables_mir_escape_analysis(),
    )?;
    materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;
    let initial_requests = materializer.seed_requests(typecheck_types, monomorph_requests)?;
    materializer.run(initial_requests)
}

#[derive(Clone)]
struct TemplateRootInfo {
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    family: Vec<FunDecl>,
}

#[derive(Clone)]
struct TemplateRootCandidate {
    request_lookup_key: RequestTemplateKey,
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    signature_key: String,
    root_fun: FunDecl,
}

#[derive(Clone)]
struct DeclOnlyTemplateCandidate {
    request_lookup_key: RequestTemplateKey,
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    signature_key: String,
    signature: CallableSignatureInfo,
}

#[derive(Clone)]
struct TemplateCatalogCandidate {
    template: TemplateKey,
    signature_key: String,
    prefers_materialized_body: bool,
}

#[derive(Clone)]
struct TemplateSignatureInfo {
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    fun_ty: TypeId,
    return_ty: TypeId,
    params: Vec<CallableSignatureParam>,
}

#[derive(Clone)]
struct SiteInstanceBinding {
    template: TemplateKey,
    type_args: Vec<TypeId>,
    eff_args: Vec<EffectRow>,
}

#[derive(Default)]
struct InstanceSubstitution {
    type_params: HashMap<String, TypeId>,
    effect_params: HashMap<String, EffectRow>,
}

struct RewriteContext<'a> {
    locals: &'a [LocalDecl],
    substitution: &'a InstanceSubstitution,
    template_source_path: &'a Path,
    template_root_fqn: &'a str,
    instance_root_fqn: &'a str,
}

#[derive(Clone)]
struct ReachableMirFun {
    source_path: PathBuf,
    fun: FunDecl,
}

#[derive(Clone)]
struct PassPublishedOrdinaryCallable {
    source_path: PathBuf,
    fun: FunDecl,
}

fn dispatch_direct_call_args(
    call_span: Span,
    receiver: &Operand,
    args: &[CallArg],
) -> Vec<CallArg> {
    let mut direct_args = Vec::with_capacity(args.len() + 1);
    direct_args.push(CallArg {
        span: call_span,
        name: None,
        value: receiver.clone(),
    });
    direct_args.extend(args.iter().cloned());
    direct_args
}

fn reachable_body_block_indices(body: &Body) -> Vec<usize> {
    match body.reachable_blocks() {
        Ok(blocks) => blocks
            .into_iter()
            .map(|block| block.as_u32() as usize)
            .collect(),
        Err(_) => (0..body.blocks.len()).collect(),
    }
}

struct MirInstanceMaterializer {
    types: TypeStore,
    builtins: BuiltinTypes,
    opt_level: OptLevel,
    known_receiver_subclasses: crate::devirtualize::KnownReceiverSubclassIndex,
    class_vtables: crate::vtable::ClassVtableIndex,
    interfaces: crate::itable::InterfaceIndex,
    class_itables: crate::itable::ClassItableIndex,
    request_root_funs: Vec<ReachableMirFun>,
    hir_direct_instance_keys_by_fun: HashMap<(PathBuf, Span), Vec<InstanceKey>>,
    generic_family_fqns: HashSet<String>,
    request_templates: HashMap<RequestTemplateKey, TemplateKey>,
    roots: HashMap<TemplateKey, TemplateRootInfo>,
    template_signatures: HashMap<TemplateKey, TemplateSignatureInfo>,
    template_symbol_suffixes: HashMap<TemplateKey, String>,
    roots_by_fqn: HashMap<String, Vec<TemplateKey>>,
    direct_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    top_level_immutable_values: crate::hir::TopLevelImmutableValueIndex,
    request_sources: HashSet<PathBuf>,
    filter_initial_requests_to_reachable_call_sites: bool,
    reachable_fun_bodies_by_request: HashMap<RequestTemplateKey, ReachableMirFun>,
    reachable_fun_bodies_by_fqn: HashMap<String, Vec<ReachableMirFun>>,
    all_fun_bodies_by_fqn: HashMap<String, Vec<FunDecl>>,
    call_bindings: HashMap<SourceSiteKey, SiteInstanceBinding>,
    value_ref_bindings: HashMap<SourceSiteKey, SiteInstanceBinding>,
    reachable_request_call_sites: HashSet<SourceSiteKey>,
    reachable_request_stmt_spans: Vec<(PathBuf, Span)>,
    scanned_top_level_immutable_values: HashSet<String>,
    scanned_non_generic_funs: HashSet<(PathBuf, Span)>,
    caller_side_pass_candidates: Vec<FunDecl>,
    pass_published_ordinary_callables: Vec<PassPublishedOrdinaryCallable>,
    enable_summary_driven_inlining: bool,
    enable_mir_escape_analysis: bool,
    queued: HashSet<InstanceKey>,
    queue: VecDeque<InstanceKey>,
    materialized: HashMap<InstanceKey, Vec<FunDecl>>,
    declaration_only_instances: HashSet<InstanceKey>,
}

struct ReachableRvalueScanContext<'a> {
    span: Span,
    result_ty: Option<TypeId>,
    template_source_path: &'a Path,
    locals: &'a [LocalDecl],
    substitution: &'a InstanceSubstitution,
}

struct DirectCallInferenceInput<'a> {
    template_source_path: &'a Path,
    call_span: Span,
    callee_fqn: &'a str,
    args: &'a [CallArg],
    result_ty: Option<TypeId>,
    locals: &'a [LocalDecl],
    substitution: &'a InstanceSubstitution,
}

impl MirInstanceMaterializer {
    fn new(
        generic_file: File,
        types: TypeStore,
        builtins: BuiltinTypes,
        construction_inputs: MaterializerConstructionInputs<'_>,
        opt_level: OptLevel,
        enable_summary_driven_inlining: bool,
        enable_mir_escape_analysis: bool,
    ) -> MaterializeResult<Self> {
        let MaterializerConstructionInputs {
            typecheck_types,
            template_infos,
            callable_body_infos,
            callable_signatures,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            top_level_fun_value_refs,
            top_level_fun_call_bindings,
            top_level_immutable_values,
            request_sources,
            request_root_mode,
            request_root_fun_keys,
        } = construction_inputs;
        let mut generic_funs = Vec::new();
        for item in &generic_file.items {
            if let Item::Fun(fun) = item {
                generic_funs.push(fun.clone());
            }
        }
        let callable_signatures = callable_signatures
            .into_iter()
            .map(|signature| (signature.template.clone(), signature))
            .collect::<HashMap<_, _>>();

        let mut root_candidates = Vec::new();
        let mut decl_only_candidates = Vec::new();
        let mut canonical_candidates = Vec::new();
        for info in template_infos {
            let template = info.template.clone();
            let root_fun = generic_funs
                .iter()
                .find(|fun| fun.fqn == template.fqn && fun.span == template.decl_span)
                .cloned();
            let Some(root_fun) = root_fun else {
                if !info.has_body {
                    let Some(signature) = callable_signatures.get(&template).cloned() else {
                        return Err(frontend_err(format!(
                            "materialize 无法定位 declaration-only generic template 的 HIR 签名：{}@{}:{:?}",
                            template.fqn,
                            template.source_path.display(),
                            template.decl_span
                        )));
                    };
                    canonical_candidates.push(TemplateCatalogCandidate {
                        template: template.clone(),
                        signature_key: info.signature_key.clone(),
                        prefers_materialized_body: false,
                    });
                    decl_only_candidates.push(DeclOnlyTemplateCandidate {
                        request_lookup_key: info.request_lookup_key,
                        template,
                        type_param_names: info.type_param_names,
                        eff_param_name: info.eff_param_name,
                        signature_key: info.signature_key,
                        signature,
                    });
                    continue;
                }
                return Err(materialize_err(
                    MirMaterializeError::MissingMirRootForTemplate {
                        fqn: template.fqn.clone(),
                        file: template.source_path.display().to_string(),
                        span: template.decl_span,
                    },
                ));
            };

            canonical_candidates.push(TemplateCatalogCandidate {
                template: template.clone(),
                signature_key: info.signature_key.clone(),
                prefers_materialized_body: root_fun.body.is_some(),
            });
            root_candidates.push(TemplateRootCandidate {
                request_lookup_key: info.request_lookup_key,
                template,
                type_param_names: info.type_param_names,
                eff_param_name: info.eff_param_name,
                signature_key: info.signature_key,
                root_fun,
            });
        }

        let canonical_templates = canonical_template_map(&canonical_candidates);

        let mut request_templates = HashMap::new();
        let mut roots = HashMap::new();
        let mut template_signatures = HashMap::new();
        let mut canonical_signature_keys = HashMap::new();
        for candidate in root_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every root candidate");
            request_templates.insert(candidate.request_lookup_key, canonical.clone());
            canonical_signature_keys
                .entry(canonical.clone())
                .or_insert_with(|| candidate.signature_key.clone());

            if candidate.template != canonical || roots.contains_key(&canonical) {
                continue;
            }

            let family = generic_funs
                .iter()
                .filter(|fun| belongs_to_template_family(fun, &candidate.root_fun))
                .cloned()
                .collect::<Vec<_>>();
            template_signatures.insert(
                canonical.clone(),
                TemplateSignatureInfo {
                    template: canonical.clone(),
                    type_param_names: candidate.type_param_names.clone(),
                    eff_param_name: candidate.eff_param_name.clone(),
                    fun_ty: candidate.root_fun.ty,
                    return_ty: candidate.root_fun.return_ty,
                    params: candidate
                        .root_fun
                        .params
                        .iter()
                        .map(|param| CallableSignatureParam {
                            name: param.name.clone(),
                            ty: param.ty,
                        })
                        .collect(),
                },
            );
            roots.insert(
                canonical.clone(),
                TemplateRootInfo {
                    template: canonical,
                    type_param_names: candidate.type_param_names,
                    eff_param_name: candidate.eff_param_name,
                    family,
                },
            );
        }

        for candidate in decl_only_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every decl-only candidate");
            request_templates.insert(candidate.request_lookup_key, canonical.clone());
            canonical_signature_keys
                .entry(canonical.clone())
                .or_insert_with(|| candidate.signature_key.clone());

            if candidate.template != canonical || template_signatures.contains_key(&canonical) {
                continue;
            }

            template_signatures.insert(
                canonical.clone(),
                TemplateSignatureInfo {
                    template: canonical,
                    type_param_names: candidate.type_param_names,
                    eff_param_name: candidate.eff_param_name,
                    fun_ty: candidate.signature.fun_ty,
                    return_ty: candidate.signature.return_ty,
                    params: candidate.signature.params,
                },
            );
        }

        let template_symbol_suffixes = build_template_symbol_suffixes(&canonical_signature_keys);
        let mut roots_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
        for template in template_signatures.keys() {
            roots_by_fqn
                .entry(template.fqn.clone())
                .or_default()
                .push(template.clone());
        }

        let request_root_funs = request_root_fun_keys
            .into_iter()
            .filter_map(|key| {
                generic_funs
                    .iter()
                    .find(|fun| fun.fqn == key.fqn && fun.span == key.span)
                    .cloned()
                    .map(|fun| ReachableMirFun {
                        source_path: key.source_path,
                        fun,
                    })
            })
            .collect::<Vec<_>>();

        let generic_family_fqns = roots
            .values()
            .flat_map(|root| root.family.iter().map(|fun| fun.fqn.clone()))
            .collect::<HashSet<_>>();

        let mut reachable_fun_bodies_by_request = HashMap::new();
        let mut reachable_fun_bodies_by_fqn: HashMap<String, Vec<ReachableMirFun>> = HashMap::new();
        let mut all_fun_bodies_by_fqn: HashMap<String, Vec<FunDecl>> = HashMap::new();
        for info in callable_body_infos {
            let Some(fun) = generic_funs
                .iter()
                .find(|fun| fun.fqn == info.fqn && fun.span == info.body_span)
                .cloned()
            else {
                continue;
            };
            let reachable = ReachableMirFun {
                source_path: info.source_path.clone(),
                fun,
            };
            reachable_fun_bodies_by_request.insert(info.request_lookup_key, reachable.clone());
            reachable_fun_bodies_by_fqn
                .entry(reachable.fun.fqn.clone())
                .or_default()
                .push(reachable);
        }
        for fun in &generic_funs {
            let Some(_) = &fun.body else {
                continue;
            };
            let entry = all_fun_bodies_by_fqn.entry(fun.fqn.clone()).or_default();
            if entry.iter().any(|existing| existing.span == fun.span) {
                continue;
            }
            entry.push(fun.clone());
        }

        let mut materializer = Self {
            types,
            builtins,
            opt_level,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            request_root_funs,
            hir_direct_instance_keys_by_fun: HashMap::new(),
            generic_family_fqns,
            request_templates,
            roots,
            template_signatures,
            template_symbol_suffixes,
            roots_by_fqn,
            direct_call_bindings: top_level_fun_call_bindings.clone(),
            top_level_immutable_values,
            request_sources,
            filter_initial_requests_to_reachable_call_sites: matches!(
                request_root_mode,
                super::MaterializeRequestRootMode::EntryMain { .. }
            ),
            reachable_fun_bodies_by_request,
            reachable_fun_bodies_by_fqn,
            all_fun_bodies_by_fqn,
            call_bindings: HashMap::new(),
            value_ref_bindings: HashMap::new(),
            reachable_request_call_sites: HashSet::new(),
            reachable_request_stmt_spans: Vec::new(),
            scanned_top_level_immutable_values: HashSet::new(),
            scanned_non_generic_funs: HashSet::new(),
            caller_side_pass_candidates: Vec::new(),
            pass_published_ordinary_callables: Vec::new(),
            enable_summary_driven_inlining,
            enable_mir_escape_analysis,
            queued: HashSet::new(),
            queue: VecDeque::new(),
            materialized: HashMap::new(),
            declaration_only_instances: HashSet::new(),
        };
        materializer.load_site_instance_bindings(
            typecheck_types,
            top_level_fun_value_refs,
            top_level_fun_call_bindings,
        )?;
        Ok(materializer)
    }

    fn load_site_instance_bindings(
        &mut self,
        typecheck_types: &TypeStore,
        top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
        top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    ) -> MaterializeResult<()> {
        for (site, binding) in top_level_fun_call_bindings {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                    },
                ));
            };
            let type_args = binding
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = binding
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            self.call_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }

        for (site, binding) in top_level_fun_value_refs {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                    },
                ));
            };
            let type_args = binding
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = binding
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            self.value_ref_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }

        Ok(())
    }

    fn resolve_request_template(
        &self,
        fqn: &str,
        decl_file: &Path,
        decl_span: Span,
    ) -> Option<TemplateKey> {
        self.request_templates
            .get(&(fqn.to_string(), decl_file.to_path_buf(), decl_span))
            .cloned()
            .or_else(|| {
                let matches = self
                    .request_templates
                    .iter()
                    .filter(|((candidate_fqn, candidate_file, _), _)| {
                        candidate_fqn == fqn && candidate_file == decl_file
                    })
                    .map(|(_, template)| template.clone())
                    .collect::<HashSet<_>>();
                (matches.len() == 1).then(|| matches.into_iter().next().unwrap())
            })
    }

    fn seed_requests(
        &mut self,
        typecheck_types: &TypeStore,
        monomorph_requests: &[MonomorphRequest],
    ) -> MaterializeResult<Vec<InstanceKey>> {
        let request_root_instances = self.seed_request_root_direct_call_instances()?;
        let mut initial = Vec::new();
        for request in monomorph_requests {
            if !self.request_sources.contains(&request.request_source_path) {
                continue;
            }
            if !self.monomorph_request_is_reachable_initial_seed(request) {
                continue;
            }
            let key = &request.key;
            let Some(template) = self.resolve_request_template(
                &key.symbol.fqn,
                &key.symbol.decl_file,
                key.symbol.decl_span,
            ) else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: key.symbol.fqn.clone(),
                        file: key.symbol.decl_file.display().to_string(),
                        span: key.symbol.decl_span,
                    },
                ));
            };

            if key.type_args.is_empty() && key.eff_args.is_empty() {
                continue;
            }
            let type_args = key
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect::<Vec<_>>();
            let eff_args = key
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect::<Vec<_>>();
            if !instance_request_is_concrete(&self.types, &type_args, &eff_args) {
                continue;
            }
            initial.push(InstanceKey {
                template,
                type_args,
                eff_args,
            });
        }
        initial.extend(request_root_instances);
        initial.sort_by_key(|a| self.instance_fqn(a));
        initial.dedup();
        Ok(initial)
    }

    fn monomorph_request_is_reachable_initial_seed(&self, request: &MonomorphRequest) -> bool {
        if !self.filter_initial_requests_to_reachable_call_sites {
            return true;
        }
        let site = (request.request_source_path.clone(), request.call_span);
        self.reachable_request_call_sites.contains(&site)
            || self
                .reachable_request_stmt_spans
                .iter()
                .any(|(source_path, stmt_span)| {
                    source_path == &request.request_source_path
                        && request.call_span.start >= stmt_span.start
                        && request.call_span.end <= stmt_span.end
                })
    }

    fn seed_request_root_direct_call_instances(&mut self) -> MaterializeResult<Vec<InstanceKey>> {
        if self.request_root_funs.is_empty() {
            return Ok(Vec::new());
        }
        let request_root_funs = self.request_root_funs.clone();
        let mut out = Vec::new();

        for request_root in request_root_funs {
            self.scan_reachable_non_generic_fun(&request_root, &mut out)?;
        }

        Ok(out)
    }

    fn scan_reachable_non_generic_fun(
        &mut self,
        reachable_fun: &ReachableMirFun,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        if self.generic_family_fqns.contains(&reachable_fun.fun.fqn) {
            return Ok(());
        }
        let scan_key = (reachable_fun.source_path.clone(), reachable_fun.fun.span);
        if !self.scanned_non_generic_funs.insert(scan_key.clone()) {
            return Ok(());
        }
        let Some(body) = &reachable_fun.fun.body else {
            return Ok(());
        };
        if let Some(hir_direct_instances) =
            self.hir_direct_instance_keys_by_fun.get(&scan_key).cloned()
        {
            out.extend(hir_direct_instances);
        }
        let substitution = InstanceSubstitution::default();
        let locals = &body.locals;
        for block_idx in reachable_body_block_indices(body) {
            let Some(block) = body.blocks.get(block_idx) else {
                continue;
            };
            for stmt in &block.stmts {
                self.reachable_request_stmt_spans
                    .push((reachable_fun.source_path.clone(), stmt.span));
                if let StatementKind::Assign { target, value } = &stmt.kind {
                    let result_ty = locals.get(target.as_u32() as usize).map(|local| local.ty);
                    self.collect_reachable_instances_from_rvalue(
                        value,
                        ReachableRvalueScanContext {
                            span: stmt.span,
                            result_ty,
                            template_source_path: &reachable_fun.source_path,
                            locals,
                            substitution: &substitution,
                        },
                        out,
                    )?;
                }
            }
        }

        let mut candidate_fun = reachable_fun.fun.clone();
        let candidate_root_fqn = candidate_fun.fqn.clone();
        if let Some(candidate_body) = candidate_fun.body.as_mut() {
            self.rewrite_reachable_body(
                candidate_body,
                &substitution,
                reachable_fun.source_path.as_path(),
                &candidate_root_fqn,
                &candidate_root_fqn,
            )?;
        }
        self.caller_side_pass_candidates.push(candidate_fun.clone());
        self.pass_published_ordinary_callables
            .push(PassPublishedOrdinaryCallable {
                source_path: reachable_fun.source_path.clone(),
                fun: candidate_fun,
            });
        Ok(())
    }

    fn collect_reachable_instances_from_rvalue(
        &mut self,
        value: &Rvalue,
        scan: ReachableRvalueScanContext<'_>,
        out: &mut Vec<InstanceKey>,
    ) -> MaterializeResult<()> {
        match value {
            Rvalue::Call {
                kind: CallKind::Direct { callee_fqn },
                args,
                ..
            } => {
                self.reachable_request_call_sites
                    .insert((scan.template_source_path.to_path_buf(), scan.span));
                if let Some(instance_key) =
                    self.infer_direct_call_instance(DirectCallInferenceInput {
                        template_source_path: scan.template_source_path,
                        call_span: scan.span,
                        callee_fqn,
                        args,
                        result_ty: scan.result_ty,
                        locals: scan.locals,
                        substitution: scan.substitution,
                    })
                {
                    out.push(instance_key);
                    return Ok(());
                }
                if let Some(reachable_callee) = self.resolve_non_generic_direct_callee(
                    scan.template_source_path,
                    scan.span,
                    callee_fqn,
                ) {
                    self.scan_reachable_non_generic_fun(&reachable_callee, out)?;
                }
            }
            Rvalue::MakeClosure { fn_ptr, .. } => {
                if let Some(reachable_closure) =
                    self.resolve_non_generic_fun_body_by_fqn(scan.template_source_path, fn_ptr)
                {
                    self.scan_reachable_non_generic_fun(&reachable_closure, out)?;
                }
            }
            Rvalue::TopLevelRef(TopLevelRef { fqn }) => {
                self.reachable_request_call_sites
                    .insert((scan.template_source_path.to_path_buf(), scan.span));
                if let Some(binding) =
                    self.site_instance_binding_for_callee(scan.template_source_path, scan.span, fqn)
                    && let Some(instance_key) =
                        self.instantiate_site_binding(&binding, scan.substitution)
                {
                    out.push(instance_key);
                    return Ok(());
                }
                if let Some(reachable_fun) =
                    self.resolve_non_generic_fun_body_by_fqn(scan.template_source_path, fqn)
                {
                    self.scan_reachable_non_generic_fun(&reachable_fun, out)?;
                }
                self.scan_reachable_top_level_immutable_value(fqn);
            }
            _ => {}
        }
        Ok(())
    }

    fn scan_reachable_top_level_immutable_value(&mut self, fqn: &str) {
        if !self
            .scanned_top_level_immutable_values
            .insert(fqn.to_string())
        {
            return;
        }
        let Some(value) = self.top_level_immutable_values.get(fqn).cloned() else {
            return;
        };
        let Some(init) = value.init else {
            return;
        };

        self.reachable_request_stmt_spans
            .push((value.source_path.clone(), init.span));

        let mut refs = Vec::new();
        collect_top_level_value_refs_in_expr(&init, &mut refs);
        for dep_fqn in refs {
            self.scan_reachable_top_level_immutable_value(&dep_fqn);
        }
    }

    fn resolve_non_generic_fun_body_by_fqn(
        &self,
        default_source_path: &Path,
        fqn: &str,
    ) -> Option<ReachableMirFun> {
        if let Some(candidates) = self.reachable_fun_bodies_by_fqn.get(fqn) {
            if candidates.len() != 1 {
                return None;
            }
            let candidate = candidates[0].clone();
            return (!self.generic_family_fqns.contains(&candidate.fun.fqn)).then_some(candidate);
        }
        let candidates = self.all_fun_bodies_by_fqn.get(fqn)?;
        if candidates.len() != 1 {
            return None;
        }
        let fun = candidates[0].clone();
        (!self.generic_family_fqns.contains(&fun.fqn)).then_some(ReachableMirFun {
            source_path: default_source_path.to_path_buf(),
            fun,
        })
    }

    fn resolve_non_generic_direct_callee(
        &self,
        template_source_path: &Path,
        call_span: Span,
        callee_fqn: &str,
    ) -> Option<ReachableMirFun> {
        let call_site = (template_source_path.to_path_buf(), call_span);
        if let Some(binding) = self.direct_call_bindings.get(&call_site) {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                if let Some(fun) = self
                    .reachable_fun_bodies_by_request
                    .get(&(
                        binding.fqn.clone(),
                        binding.decl_file.clone(),
                        binding.decl_span,
                    ))
                    .cloned()
                {
                    return (!self.generic_family_fqns.contains(&fun.fun.fqn)).then_some(fun);
                }
            } else {
                return None;
            }
        }

        self.resolve_non_generic_fun_body_by_fqn(template_source_path, callee_fqn)
    }

    fn run(mut self, initial_requests: Vec<InstanceKey>) -> MaterializeResult<MaterializedMir> {
        for request in initial_requests {
            self.enqueue(request);
        }

        while let Some(instance) = self.queue.pop_front() {
            self.queued.remove(&instance);
            if self.materialized.contains_key(&instance) {
                continue;
            }
            let family = self.materialize_instance(&instance)?;
            self.materialized.insert(instance, family);
        }

        let mut materialized_instance_keys = self.materialized.keys().cloned().collect::<Vec<_>>();
        materialized_instance_keys.sort_by_key(|a| self.instance_fqn(a));
        let materialized_instance_set = materialized_instance_keys
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let decl_only_instances = self
            .declaration_only_instances
            .iter()
            .filter(|instance| !materialized_instance_set.contains(*instance))
            .cloned()
            .collect::<Vec<_>>();

        let mut instance_keys = materialized_instance_keys.clone();
        instance_keys.extend(decl_only_instances.iter().cloned());
        instance_keys.sort_by_key(|a| self.instance_fqn(a));
        instance_keys.dedup();

        let mut pass_visible_non_generic_roots = self
            .pass_published_ordinary_callables
            .iter()
            .map(|published| {
                (
                    self.non_generic_pass_view_instance_key(
                        published.source_path.as_path(),
                        &published.fun,
                    ),
                    published.fun.clone(),
                )
            })
            .collect::<Vec<_>>();
        pass_visible_non_generic_roots.sort_by_key(|(instance, _)| self.instance_fqn(instance));
        pass_visible_non_generic_roots.dedup_by(|(left, _), (right, _)| left == right);

        let mut pass_instance_keys = instance_keys.clone();
        pass_instance_keys.extend(
            pass_visible_non_generic_roots
                .iter()
                .map(|(instance, _)| instance.clone()),
        );
        pass_instance_keys.sort_by_key(|a| self.instance_fqn(a));
        pass_instance_keys.dedup();

        let root_instances = materialized_instance_keys
            .iter()
            .cloned()
            .map(|instance| InstanceRootSummaryInput {
                root_fqn: self.instance_fqn(&instance),
                instance,
            })
            .collect::<Vec<_>>();
        let mut pass_root_instances = root_instances.clone();
        pass_root_instances.extend(
            pass_visible_non_generic_roots
                .iter()
                .map(|(instance, fun)| InstanceRootSummaryInput {
                    instance: instance.clone(),
                    root_fqn: fun.fqn.clone(),
                }),
        );
        let decl_only_inputs = decl_only_instances
            .iter()
            .filter_map(|instance| {
                let signature = self.template_signatures.get(&instance.template)?;
                let substitution =
                    self.build_instance_substitution_for_signature(signature, instance);
                Some(DeclOnlySummaryInput {
                    instance: instance.clone(),
                    root_fqn: self.instance_fqn(instance),
                    declared_fun_ty: substitute_type_and_effect_params(
                        &mut self.types,
                        signature.fun_ty,
                        &substitution,
                    ),
                    declared_return_ty: substitute_type_and_effect_params(
                        &mut self.types,
                        signature.return_ty,
                        &substitution,
                    ),
                    param_count: signature.params.len(),
                })
            })
            .collect::<Vec<_>>();
        let mut callable_family_inputs = materialized_instance_keys
            .iter()
            .cloned()
            .map(|instance| {
                let root_fqn = self.instance_fqn(&instance);
                let mut callable_fqns = self
                    .materialized
                    .get(&instance)
                    .cloned()
                    .expect("materialized instance should exist")
                    .into_iter()
                    .filter(|fun| fun.body.is_some())
                    .map(|fun| fun.fqn)
                    .collect::<Vec<_>>();
                callable_fqns.sort_by(|a, b| {
                    let a_root = a == &root_fqn;
                    let b_root = b == &root_fqn;
                    (!a_root).cmp(&!b_root).then_with(|| a.cmp(b))
                });
                callable_fqns.dedup();
                MaterializedCallableFamilyInput {
                    instance,
                    root_fqn,
                    callable_fqns,
                }
            })
            .collect::<Vec<_>>();
        let mut pass_callable_family_inputs = callable_family_inputs.clone();
        pass_callable_family_inputs.extend(pass_visible_non_generic_roots.iter().map(
            |(instance, fun)| MaterializedCallableFamilyInput {
                instance: instance.clone(),
                root_fqn: fun.fqn.clone(),
                callable_fqns: vec![fun.fqn.clone()],
            },
        ));
        let decl_only_callable_family_inputs = decl_only_instances
            .iter()
            .cloned()
            .map(|instance| MaterializedCallableFamilyInput {
                root_fqn: self.instance_fqn(&instance),
                instance,
                callable_fqns: Vec::new(),
            })
            .collect::<Vec<_>>();
        callable_family_inputs.extend(decl_only_callable_family_inputs.clone());
        pass_callable_family_inputs.extend(decl_only_callable_family_inputs);
        let callable_families = MaterializedCallableFamilies::from_inputs(callable_family_inputs);
        let pass_callable_families =
            MaterializedCallableFamilies::from_inputs(pass_callable_family_inputs);

        let mut items = Vec::new();
        for key in &materialized_instance_keys {
            let mut family = self
                .materialized
                .get(key)
                .cloned()
                .expect("materialized instance should exist");
            family.sort_by(|a, b| {
                let a_root = a.fqn == self.instance_fqn(key);
                let b_root = b.fqn == self.instance_fqn(key);
                (!a_root).cmp(&!b_root).then_with(|| a.fqn.cmp(&b.fqn))
            });
            items.extend(family.into_iter().map(Item::Fun));
        }
        let mut pass_items = items.clone();
        pass_items.extend(
            pass_visible_non_generic_roots
                .into_iter()
                .map(|(_, fun)| Item::Fun(fun)),
        );
        let file = File { items };
        let pass_file = File { items: pass_items };
        let summaries = build_materialized_summary_table(
            &file,
            &self.types,
            &root_instances,
            &decl_only_inputs,
        );
        let pass_summaries = build_materialized_summary_table(
            &pass_file,
            &self.types,
            &pass_root_instances,
            &decl_only_inputs,
        );
        let pass_artifacts = MaterializedMirPassArtifacts::from_initial_publication(
            &pass_file,
            &pass_summaries,
            &pass_callable_families,
            &pass_instance_keys,
        );

        let mut materialized = MaterializedMir {
            file,
            types: self.types,
            instance_keys,
            summaries,
            opt_level: self.opt_level,
            callable_families,
            pass_artifacts,
            caller_side_pass_candidates: self.caller_side_pass_candidates,
        };
        if self.enable_summary_driven_inlining {
            super::inline::run_summary_driven_inlining(&mut materialized);
        }
        if self.enable_mir_escape_analysis {
            super::escape::run_escape_analysis(&mut materialized);
            if super::closure_simplify::run_non_escaping_closure_simplification(&mut materialized) {
                super::escape::run_escape_analysis(&mut materialized);
            }
        }
        Ok(materialized)
    }

    fn enqueue(&mut self, key: InstanceKey) {
        if self.materialized.contains_key(&key)
            || self.declaration_only_instances.contains(&key)
            || self.queued.contains(&key)
        {
            return;
        }

        if self.roots.contains_key(&key.template) {
            self.queued.insert(key.clone());
            self.queue.push_back(key);
        } else if self.template_signatures.contains_key(&key.template) {
            self.declaration_only_instances.insert(key);
        }
    }

    fn materialize_instance(&mut self, instance: &InstanceKey) -> MaterializeResult<Vec<FunDecl>> {
        let Some(root) = self.roots.get(&instance.template).cloned() else {
            return Err(materialize_err(
                MirMaterializeError::MissingGenericTemplate {
                    fqn: instance.template.fqn.clone(),
                    file: instance.template.source_path.display().to_string(),
                    span: instance.template.decl_span,
                },
            ));
        };

        if root.type_param_names.len() != instance.type_args.len() {
            return Err(materialize_err(MirMaterializeError::TypeArgArityMismatch {
                fqn: root.template.fqn.clone(),
                expected: root.type_param_names.len(),
                found: instance.type_args.len(),
                decl_span: root.template.decl_span.into(),
            }));
        }

        let substitution = self.build_instance_substitution(&root, instance)?;
        let instance_root_fqn = self.instance_fqn(instance);

        let mut out = Vec::with_capacity(root.family.len());
        for template_fun in &root.family {
            let mut fun = template_fun.clone();
            fun.fqn = rewrite_family_symbol_name(&fun.fqn, &root.template.fqn, &instance_root_fqn)
                .unwrap_or_else(|| fun.fqn.clone());
            fun.ty = substitute_type_and_effect_params(&mut self.types, fun.ty, &substitution);
            for param in &mut fun.params {
                param.ty =
                    substitute_type_and_effect_params(&mut self.types, param.ty, &substitution);
            }
            fun.return_ty =
                substitute_type_and_effect_params(&mut self.types, fun.return_ty, &substitution);
            if let Some(body) = &mut fun.body {
                self.rewrite_body(
                    body,
                    &substitution,
                    &root.template.source_path,
                    &root.template.fqn,
                    &instance_root_fqn,
                )?;
            }
            out.push(fun);
        }

        Ok(out)
    }

    fn build_instance_substitution(
        &self,
        root: &TemplateRootInfo,
        instance: &InstanceKey,
    ) -> MaterializeResult<InstanceSubstitution> {
        let mut substitution = InstanceSubstitution {
            type_params: root
                .type_param_names
                .iter()
                .cloned()
                .zip(instance.type_args.iter().copied())
                .collect(),
            effect_params: HashMap::new(),
        };

        match (&root.eff_param_name, instance.eff_args.as_slice()) {
            (None, []) => {}
            (None, eff_args) => {
                return Err(materialize_err(
                    MirMaterializeError::EffectArgArityMismatch {
                        fqn: root.template.fqn.clone(),
                        expected: 0,
                        found: eff_args.len(),
                        decl_span: root.template.decl_span.into(),
                    },
                ));
            }
            (Some(name), [row]) => {
                substitution.effect_params.insert(name.clone(), row.clone());
            }
            (Some(_), eff_args) => {
                return Err(materialize_err(
                    MirMaterializeError::EffectArgArityMismatch {
                        fqn: root.template.fqn.clone(),
                        expected: 1,
                        found: eff_args.len(),
                        decl_span: root.template.decl_span.into(),
                    },
                ));
            }
        }

        Ok(substitution)
    }

    fn build_instance_substitution_for_signature(
        &self,
        signature: &TemplateSignatureInfo,
        instance: &InstanceKey,
    ) -> InstanceSubstitution {
        let mut substitution = InstanceSubstitution {
            type_params: signature
                .type_param_names
                .iter()
                .cloned()
                .zip(instance.type_args.iter().copied())
                .collect(),
            effect_params: HashMap::new(),
        };
        if let (Some(name), [row]) = (&signature.eff_param_name, instance.eff_args.as_slice()) {
            substitution.effect_params.insert(name.clone(), row.clone());
        }
        substitution
    }

    fn rewrite_body(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        self.rewrite_body_blocks(
            body,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
            None,
        )
    }

    fn rewrite_reachable_body(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        self.rewrite_body_blocks(
            body,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
            Some(reachable_body_block_indices(body)),
        )
    }

    fn rewrite_body_blocks(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
        block_indices: Option<Vec<usize>>,
    ) -> MaterializeResult<()> {
        for local in &mut body.locals {
            local.ty = substitute_type_and_effect_params(&mut self.types, local.ty, substitution);
        }
        let locals = body.locals.clone();
        let ctx = RewriteContext {
            locals: &locals,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
        };
        if let Some(block_indices) = block_indices {
            for block_idx in block_indices {
                let Some(block) = body.blocks.get_mut(block_idx) else {
                    continue;
                };
                self.rewrite_block(block, &ctx)?;
            }
        } else {
            for block in &mut body.blocks {
                self.rewrite_block(block, &ctx)?;
            }
        }
        Ok(())
    }

    fn rewrite_block(
        &mut self,
        block: &mut super::BasicBlock,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        for stmt in &mut block.stmts {
            self.rewrite_statement(stmt, ctx)?;
        }
        self.rewrite_terminator(&mut block.terminator, ctx)
    }

    fn rewrite_statement(
        &mut self,
        stmt: &mut Statement,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match &mut stmt.kind {
            StatementKind::Assign { value, .. } => self.rewrite_rvalue(stmt.span, value, ctx)?,
            StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => {
                *receiver = self.rewrite_operand(receiver.clone());
                self.rewrite_member_access_metadata(member, ctx);
                *value = self.rewrite_operand(value.clone());
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
                if let crate::mir::StoredContinuationRoutePublication::Unique(route) =
                    continuation_route
                {
                    route.source_ty = substitute_type_and_effect_params(
                        &mut self.types,
                        route.source_ty,
                        ctx.substitution,
                    );
                }
            }
            StatementKind::StoreTopLevelVar {
                value, value_ty, ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            StatementKind::Nop | StatementKind::Todo(_) => {}
        }
        Ok(())
    }

    fn rewrite_terminator(
        &mut self,
        terminator: &mut Terminator,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match &mut terminator.kind {
            TerminatorKind::Perform { metadata, args, .. } => {
                self.rewrite_perform_metadata(metadata, ctx.substitution);
                for arg in args {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
            }
            TerminatorKind::Handle { metadata, arms, .. } => {
                self.rewrite_handle_metadata(metadata, ctx.substitution);
                for arm in arms {
                    self.rewrite_handler_arm(arm, ctx.substitution);
                }
            }
            TerminatorKind::CondBr { cond, .. } => {
                *cond = self.rewrite_operand(cond.clone());
            }
            TerminatorKind::Return { value } => {
                *value = value.take().map(|operand| self.rewrite_operand(operand));
            }
            TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
        Ok(())
    }

    fn rewrite_handle_metadata(
        &mut self,
        metadata: &mut HandleMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.result_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.result_ty, substitution);
        metadata.body_result_ty = substitute_type_and_effect_params(
            &mut self.types,
            metadata.body_result_ty,
            substitution,
        );
        metadata.finally_result_ty = metadata
            .finally_result_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
    }

    fn rewrite_handler_arm(&mut self, arm: &mut HandlerArm, substitution: &InstanceSubstitution) {
        arm.handled_effect_ty =
            substitute_type_and_effect_params(&mut self.types, arm.handled_effect_ty, substitution);
        arm.payload_tuple_ty = arm
            .payload_tuple_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
        for ty in &mut arm.payload_component_tys {
            *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
        }
        arm.body_ty = substitute_type_and_effect_params(&mut self.types, arm.body_ty, substitution);
    }

    fn rewrite_rvalue(
        &mut self,
        stmt_span: Span,
        value: &mut Rvalue,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match value {
            Rvalue::Use(operand) => *operand = self.rewrite_operand(operand.clone()),
            Rvalue::TopLevelRef(top) => {
                if let Some(rewritten) = rewrite_family_symbol_name(
                    &top.fqn,
                    ctx.template_root_fqn,
                    ctx.instance_root_fqn,
                ) {
                    top.fqn = rewritten;
                }
            }
            Rvalue::UnresolvedName { .. } => {}
            Rvalue::Unary { operand, .. } => *operand = self.rewrite_operand(operand.clone()),
            Rvalue::Binary { lhs, rhs, .. } => {
                *lhs = self.rewrite_operand(lhs.clone());
                *rhs = self.rewrite_operand(rhs.clone());
            }
            Rvalue::TypeCheck { value, test_ty, .. } => {
                *value = self.rewrite_operand(value.clone());
                *test_ty =
                    substitute_type_and_effect_params(&mut self.types, *test_ty, ctx.substitution);
            }
            Rvalue::Cast {
                value, target_ty, ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *target_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    *target_ty,
                    ctx.substitution,
                );
            }
            Rvalue::SizeOf { value_ty } => {
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            Rvalue::MemberAccess {
                receiver, member, ..
            } => {
                *receiver = self.rewrite_operand(receiver.clone());
                self.rewrite_member_access_metadata(member, ctx);
            }
            Rvalue::Call { kind, args, .. } => {
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                self.rewrite_call_kind(stmt_span, kind, args, ctx)?;
            }
            Rvalue::EnumVariant { enum_ty, args, .. } => {
                *enum_ty =
                    substitute_type_and_effect_params(&mut self.types, *enum_ty, ctx.substitution);
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
            }
            Rvalue::ClassCtor {
                args,
                hidden_effects,
                ..
            } => {
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                hidden_effects.terms = hidden_effects
                    .terms
                    .iter()
                    .map(|ty| {
                        substitute_type_and_effect_params(&mut self.types, *ty, ctx.substitution)
                    })
                    .collect();
            }
            Rvalue::MakeTuple { elements } => {
                for element in elements.iter_mut() {
                    *element = self.rewrite_operand(element.clone());
                }
            }
            Rvalue::StructLit { fields } => {
                for field in fields.iter_mut() {
                    field.value = self.rewrite_operand(field.value.clone());
                }
            }
            Rvalue::InterpolatedString { parts, .. } => {
                for part in parts.iter_mut() {
                    if let super::InterpolatedStringPart::Expr { value, ty, .. } = part {
                        *value = self.rewrite_operand(value.clone());
                        *ty = substitute_type_and_effect_params(
                            &mut self.types,
                            *ty,
                            ctx.substitution,
                        );
                    }
                }
            }
            Rvalue::TupleGet { tuple, .. } => *tuple = self.rewrite_operand(tuple.clone()),
            Rvalue::CaptureBoxNew { value } => *value = self.rewrite_operand(value.clone()),
            Rvalue::CaptureBoxGet { box_operand } => {
                *box_operand = self.rewrite_operand(box_operand.clone());
            }
            Rvalue::CaptureBoxSet { box_operand, value } => {
                *box_operand = self.rewrite_operand(box_operand.clone());
                *value = self.rewrite_operand(value.clone());
            }
            Rvalue::PatternMatch { subject, pattern } => {
                *subject = self.rewrite_operand(subject.clone());
                self.rewrite_pattern(pattern, ctx.substitution);
            }
            Rvalue::PatternExtract { subject, path } => {
                *subject = self.rewrite_operand(subject.clone());
                let _ = path;
            }
            Rvalue::MakeClosure { env, fn_ptr } => {
                *env = self.rewrite_operand(env.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, ctx.template_root_fqn, ctx.instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            Rvalue::PerformResult { effect_ty, .. } => {
                *effect_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    *effect_ty,
                    ctx.substitution,
                );
            }
            Rvalue::Todo(_) => {}
        }
        Ok(())
    }

    fn rewrite_call_kind(
        &mut self,
        call_span: Span,
        kind: &mut CallKind,
        args: &mut Vec<CallArg>,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match kind {
            CallKind::Direct { callee_fqn } => {
                if let Some(rewritten) = rewrite_family_symbol_name(
                    callee_fqn,
                    ctx.template_root_fqn,
                    ctx.instance_root_fqn,
                ) {
                    *callee_fqn = rewritten;
                    return Ok(());
                }
                self.materialize_direct_call_target(
                    ctx.template_source_path,
                    call_span,
                    callee_fqn,
                    args,
                    ctx.locals,
                    ctx.substitution,
                )?;
            }
            CallKind::Closure { callee, fn_ptr } => {
                *callee = self.rewrite_operand(callee.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, ctx.template_root_fqn, ctx.instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            CallKind::FunValue { callee } => *callee = self.rewrite_operand(callee.clone()),
            CallKind::Virtual { receiver, dispatch } => {
                *receiver = self.rewrite_operand(receiver.clone());
                dispatch.receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    ctx.substitution,
                );
                if let Some(target_fqn) = crate::devirtualize::try_devirtualize_dispatch_target(
                    crate::hir::DispatchCallKind::Virtual,
                    &dispatch.owner_fqn,
                    &dispatch.member_name,
                    args.len(),
                    dispatch.receiver_ty,
                    &self.types,
                    crate::devirtualize::DispatchTargetFacts {
                        known_receiver_subclasses: &self.known_receiver_subclasses,
                        class_vtables: &self.class_vtables,
                        interfaces: &self.interfaces,
                        class_itables: &self.class_itables,
                    },
                ) {
                    let direct_args = dispatch_direct_call_args(call_span, receiver, args);
                    let mut direct_fqn = target_fqn;
                    self.materialize_direct_call_target(
                        ctx.template_source_path,
                        call_span,
                        &mut direct_fqn,
                        &direct_args,
                        ctx.locals,
                        ctx.substitution,
                    )?;
                    *args = direct_args;
                    *kind = CallKind::Direct {
                        callee_fqn: direct_fqn,
                    };
                }
            }
            CallKind::Interface { receiver, dispatch } => {
                *receiver = self.rewrite_operand(receiver.clone());
                dispatch.receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    ctx.substitution,
                );
                if let Some(target_fqn) = crate::devirtualize::try_devirtualize_dispatch_target(
                    crate::hir::DispatchCallKind::Interface,
                    &dispatch.owner_fqn,
                    &dispatch.member_name,
                    args.len(),
                    dispatch.receiver_ty,
                    &self.types,
                    crate::devirtualize::DispatchTargetFacts {
                        known_receiver_subclasses: &self.known_receiver_subclasses,
                        class_vtables: &self.class_vtables,
                        interfaces: &self.interfaces,
                        class_itables: &self.class_itables,
                    },
                ) {
                    let direct_args = dispatch_direct_call_args(call_span, receiver, args);
                    let mut direct_fqn = target_fqn;
                    self.materialize_direct_call_target(
                        ctx.template_source_path,
                        call_span,
                        &mut direct_fqn,
                        &direct_args,
                        ctx.locals,
                        ctx.substitution,
                    )?;
                    *args = direct_args;
                    *kind = CallKind::Direct {
                        callee_fqn: direct_fqn,
                    };
                }
            }
            CallKind::Resume {
                continuation,
                resume,
            } => {
                *continuation = self.rewrite_operand(continuation.clone());
                resume.continuation_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.continuation_ty,
                    ctx.substitution,
                );
                resume.resume_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.resume_ty,
                    ctx.substitution,
                );
                resume.answer_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.answer_ty,
                    ctx.substitution,
                );
                resume.return_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.return_ty,
                    ctx.substitution,
                );
                resume.out_effects = substitute_type_and_effect_params_in_effect_row(
                    &mut self.types,
                    &resume.out_effects,
                    ctx.substitution,
                );
                resume.runtime_error_effect_ty = resume.runtime_error_effect_ty.map(|ty| {
                    substitute_type_and_effect_params(&mut self.types, ty, ctx.substitution)
                });
            }
        }
        Ok(())
    }

    fn materialize_direct_call_target(
        &mut self,
        template_source_path: &Path,
        call_span: Span,
        callee_fqn: &mut String,
        args: &[CallArg],
        locals: &[LocalDecl],
        substitution: &InstanceSubstitution,
    ) -> MaterializeResult<()> {
        if let Some(instance_key) = self.infer_direct_call_instance(DirectCallInferenceInput {
            template_source_path,
            call_span,
            callee_fqn,
            args,
            result_ty: None,
            locals,
            substitution,
        }) {
            *callee_fqn = self.instance_fqn(&instance_key);
            self.enqueue(instance_key);
            return Ok(());
        }
        if let Some(reachable_callee) =
            self.resolve_non_generic_direct_callee(template_source_path, call_span, callee_fqn)
        {
            let mut discovered = Vec::new();
            self.scan_reachable_non_generic_fun(&reachable_callee, &mut discovered)?;
            for instance in discovered {
                self.enqueue(instance);
            }
        }
        Ok(())
    }

    fn infer_direct_call_instance(
        &mut self,
        input: DirectCallInferenceInput<'_>,
    ) -> Option<InstanceKey> {
        if let Some(binding) = self.site_instance_binding_for_callee(
            input.template_source_path,
            input.call_span,
            input.callee_fqn,
        ) {
            return self.instantiate_site_binding(&binding, input.substitution);
        }

        let candidates = self.roots_by_fqn.get(input.callee_fqn)?;
        if candidates.len() != 1 {
            return None;
        }
        let signature = self.template_signatures.get(&candidates[0])?;
        if signature.type_param_names.is_empty() || signature.eff_param_name.is_some() {
            return None;
        }

        let arg_to_param = map_call_args_to_signature_params(&signature.params, input.args)?;
        let mut bindings = HashMap::new();
        for (arg_idx, param_idx) in arg_to_param.into_iter().enumerate() {
            let param = signature.params.get(param_idx)?;
            if !type_contains_param(&self.types, param.ty) {
                continue;
            }
            let arg = input.args.get(arg_idx)?;
            let concrete_ty = operand_type(&self.types, self.builtins, input.locals, &arg.value)?;
            collect_type_param_bindings(&self.types, param.ty, concrete_ty, &mut bindings);
        }
        if let Some(result_ty) = input.result_ty
            && type_contains_param(&self.types, signature.return_ty)
            && !type_contains_param(&self.types, result_ty)
        {
            collect_type_param_bindings(&self.types, signature.return_ty, result_ty, &mut bindings);
        }

        let mut ordered = Vec::with_capacity(signature.type_param_names.len());
        for name in &signature.type_param_names {
            let ty = bindings.get(name).copied()?;
            if type_contains_param(&self.types, ty) {
                return None;
            }
            ordered.push(ty);
        }
        if ordered.is_empty() {
            return None;
        }

        Some(InstanceKey {
            template: signature.template.clone(),
            type_args: ordered,
            eff_args: Vec::new(),
        })
    }

    fn lookup_site_instance_binding(
        &self,
        template_source_path: &Path,
        call_span: Span,
    ) -> Option<&SiteInstanceBinding> {
        let key = (template_source_path.to_path_buf(), call_span);
        self.call_bindings
            .get(&key)
            .or_else(|| self.value_ref_bindings.get(&key))
    }

    fn site_instance_binding_for_callee(
        &self,
        template_source_path: &Path,
        call_span: Span,
        callee_fqn: &str,
    ) -> Option<SiteInstanceBinding> {
        let binding = self
            .lookup_site_instance_binding(template_source_path, call_span)?
            .clone();
        if binding.template.fqn == callee_fqn
            || callee_fqn
                .strip_prefix(binding.template.fqn.as_str())
                .is_some_and(|suffix| suffix.starts_with("::<"))
        {
            return Some(binding);
        }
        let template = self.remap_site_binding_template(&binding.template, callee_fqn)?;
        Some(SiteInstanceBinding {
            template,
            type_args: binding.type_args,
            eff_args: binding.eff_args,
        })
    }

    fn remap_site_binding_template(
        &self,
        source_template: &TemplateKey,
        target_fqn: &str,
    ) -> Option<TemplateKey> {
        let source_signature = self.template_signatures.get(source_template)?;
        let candidates = self.roots_by_fqn.get(target_fqn)?;
        let compatible = candidates
            .iter()
            .filter_map(|candidate| {
                let signature = self.template_signatures.get(candidate)?;
                (signature.params.len() == source_signature.params.len()
                    && signature.type_param_names.len() == source_signature.type_param_names.len()
                    && signature.eff_param_name.is_some()
                        == source_signature.eff_param_name.is_some())
                .then_some(candidate.clone())
            })
            .collect::<Vec<_>>();
        match compatible.as_slice() {
            [template] => Some(template.clone()),
            _ => None,
        }
    }

    fn instantiate_site_binding(
        &mut self,
        binding: &SiteInstanceBinding,
        substitution: &InstanceSubstitution,
    ) -> Option<InstanceKey> {
        let type_args = binding
            .type_args
            .iter()
            .copied()
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution))
            .collect::<Vec<_>>();
        let eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                substitute_type_and_effect_params_in_effect_row(&mut self.types, row, substitution)
            })
            .collect::<Vec<_>>();
        if (type_args.is_empty() && eff_args.is_empty())
            || !instance_request_is_concrete(&self.types, &type_args, &eff_args)
        {
            return None;
        }
        Some(InstanceKey {
            template: binding.template.clone(),
            type_args,
            eff_args,
        })
    }

    fn rewrite_member_access_metadata(
        &mut self,
        member: &mut MemberAccessMetadata,
        ctx: &RewriteContext<'_>,
    ) {
        member.receiver_ty = substitute_type_and_effect_params(
            &mut self.types,
            member.receiver_ty,
            ctx.substitution,
        );
        member.hidden_effects.terms = member
            .hidden_effects
            .terms
            .iter()
            .map(|ty| substitute_type_and_effect_params(&mut self.types, *ty, ctx.substitution))
            .collect();
        if let Some(target) = &mut member.resolved {
            match target {
                MemberTarget::Fun { fqn } | MemberTarget::ExtensionFun { fqn } => {
                    if let Some(rewritten) = rewrite_family_symbol_name(
                        fqn,
                        ctx.template_root_fqn,
                        ctx.instance_root_fqn,
                    ) {
                        *fqn = rewritten;
                    }
                }
                MemberTarget::Value { .. } | MemberTarget::ExtensionValue { .. } => {}
            }
        }
    }

    fn rewrite_pattern(&mut self, pattern: &mut Pattern, substitution: &InstanceSubstitution) {
        match pattern {
            Pattern::Is { ty } | Pattern::Bind { ty, .. } => {
                *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
            }
            Pattern::Or { pats } => {
                for pat in pats {
                    self.rewrite_pattern(pat, substitution);
                }
            }
            Pattern::Tuple { elements } | Pattern::Variant { args: elements, .. } => {
                for pat in elements {
                    self.rewrite_pattern(pat, substitution);
                }
            }
            Pattern::Else
            | Pattern::Wildcard
            | Pattern::Rest
            | Pattern::IntLit { .. }
            | Pattern::CharLit { .. }
            | Pattern::StringLit { .. }
            | Pattern::BoolLit { .. } => {}
        }
    }

    fn rewrite_perform_metadata(
        &mut self,
        metadata: &mut PerformMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.effect_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.effect_ty, substitution);
        metadata.payload_tuple_ty = metadata
            .payload_tuple_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
        for ty in &mut metadata.payload_component_tys {
            *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
        }
    }

    fn rewrite_operand(&mut self, operand: Operand) -> Operand {
        operand
    }

    fn template_symbol_suffix(&self, template: &TemplateKey) -> &str {
        self.template_symbol_suffixes
            .get(template)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn instance_fqn(&self, instance: &InstanceKey) -> String {
        let symbol_suffix = self.template_symbol_suffix(&instance.template);
        if instance.type_args.is_empty() && instance.eff_args.is_empty() {
            return format!("{}{symbol_suffix}", instance.template.fqn);
        }
        let mut args = instance
            .type_args
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>();
        args.extend(
            instance
                .eff_args
                .iter()
                .map(|row| format!("eff {}", self.format_effect_row_stable(row))),
        );
        format!(
            "{}::<{}>{symbol_suffix}",
            instance.template.fqn,
            args.join(", ")
        )
    }

    fn format_effect_row_stable(&self, row: &EffectRow) -> String {
        if row.terms.is_empty() {
            return "Pure".to_string();
        }
        row.terms
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>()
            .join(" + ")
    }

    fn non_generic_pass_view_instance_key(&self, source_path: &Path, fun: &FunDecl) -> InstanceKey {
        InstanceKey {
            template: TemplateKey {
                fqn: fun.fqn.clone(),
                source_path: source_path.to_path_buf(),
                decl_span: fun.span,
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        }
    }
}

fn canonical_template_map(
    candidates: &[TemplateCatalogCandidate],
) -> HashMap<(String, String), TemplateKey> {
    let mut grouped: HashMap<(String, String), Vec<&TemplateCatalogCandidate>> = HashMap::new();
    for candidate in candidates {
        grouped
            .entry((
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            ))
            .or_default()
            .push(candidate);
    }

    let mut out = HashMap::new();
    for (key, group) in grouped {
        let chosen = choose_canonical_template(&group);
        out.insert(key, chosen.template.clone());
    }
    out
}

fn choose_canonical_template<'a>(
    candidates: &[&'a TemplateCatalogCandidate],
) -> &'a TemplateCatalogCandidate {
    let mut preferred = candidates
        .iter()
        .copied()
        .filter(|candidate| candidate.prefers_materialized_body)
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter().copied());
    }
    preferred.sort_by(|a, b| {
        a.template
            .source_path
            .cmp(&b.template.source_path)
            .then_with(|| a.template.decl_span.start.cmp(&b.template.decl_span.start))
            .then_with(|| a.template.decl_span.end.cmp(&b.template.decl_span.end))
    });
    preferred
        .into_iter()
        .next()
        .expect("template candidate group must not be empty")
}

fn build_template_symbol_suffixes(
    signature_keys: &HashMap<TemplateKey, String>,
) -> HashMap<TemplateKey, String> {
    let mut templates_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
    for template in signature_keys.keys() {
        templates_by_fqn
            .entry(template.fqn.clone())
            .or_default()
            .push(template.clone());
    }

    let mut out = HashMap::new();
    for (_, mut templates) in templates_by_fqn {
        templates.sort_by(template_key_sort);
        let overloaded = templates.len() > 1;
        for template in templates {
            let symbol_suffix = if overloaded {
                let signature_key = signature_keys
                    .get(&template)
                    .expect("every template symbol suffix should have a signature key");
                format!(
                    "$overload${}",
                    stable_template_symbol_suffix(&template, signature_key)
                )
            } else {
                String::new()
            };
            out.insert(template, symbol_suffix);
        }
    }
    out
}

fn template_key_sort(lhs: &TemplateKey, rhs: &TemplateKey) -> std::cmp::Ordering {
    lhs.source_path
        .cmp(&rhs.source_path)
        .then_with(|| lhs.decl_span.start.cmp(&rhs.decl_span.start))
        .then_with(|| lhs.decl_span.end.cmp(&rhs.decl_span.end))
}

fn stable_template_symbol_suffix(template: &TemplateKey, signature_key: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    stable_hash_bytes(&mut hash, template.source_path.to_string_lossy().as_bytes());
    stable_hash_bytes(&mut hash, &[0xff]);
    stable_hash_bytes(&mut hash, &template.decl_span.start.to_le_bytes());
    stable_hash_bytes(&mut hash, &template.decl_span.end.to_le_bytes());
    stable_hash_bytes(&mut hash, &[0xfe]);
    stable_hash_bytes(&mut hash, signature_key.as_bytes());
    format!("{hash:016x}")
}

fn stable_hash_bytes(hash: &mut u64, bytes: &[u8]) {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn belongs_to_template_family(fun: &FunDecl, root_fun: &FunDecl) -> bool {
    if fun.fqn == root_fun.fqn {
        return fun.span == root_fun.span;
    }
    fun.fqn.strip_prefix(&root_fun.fqn).is_some_and(|suffix| {
        suffix.starts_with(".$lambda") && span_contains(root_fun.span, fun.span)
    })
}

fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn rewrite_family_symbol_name(
    symbol: &str,
    root_fqn: &str,
    instance_root_fqn: &str,
) -> Option<String> {
    if symbol == root_fqn {
        return Some(instance_root_fqn.to_string());
    }
    let suffix = symbol.strip_prefix(root_fqn)?;
    suffix
        .starts_with(".$lambda")
        .then(|| format!("{instance_root_fqn}{suffix}"))
}

fn re_intern_effect_row_from(
    types: &mut TypeStore,
    other: &TypeStore,
    row: &EffectRow,
) -> EffectRow {
    EffectRow::new(
        row.terms
            .iter()
            .map(|&term| types.re_intern_from(other, term))
            .collect(),
    )
}

fn substitute_type_and_effect_params(
    types: &mut TypeStore,
    ty: TypeId,
    substitution: &InstanceSubstitution,
) -> TypeId {
    match types.kind(ty).clone() {
        TypeKind::Param(param) => {
            if param.decl_file.as_os_str() == crate::hir::EFFECT_ROW_PARAM_DECL_FILE {
                ty
            } else {
                substitution
                    .type_params
                    .get(&param.name)
                    .copied()
                    .unwrap_or(ty)
            }
        }
        TypeKind::StarProjection(star) => {
            let read_ty = substitute_type_and_effect_params(types, star.read_ty, substitution);
            types.ty_star_projection(read_ty)
        }
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Ref(RefTypeKind::String) => ty,
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_and_effect_params(types, arg, substitution))
                .collect();
            let eff = nominal.eff.as_ref().map(|row| {
                substitute_type_and_effect_params_in_effect_row(types, row, substitution)
            });
            types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let receiver = fun
                .receiver
                .map(|receiver| substitute_type_and_effect_params(types, receiver, substitution));
            let params = fun
                .params
                .iter()
                .map(|&param| substitute_type_and_effect_params(types, param, substitution))
                .collect();
            let return_ty = substitute_type_and_effect_params(types, fun.return_ty, substitution);
            let effects =
                substitute_type_and_effect_params_in_effect_row(types, &fun.effects, substitution);
            types.ty_function(receiver, params, return_ty, effects, fun.effects_closed)
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let variants = union
                .variants
                .iter()
                .map(|&variant| substitute_type_and_effect_params(types, variant, substitution))
                .collect();
            types.ty_union(variants)
        }
        TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Char)
        | TypeKind::Value(ValueTypeKind::Float64)
        | TypeKind::Value(ValueTypeKind::Float32)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => ty,
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let inner = substitute_type_and_effect_params(types, inner, substitution);
            types.ty_option(inner)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let elements = elements
                .iter()
                .map(|&element| substitute_type_and_effect_params(types, element, substitution))
                .collect();
            types.ty_tuple(elements)
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_and_effect_params(types, arg, substitution))
                .collect();
            let eff = nominal.eff.as_ref().map(|row| {
                substitute_type_and_effect_params_in_effect_row(types, row, substitution)
            });
            types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
    }
}

fn substitute_type_and_effect_params_in_effect_row(
    types: &mut TypeStore,
    row: &EffectRow,
    substitution: &InstanceSubstitution,
) -> EffectRow {
    let mut terms = Vec::new();
    for &term in &row.terms {
        if let Some(name) = effect_row_param_marker_name(types, term)
            && let Some(bound) = substitution.effect_params.get(&name)
        {
            terms.extend(bound.terms.iter().copied().map(|bound_term| {
                substitute_type_and_effect_params(types, bound_term, substitution)
            }));
            continue;
        }
        terms.push(substitute_type_and_effect_params(types, term, substitution));
    }
    EffectRow::new(terms)
}

fn effect_row_param_marker_name(types: &TypeStore, ty: TypeId) -> Option<String> {
    match types.kind(ty) {
        TypeKind::Param(param)
            if param.decl_file.as_os_str() == crate::hir::EFFECT_ROW_PARAM_DECL_FILE =>
        {
            Some(param.name.clone())
        }
        _ => None,
    }
}

fn map_call_args_to_signature_params(
    params: &[CallableSignatureParam],
    args: &[CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    Some(out)
}

fn operand_type(
    types: &TypeStore,
    builtins: BuiltinTypes,
    locals: &[LocalDecl],
    operand: &Operand,
) -> Option<TypeId> {
    match operand {
        Operand::Local(local) => locals.get(local.as_u32() as usize).map(|decl| decl.ty),
        Operand::Const(ConstValue::Bool(_)) => Some(builtins.bool_),
        Operand::Const(ConstValue::Char) => Some(builtins.char_),
        Operand::Const(ConstValue::Unit) => Some(builtins.unit),
        Operand::Const(ConstValue::Int) => Some(builtins.int),
        Operand::Const(ConstValue::SynthInt(_)) => Some(builtins.int),
        Operand::Const(ConstValue::Float64) => Some(builtins.float64),
        Operand::Const(ConstValue::Float32) => Some(builtins.float32),
        Operand::Const(ConstValue::String) => Some(builtins.string),
    }
    .filter(|ty| {
        !type_contains_param(types, *ty)
            && !matches!(types.kind(*ty), TypeKind::Ref(RefTypeKind::Any))
    })
}

fn instance_request_is_concrete(
    types: &TypeStore,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> bool {
    type_args.iter().all(|&ty| !type_contains_param(types, ty))
        && eff_args
            .iter()
            .all(|row| !effect_row_contains_param(types, row))
}

fn effect_row_contains_param(types: &TypeStore, row: &EffectRow) -> bool {
    row.terms
        .iter()
        .copied()
        .any(|term| type_contains_param(types, term))
}

fn collect_type_param_names_in_type(types: &TypeStore, ty: TypeId, out: &mut Vec<String>) {
    match types.kind(ty) {
        TypeKind::Param(param) => {
            if param.decl_file.as_os_str() != crate::hir::EFFECT_ROW_PARAM_DECL_FILE
                && !out.contains(&param.name)
            {
                out.push(param.name.clone());
            }
        }
        TypeKind::StarProjection(star) => {
            collect_type_param_names_in_type(types, star.read_ty, out)
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            for &arg in &nominal.args {
                collect_type_param_names_in_type(types, arg, out);
            }
            if let Some(eff) = &nominal.eff {
                for &term in &eff.terms {
                    collect_type_param_names_in_type(types, term, out);
                }
            }
        }
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            collect_type_param_names_in_type(types, *inner, out)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            for &element in elements {
                collect_type_param_names_in_type(types, element, out);
            }
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            if let Some(receiver) = fun.receiver {
                collect_type_param_names_in_type(types, receiver, out);
            }
            for &param in &fun.params {
                collect_type_param_names_in_type(types, param, out);
            }
            collect_type_param_names_in_type(types, fun.return_ty, out);
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            for &variant in &union.variants {
                collect_type_param_names_in_type(types, variant, out);
            }
        }
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
        | TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Char)
        | TypeKind::Value(ValueTypeKind::Float64)
        | TypeKind::Value(ValueTypeKind::Float32)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
    }
}

fn function_type_has_effect_param(types: &TypeStore, fun_ty: TypeId) -> bool {
    let TypeKind::Ref(RefTypeKind::Function(fun)) = types.kind(fun_ty) else {
        return false;
    };
    fun.effects
        .terms
        .iter()
        .any(|&term| effect_row_param_marker_name(types, term).is_some())
}

fn type_contains_param(types: &TypeStore, ty: TypeId) -> bool {
    let mut stack = vec![ty];
    while let Some(id) = stack.pop() {
        match types.kind(id) {
            TypeKind::Param(_) => return true,
            TypeKind::StarProjection(star) => stack.push(star.read_ty),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                stack.extend(nominal.args.iter().copied());
                if let Some(eff) = &nominal.eff {
                    stack.extend(eff.terms.iter().copied());
                }
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                stack.extend(elements.iter().copied())
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if let Some(receiver) = fun.receiver {
                    stack.push(receiver);
                }
                stack.extend(fun.params.iter().copied());
                stack.push(fun.return_ty);
                stack.extend(fun.effects.terms.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => {
                stack.extend(union.variants.iter().copied())
            }
            TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
            | TypeKind::Value(ValueTypeKind::Unit)
            | TypeKind::Value(ValueTypeKind::Nothing)
            | TypeKind::Value(ValueTypeKind::Bool)
            | TypeKind::Value(ValueTypeKind::Char)
            | TypeKind::Value(ValueTypeKind::Float64)
            | TypeKind::Value(ValueTypeKind::Float32)
            | TypeKind::Value(ValueTypeKind::Int)
            | TypeKind::Value(ValueTypeKind::UInt)
            | TypeKind::Value(ValueTypeKind::IntN(_))
            | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
        }
    }
    false
}

fn collect_type_param_bindings(
    types: &TypeStore,
    declared_ty: TypeId,
    concrete_ty: TypeId,
    bindings: &mut HashMap<String, TypeId>,
) {
    match (types.kind(declared_ty), types.kind(concrete_ty)) {
        (TypeKind::Param(param), _) => match bindings.get(&param.name).copied() {
            Some(existing) if existing == concrete_ty => {}
            Some(_) => {}
            None => {
                bindings.insert(param.name.clone(), concrete_ty);
            }
        },
        (
            TypeKind::Ref(RefTypeKind::Nominal(declared)),
            TypeKind::Ref(RefTypeKind::Nominal(concrete)),
        )
        | (
            TypeKind::Value(ValueTypeKind::Nominal(declared)),
            TypeKind::Value(ValueTypeKind::Nominal(concrete)),
        ) => {
            if declared.fqn != concrete.fqn || declared.args.len() != concrete.args.len() {
                return;
            }
            for (decl_arg, concrete_arg) in declared.args.iter().zip(concrete.args.iter()) {
                collect_type_param_bindings(types, *decl_arg, *concrete_arg, bindings);
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Option(declared_inner)),
            TypeKind::Value(ValueTypeKind::Option(concrete_inner)),
        ) => {
            collect_type_param_bindings(types, *declared_inner, *concrete_inner, bindings);
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(declared_elements)),
            TypeKind::Value(ValueTypeKind::Tuple(concrete_elements)),
        ) => {
            if declared_elements.len() != concrete_elements.len() {
                return;
            }
            for (decl_elem, concrete_elem) in declared_elements.iter().zip(concrete_elements.iter())
            {
                collect_type_param_bindings(types, *decl_elem, *concrete_elem, bindings);
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(declared_fun)),
            TypeKind::Ref(RefTypeKind::Function(concrete_fun)),
        ) => {
            match (declared_fun.receiver, concrete_fun.receiver) {
                (Some(declared_receiver), Some(concrete_receiver)) => collect_type_param_bindings(
                    types,
                    declared_receiver,
                    concrete_receiver,
                    bindings,
                ),
                (None, None) => {}
                _ => return,
            }
            if declared_fun.params.len() != concrete_fun.params.len() {
                return;
            }
            for (decl_param, concrete_param) in
                declared_fun.params.iter().zip(concrete_fun.params.iter())
            {
                collect_type_param_bindings(types, *decl_param, *concrete_param, bindings);
            }
            collect_type_param_bindings(
                types,
                declared_fun.return_ty,
                concrete_fun.return_ty,
                bindings,
            );
        }
        (
            TypeKind::Ref(RefTypeKind::Union(declared_union)),
            TypeKind::Ref(RefTypeKind::Union(concrete_union)),
        ) => {
            if declared_union.variants.len() != concrete_union.variants.len() {
                return;
            }
            for (decl_variant, concrete_variant) in declared_union
                .variants
                .iter()
                .zip(concrete_union.variants.iter())
            {
                collect_type_param_bindings(types, *decl_variant, *concrete_variant, bindings);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{LocalSourceKind, MirLoweringFacts, lower_hir_file_for_dump_with_facts};
    use crate::session::Session;
    use crate::source::SourceFile;

    /// 构造“完整编译单元 facts + 仅部分文件贡献实例请求”的最小测试输入。
    fn prepare_typechecked_compilation_unit_inputs(
        session: &Session,
        files: Vec<SourceFile>,
        request_file_indices: &[usize],
    ) -> (
        Vec<(SourceFile, ast::File)>,
        Index,
        TypeEnv,
        TypeStore,
        Vec<MonomorphRequest>,
    ) {
        let mut files = files
            .into_iter()
            .map(|source| {
                let ast = parse_file(&source).unwrap();
                (source, ast)
            })
            .collect::<Vec<_>>();

        for (source, ast) in &files {
            typecheck::check_file_headers(source, ast).unwrap();
            typecheck::check_file_struct_decls(source, ast).unwrap();
        }

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> =
                Vec::with_capacity(session.sysroot().files.len() + files.len());
            for file in &session.sysroot().files {
                unit.push((&file.source, &file.ast));
            }
            for (source, ast) in &files {
                unit.push((source, ast));
            }
            Index::build(&unit).unwrap()
        };

        let mut resolved_headers = Vec::with_capacity(files.len());
        for (source, ast) in &files {
            resolved_headers.push(crate::resolve::check_file_headers(source, ast, &index).unwrap());
        }
        for ((source, ast), headers) in files.iter_mut().zip(resolved_headers.iter()) {
            crate::resolve::check_file_bodies(source, ast, &index, headers).unwrap();
        }

        let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        for (source, ast) in &files {
            env.extend_from_file(source, ast, &index).unwrap();
        }

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let mut monomorph_requests = Vec::new();
        for (file_index, ((source, ast), headers)) in
            files.iter().zip(resolved_headers.iter()).enumerate()
        {
            typecheck::check_file_annotations(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
            typecheck::check_file_type_refs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();

            if request_file_indices.contains(&file_index) {
                monomorph_requests.extend(
                    typecheck::check_file_exprs_with_monomorph_requests(
                        source,
                        ast,
                        &index,
                        &headers.imports,
                        &env,
                        &mut types,
                        builtins,
                    )
                    .unwrap(),
                );
            } else {
                typecheck::check_file_exprs(
                    source,
                    ast,
                    &index,
                    &headers.imports,
                    &env,
                    &mut types,
                    builtins,
                )
                .unwrap();
            }
        }

        (files, index, env, types, monomorph_requests)
    }

    #[test]
    fn materializer_filters_initial_monomorph_requests_by_call_site_source() {
        let sess = Session::new().unwrap();
        let main = SourceFile::new_virtual(
            "<mem>/request_source_main.scoop",
            r#"
package fixtures.materialize

fun main() {}
"#,
        );
        let support = SourceFile::new_virtual(
            "<mem>/request_source_support.scoop",
            r#"
package fixtures.materialize

fun <T> id(x: T): T {
    return x
}

fun support(): Int {
    return id<Int>(1)
}
"#,
        );
        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(&sess, vec![main, support], &[0, 1]);
        let main_path = files[0].0.path().to_path_buf();
        let support_path = files[1].0.path().to_path_buf();
        assert!(
            monomorph_requests.iter().any(|request| {
                request.key.symbol.fqn == "fixtures.materialize.id"
                    && request.request_source_path == support_path
            }),
            "test setup 应故意收集 support source 中的 id<Int> request"
        );

        let mut compilation_unit: Vec<(&SourceFile, &ast::File)> =
            Vec::with_capacity(sess.sysroot().files.len() + files.len());
        for file in &sess.sysroot().files {
            compilation_unit.push((&file.source, &file.ast));
        }
        for (source, ast) in &files {
            compilation_unit.push((source, ast));
        }

        let main_only = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            std::slice::from_ref(&main_path),
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();
        assert!(
            main_only
                .instance_keys
                .iter()
                .all(|key| key.template.fqn != "fixtures.materialize.id"),
            "support source 中收集到的 id<Int> request 不应在 main-only request roots 下成为 initial seed"
        );

        let support_roots = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            std::slice::from_ref(&support_path),
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();
        assert!(
            support_roots
                .instance_keys
                .iter()
                .any(|key| key.template.fqn == "fixtures.materialize.id"),
            "同一个 request 来自 request source 时仍应正常进入 initial seeds"
        );
    }

    #[test]
    fn generic_mir_template_for_dump_stays_free_of_hir_level_instances() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_generic_template_boundary.scoop",
            r#"
package fixtures.materialize

class Box<T>(val value: T) {
    fun get(): T {
        return value
    }
}

fun id<T>(x: T): T {
    return x
}

fun entry(): Int {
    val box: Box<Int> = Box(1)
    val a = id(1)
    return a + box.get()
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let mut lowered_hir =
            crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
                &inputs.index,
                &compilation_unit,
                &compilation_unit,
                Some(&inputs.env),
                &inputs.typecheck_types,
            )
            .unwrap();

        let hir_fun_fqns: Vec<&str> = lowered_hir
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::hir::Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(hir_fun_fqns.contains(&"fixtures.materialize.id"));
        assert!(hir_fun_fqns.contains(&"fixtures.materialize.entry"));
        assert!(
            hir_fun_fqns.iter().all(|fqn| !fqn.contains("::<")),
            "generic typed HIR 不应预先混入 standalone generic HIR instances: {hir_fun_fqns:?}"
        );

        let hir_member_fqns: Vec<&str> = lowered_hir
            .member_funs
            .iter()
            .map(|fun| fun.fqn.as_str())
            .collect::<Vec<_>>();
        assert!(hir_member_fqns.contains(&"fixtures.materialize.Box.get"));
        assert!(
            hir_member_fqns.iter().all(|fqn| !fqn.contains("::<")),
            "generic typed HIR 不应预先混入 owner-specialized member instances: {hir_member_fqns:?}"
        );

        let builtins = lowered_hir.types.intern_builtins();
        let facts = MirLoweringFacts::from_lowered_hir(&lowered_hir);
        let generic_file = lower_hir_file_for_dump_with_facts(
            builtins,
            &mut lowered_hir.types,
            &lowered_hir.file,
            &lowered_hir.member_funs,
            &facts,
        );

        let mir_fun_fqns: Vec<&str> = generic_file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(mir_fun_fqns.contains(&"fixtures.materialize.id"));
        assert!(mir_fun_fqns.contains(&"fixtures.materialize.Box.get"));
        assert!(
            mir_fun_fqns.iter().all(|fqn| !fqn.contains("::<")),
            "generic MIR template 不应在 materializer 之前混入 monomorphic roots: {mir_fun_fqns:?}"
        );
    }

    #[test]
    fn materialize_for_dump_dedups_repeated_instance_requests() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_instance_dedup.scoop",
            r#"
package fixtures.materialize

fun id<T>(x: T): T {
    return x
}

fun entry(): Int {
    val a = id(1)
    val b = id(2)
    return a + b
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let id_instances = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.id")
            .collect::<Vec<_>>();
        assert_eq!(
            id_instances.len(),
            1,
            "重复请求同一 generic instance 时应只保留一个 InstanceKey"
        );
        assert_eq!(
            materialized
                .file
                .items
                .iter()
                .filter(|item| matches!(
                    item,
                    Item::Fun(fun) if fun.fqn == "fixtures.materialize.id::<Int>"
                ))
                .count(),
            1,
            "per-InstanceKey cache 应确保同一实例只 materialize 一次"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_distinguishes_same_type_args_with_different_effect_rows()
     {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_compilation_unit_effect_rows.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Int
}

effect Zap {
    fun pong(): Int
}

fun <T, eff E = Pure> wrap(x: T): T / E {
    return x
}

fun entry(): Unit / (Boom + Zap) {
    val a = wrap<Int, eff Boom>(1)
    val b = wrap<Int, eff Zap>(2)
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[source.path().to_path_buf()],
            &inputs.index,
            Some(&inputs.env),
            &inputs.typecheck_types,
            &inputs.monomorph_requests,
        )
        .unwrap();

        let wrap_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.wrap")
            .collect::<Vec<_>>();
        assert_eq!(wrap_keys.len(), 2);
        assert!(wrap_keys.iter().all(|key| key.type_args.len() == 1));
        assert!(wrap_keys.iter().all(|key| key.eff_args.len() == 1));
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun)
                    if fun.fqn == "fixtures.materialize.wrap::<Int, eff fixtures.materialize.Boom>"
            )),
            "编译单元 materialization 应保留 Boom effect-row 实例"
        );
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun)
                    if fun.fqn == "fixtures.materialize.wrap::<Int, eff fixtures.materialize.Zap>"
            )),
            "编译单元 materialization 应保留 Zap effect-row 实例"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_keeps_cross_file_effect_roots_when_request_sources_are_subset()
     {
        let sess = Session::new().unwrap();
        let helper_source = SourceFile::new_virtual(
            "<mem>/materialize_cross_file_helper.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

fun <eff E = Pure> id(x: Int): Int / E {
    return x
}

fun <eff E = Pure> wrap(x: Int): Int / E {
    return id<eff E>(x)
}
"#,
        );
        let main_source = SourceFile::new_virtual(
            "<mem>/materialize_cross_file_main.scoop",
            r#"
package fixtures.materialize

fun entry(): Int / Boom {
    return wrap<eff Boom>(1)
}
"#,
        );
        let main_source_path = main_source.path().to_path_buf();

        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(
                &sess,
                vec![helper_source, main_source],
                &[1],
            );
        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();

        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[main_source_path],
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();

        let wrap_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.wrap")
            .collect::<Vec<_>>();
        let id_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.id")
            .collect::<Vec<_>>();
        assert_eq!(wrap_keys.len(), 1);
        assert_eq!(id_keys.len(), 1);
        assert_eq!(wrap_keys[0].eff_args.len(), 1);
        assert_eq!(id_keys[0].eff_args.len(), 1);
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun)
                    if fun.fqn == "fixtures.materialize.wrap::<eff fixtures.materialize.Boom>"
            )),
            "跨文件 helper 中定义的 wrap 应在编译单元 materialization 中保留 concrete root"
        );
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun)
                    if fun.fqn == "fixtures.materialize.id::<eff fixtures.materialize.Boom>"
            )),
            "跨文件 helper 中嵌套调用的 id 应通过 helper 文件内的 site binding 继续 materialize"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_skips_unreachable_generic_requests_from_non_request_sources()
     {
        let sess = Session::new().unwrap();
        let helper_source = SourceFile::new_virtual(
            "<mem>/materialize_unreachable_helper.scoop",
            r#"
package fixtures.materialize

fun <T> id(x: T): T {
    return x
}

fun helperOnly(): Int {
    return id(1)
}
"#,
        );
        let main_source = SourceFile::new_virtual(
            "<mem>/materialize_unreachable_main.scoop",
            r#"
package fixtures.materialize

fun entry(): Int {
    return 0
}
"#,
        );
        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(
                &sess,
                vec![helper_source, main_source.clone()],
                &[1],
            );
        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();
        let request_source_paths = vec![main_source.path().to_path_buf()];
        let lowered =
            crate::hir::lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
            &index,
            &compilation_unit,
            &compilation_unit,
            &monomorph_requests,
            Some(&env),
            &types,
            crate::hir::MirInstanceCollectionOptions {
                request_source_paths: &request_source_paths,
                request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
                opt_level: OptLevel::O2,
            },
        )
        .unwrap();

        assert!(
            lowered.file.items.iter().any(|item| matches!(
                item,
                crate::hir::Item::Fun(fun) if fun.fqn == "fixtures.materialize.helperOnly"
            )),
            "support source 仍应参与 lowering，保证 helper 实现体继续进入 HIR 兼容输出"
        );
        assert!(
            lowered.file.items.iter().all(|item| !matches!(
                item,
                crate::hir::Item::Fun(fun) if fun.fqn == "fixtures.materialize.id::<Int>"
            )),
            "未被 request-root 路径触达的 helper-only generic 实例不应被物化进 HIR 兼容输出"
        );
    }

    #[test]
    fn request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_unreachable_mir_block.scoop",
            r#"
package fixtures.materialize

fun <T> id(x: T): T {
    return x
}

fun main(): Int {
    return 0
}
"#,
        );
        let source_path = source.path().to_path_buf();

        let (files, index, env, typecheck_types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(&sess, vec![source.clone()], &[0]);
        assert!(
            monomorph_requests
                .iter()
                .all(|request| request.key.symbol.fqn != "fixtures.materialize.id"),
            "test setup 不应通过源代码本身收集 id<T> request"
        );

        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();
        let template_infos = collect_generic_template_infos(&compilation_unit);
        let callable_body_infos = collect_callable_body_infos(&compilation_unit);
        let (top_level_fun_value_refs, top_level_fun_call_bindings) =
            collect_site_instance_bindings(&compilation_unit);
        let mut lowered_hir =
            crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
                &index,
                &compilation_unit,
                &compilation_unit,
                Some(&env),
                &typecheck_types,
            )
            .unwrap();
        let request_root_fun_keys = collect_request_root_fun_keys(
            &lowered_hir,
            std::slice::from_ref(&source_path),
            &index,
            crate::mir::MaterializeRequestRootMode::EntryMain { fqn: None },
        );
        assert_eq!(
            request_root_fun_keys
                .iter()
                .map(|key| key.fqn.as_str())
                .collect::<Vec<_>>(),
            vec!["fixtures.materialize.main"],
            "entry-main 模式下测试应只从 main 扫描 request roots"
        );
        let request_sources = [source_path.clone()].into_iter().collect::<HashSet<_>>();
        let callable_signatures = collect_callable_signature_infos(&lowered_hir);
        let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
            &mut lowered_hir,
            &typecheck_types,
            &top_level_fun_call_bindings,
        );
        assert!(
            hir_direct_instance_keys_by_fun
                .values()
                .flatten()
                .all(|key| key.template.fqn != "fixtures.materialize.id"),
            "test setup 不应通过 HIR fallback 预先发现 id<T> 实例"
        );
        let known_receiver_subclasses =
            crate::devirtualize::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
        let class_vtables = lowered_hir.class_vtables.clone();
        let interfaces = lowered_hir.interfaces.clone();
        let class_itables = lowered_hir.class_itables.clone();
        let builtins = lowered_hir.types.intern_builtins();
        let facts = MirLoweringFacts::from_lowered_hir(&lowered_hir);
        let mut generic_file = lower_hir_file_for_dump_with_facts(
            builtins,
            &mut lowered_hir.types,
            &lowered_hir.file,
            &lowered_hir.member_funs,
            &facts,
        );
        append_unreachable_id_call_to_main(&mut generic_file, builtins);
        let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
        let types = lowered_hir.types;

        let mut materializer = MirInstanceMaterializer::new(
            generic_file,
            types,
            builtins,
            MaterializerConstructionInputs {
                typecheck_types: &typecheck_types,
                template_infos,
                callable_body_infos,
                callable_signatures,
                known_receiver_subclasses,
                class_vtables,
                interfaces,
                class_itables,
                top_level_fun_value_refs,
                top_level_fun_call_bindings,
                top_level_immutable_values,
                request_sources,
                request_root_mode: crate::mir::MaterializeRequestRootMode::EntryMain { fqn: None },
                request_root_fun_keys,
            },
            OptLevel::O0,
            false,
            false,
        )
        .unwrap();
        materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;
        let initial_requests = materializer
            .seed_requests(&typecheck_types, &monomorph_requests)
            .unwrap();
        let initial_id_keys = initial_requests
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.id")
            .collect::<Vec<_>>();
        assert!(
            initial_id_keys.is_empty(),
            "MIR 不可达 block 中的 id<Int> direct-call 不应进入 initial requests：{initial_id_keys:#?}"
        );
        assert!(
            initial_requests.is_empty(),
            "test setup 不应产生任何 initial requests：{initial_requests:#?}"
        );
        let materialized = materializer.run(initial_requests).unwrap();

        let id_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.id")
            .collect::<Vec<_>>();
        assert!(
            id_keys.is_empty(),
            "MIR 不可达 block 中的 id<Int> direct-call 不应产生额外实例：{id_keys:#?}"
        );
        assert!(
            materialized.file.items.iter().all(|item| !matches!(
                item,
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.id::<Int>"
            )),
            "MIR 不可达 block 中的 id<Int> direct-call 不应物化为 callable body"
        );
    }

    fn append_unreachable_id_call_to_main(generic_file: &mut File, builtins: BuiltinTypes) {
        let main_fun = generic_file
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.main" => Some(fun),
                _ => None,
            })
            .expect("test setup should contain fixtures.materialize.main");
        let body = main_fun.body.as_mut().expect("main should have MIR body");
        let call_span = Span::new(10_000, 10_010);
        let result = body.push_local(LocalDecl {
            span: call_span,
            name: Some("unreachable_id_result".to_string()),
            ty: builtins.int,
            source: LocalSourceKind::SourceLocal,
        });
        let unreachable_block = body.push_block(crate::mir::BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: call_span,
                kind: StatementKind::Assign {
                    target: result,
                    value: Rvalue::Call {
                        site_id: crate::mir::SiteId::from_raw(0),
                        kind: CallKind::Direct {
                            callee_fqn: "fixtures.materialize.id".to_string(),
                        },
                        args: vec![CallArg {
                            span: call_span,
                            name: None,
                            value: Operand::Const(ConstValue::Int),
                        }],
                    },
                },
            }],
            terminator: Terminator {
                span: call_span,
                kind: TerminatorKind::Return {
                    value: Some(Operand::Local(result)),
                },
                unwind: crate::mir::UnwindAction::NoUnwind,
            },
        });

        assert!(
            body.unreachable_blocks()
                .unwrap()
                .contains(&unreachable_block),
            "test setup 应追加一个结构上不可达的 MIR block"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_handles_owner_specialized_effect_generic_member_calls()
     {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_owner_specialized_effect_member.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

class Box<T>(val value: T) {
    fun <eff E = Pure> forward(): T / E {
        return value
    }
}

fun <eff E = Pure> wrap(box: Box<Int>): Int / E {
    return box.forward<eff E>()
}

fun entry(): Int / Boom {
    return wrap<eff Boom>(Box(1))
}
"#,
        );

        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(&sess, vec![source.clone()], &[0]);
        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();

        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[source.path().to_path_buf()],
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();

        let forward_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.Box.forward")
            .collect::<Vec<_>>();
        assert_eq!(forward_keys.len(), 1);
        assert_eq!(forward_keys[0].type_args.len(), 1);
        assert_eq!(forward_keys[0].eff_args.len(), 1);
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun)
                    if fun.fqn
                        == "fixtures.materialize.Box.forward::<Int, eff fixtures.materialize.Boom>"
            )),
            "generic owner + effect-generic member direct-call 应产出同时携带 owner args 与 eff_args 的 concrete MIR root"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_seeds_owner_specialized_getter_from_request_roots()
     {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_owner_specialized_getter.scoop",
            r#"
package fixtures.materialize

struct Box<T>(val value: T) {
    val doubled: T
        get() = this.value
}

fun entry(): Int {
    val box: Box<Int> = Box(1)
    val unused: Box<String> = Box("x")
    return box.doubled
}
"#,
        );

        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(&sess, vec![source.clone()], &[0]);
        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();

        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[source.path().to_path_buf()],
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();

        let getter_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.Box.doubled")
            .collect::<Vec<_>>();
        assert_eq!(getter_keys.len(), 1);
        assert_eq!(getter_keys[0].type_args.len(), 1);
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<Int>"
            )),
            "generic owner getter 应从请求根非调用式访问进入 materialization"
        );
        assert!(
            !materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<String>"
            )),
            "请求根扫描应保持 call-site driven，不应因为 `Box<String>` 出现在 TypeStore 中就 eager materialize 未调用 getter"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_cross_file_non_generic_helper()
     {
        let sess = Session::new().unwrap();
        let helper = SourceFile::new_virtual(
            "<mem>/materialize_owner_specialized_getter_helper.scoop",
            r#"
package fixtures.materialize

struct Box<T>(val value: T) {
    val doubled: T
        get() = this.value
}

fun helper(box: Box<Int>): Int {
    return box.doubled
}
"#,
        );
        let main = SourceFile::new_virtual(
            "<mem>/materialize_owner_specialized_getter_main.scoop",
            r#"
package fixtures.materialize

fun entry(): Int {
    return helper(Box(1))
}
"#,
        );

        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(&sess, vec![helper, main.clone()], &[1]);
        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();

        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[main.path().to_path_buf()],
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();

        let getter_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.Box.doubled")
            .collect::<Vec<_>>();
        assert_eq!(getter_keys.len(), 1);
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<Int>"
            )),
            "跨文件非泛型 helper 中触发的 owner-specialized getter 应继续进入 MIR materialization"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_non_generic_helper_called_by_generic_instance()
     {
        let sess = Session::new().unwrap();
        let helper = SourceFile::new_virtual(
            "<mem>/materialize_owner_specialized_getter_helper_via_generic_instance.scoop",
            r#"
package fixtures.materialize

struct Box<T>(val value: T) {
    val doubled: T
        get() = this.value
}

fun helper(box: Box<Int>): Int {
    return box.doubled
}
"#,
        );
        let main = SourceFile::new_virtual(
            "<mem>/materialize_owner_specialized_getter_generic_instance_main.scoop",
            r#"
package fixtures.materialize

fun <eff E = Pure> wrap(box: Box<Int>): Int / E {
    return helper(box)
}

fun entry(): Int {
    return wrap(Box(1))
}
"#,
        );

        let (files, index, env, types, monomorph_requests) =
            prepare_typechecked_compilation_unit_inputs(&sess, vec![helper, main.clone()], &[1]);
        let compilation_unit = files
            .iter()
            .map(|(source, ast)| (source, ast))
            .collect::<Vec<_>>();

        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[main.path().to_path_buf()],
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
        )
        .unwrap();

        let getter_keys = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.Box.doubled")
            .collect::<Vec<_>>();
        assert_eq!(getter_keys.len(), 1);
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<Int>"
            )),
            "generic instance 经由非泛型 helper 可达的 owner-specialized getter 应继续进入 MIR materialization"
        );
    }

    #[test]
    fn typechecked_compilation_unit_materialization_reaches_generic_calls_through_non_generic_async_closure_body()
     {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_async_closure_helper_reachability.scoop",
            r#"
package fixtures.materialize

import scoop.core.*

fun entry(): Int {
    val task: Task<Int> = async {
        val inner: Task<Int> = async { 41 }
        val value: Int = await inner
        value + 1
    }
    return 0
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[source.path().to_path_buf()],
            &inputs.index,
            Some(&inputs.env),
            &inputs.typecheck_types,
            &inputs.monomorph_requests,
        )
        .unwrap();

        for fqn in [
            "scoop.core.__task_create::<Int>",
            "scoop.core.__task_step_ready::<Int>",
            "scoop.core.__task_step_pending::<Int>",
        ] {
            assert!(
                materialized.file.items.iter().any(|item| matches!(
                    item,
                    Item::Fun(fun) if fun.fqn == fqn
                )),
                "non-generic async closure body 内的 task helper 应继续进入编译单元 materialization，缺失 `{fqn}`"
            );
        }
    }

    #[test]
    fn dump_materialization_inputs_keep_eff_args_for_extension_direct_call_binding() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_extension_binding_effect.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

fun <eff E = Pure> Int.forward(): Int / E {
    return this
}

fun <eff E = Pure> wrap(x: Int): Int / E {
    return x.forward<eff E>()
}

fun entry(): Int / Boom {
    return wrap<eff Boom>(1)
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let bindings = call_bindings
            .values()
            .filter(|binding| binding.fqn == "fixtures.materialize.forward")
            .collect::<Vec<_>>();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        assert_eq!(binding.decl_file, source.path().to_path_buf());
        assert!(binding.decl_span.start < binding.decl_span.end);
        assert!(binding.type_args.is_empty());
        assert_eq!(binding.eff_args.len(), 1);
        assert!(
            !binding.eff_args[0].is_pure(),
            "extension direct-call 的 TopLevelFunCallBinding 不应退回 Pure"
        );

        let keys = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "fixtures.materialize.forward")
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].key.type_args.is_empty());
        assert_eq!(keys[0].key.eff_args.len(), 1);
        assert!(
            !keys[0].key.eff_args[0].is_pure(),
            "extension direct-call 的 monomorph key 应保留非 Pure 的 eff_args"
        );
    }

    #[test]
    fn dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_member_binding_effect.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

class Box() {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}

fun entry(): Int / Boom {
    return wrap<eff Boom>(Box())
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let bindings = call_bindings
            .values()
            .filter(|binding| binding.fqn == "fixtures.materialize.Box.forward")
            .collect::<Vec<_>>();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        assert_eq!(binding.decl_file, source.path().to_path_buf());
        assert!(binding.decl_span.start < binding.decl_span.end);
        assert!(binding.type_args.is_empty());
        assert_eq!(binding.eff_args.len(), 1);
        assert!(
            !binding.eff_args[0].is_pure(),
            "成员 direct-call 的 TopLevelFunCallBinding 不应退回 Pure"
        );

        let keys = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.forward")
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].key.type_args.is_empty());
        assert_eq!(keys[0].key.eff_args.len(), 1);
        assert!(
            !keys[0].key.eff_args[0].is_pure(),
            "成员 direct-call 的 monomorph key 应保留非 Pure 的 eff_args"
        );
    }

    #[test]
    fn dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding_from_lambda() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_member_lambda_binding_effect.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

class Box() {
    fun <eff E = Pure> lift(f: () -> Int / E): Int / E {
        return f()
    }
}

fun entry(): Int / Boom {
    val box: Box = Box()
    return box.lift({
        perform Boom.ping()
        1
    })
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let bindings = call_bindings
            .values()
            .filter(|binding| binding.fqn == "fixtures.materialize.Box.lift")
            .collect::<Vec<_>>();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        assert_eq!(binding.decl_file, source.path().to_path_buf());
        assert!(binding.decl_span.start < binding.decl_span.end);
        assert!(binding.type_args.is_empty());
        assert_eq!(binding.eff_args.len(), 1);
        assert!(
            !binding.eff_args[0].is_pure(),
            "lambda-derived 成员 direct-call binding 应保留非 Pure eff_args"
        );

        let keys = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.lift")
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].key.type_args.is_empty());
        assert_eq!(keys[0].key.eff_args.len(), 1);
        assert!(
            !keys[0].key.eff_args[0].is_pure(),
            "lambda-derived 成员 direct-call monomorph key 应保留非 Pure eff_args"
        );
    }

    #[test]
    fn dump_materialization_inputs_keep_owner_type_args_and_eff_args_for_operator_overload_binding()
    {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_operator_overload_binding_effect.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

struct Box<T>(val value: Int) {
    fun <eff E = Boom> plus(other: Box<T>): Box<T> / Boom {
        perform Boom.ping()
        return Box { value: this.value + other.value }
    }
}

fun entry(): Box<Int> / Boom {
    val lhs: Box<Int> = Box { value: 1 }
    val rhs: Box<Int> = Box { value: 2 }
    return lhs + rhs
}
"#,
        );

        let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let builtins = inputs.typecheck_types.intern_builtins();

        let bindings = call_bindings
            .values()
            .filter(|binding| binding.fqn == "fixtures.materialize.Box.plus")
            .collect::<Vec<_>>();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        assert_eq!(binding.decl_file, source.path().to_path_buf());
        assert!(binding.decl_span.start < binding.decl_span.end);
        assert_eq!(binding.type_args.len(), 1);
        assert_eq!(
            binding.type_args[0], builtins.int,
            "operator-overload binding 应保留 owner specialization 的 Int type arg"
        );
        assert_eq!(binding.eff_args.len(), 1);
        assert!(
            !binding.eff_args[0].is_pure(),
            "operator-overload binding 不应把默认 `Boom` eff_arg 退回 Pure"
        );

        let keys = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.plus")
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key.type_args.len(), 1);
        assert_eq!(
            keys[0].key.type_args[0], builtins.int,
            "operator-overload monomorph key 应保留 owner specialization 的 Int type arg"
        );
        assert_eq!(keys[0].key.eff_args.len(), 1);
        assert!(
            !keys[0].key.eff_args[0].is_pure(),
            "operator-overload monomorph key 不应把默认 `Boom` eff_arg 退回 Pure"
        );
    }

    #[test]
    fn dump_materialization_inputs_keep_owner_type_args_and_eff_args_for_compare_to_binding() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_compare_to_binding_effect.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

struct Box<T>(val value: Int) {
    fun <eff E = Boom> compareTo(other: Box<T>): Int / Boom {
        perform Boom.ping()
        return this.value - other.value
    }
}

fun entry(): Bool / Boom {
    val lhs: Box<Int> = Box { value: 1 }
    val rhs: Box<Int> = Box { value: 2 }
    return lhs < rhs
}
"#,
        );

        let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let builtins = inputs.typecheck_types.intern_builtins();

        let bindings = call_bindings
            .values()
            .filter(|binding| binding.fqn == "fixtures.materialize.Box.compareTo")
            .collect::<Vec<_>>();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        assert_eq!(binding.decl_file, source.path().to_path_buf());
        assert!(binding.decl_span.start < binding.decl_span.end);
        assert_eq!(binding.type_args.len(), 1);
        assert_eq!(
            binding.type_args[0], builtins.int,
            "compareTo binding 应保留 owner specialization 的 Int type arg"
        );
        assert_eq!(binding.eff_args.len(), 1);
        assert!(
            !binding.eff_args[0].is_pure(),
            "compareTo binding 不应把默认 `Boom` eff_arg 退回 Pure"
        );

        let keys = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.compareTo")
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key.type_args.len(), 1);
        assert_eq!(
            keys[0].key.type_args[0], builtins.int,
            "compareTo monomorph key 应保留 owner specialization 的 Int type arg"
        );
        assert_eq!(keys[0].key.eff_args.len(), 1);
        assert!(
            !keys[0].key.eff_args[0].is_pure(),
            "compareTo monomorph key 不应把默认 `Boom` eff_arg 退回 Pure"
        );
    }

    #[test]
    fn dump_materialization_inputs_keep_precise_type_args_for_object_member_call_results() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_object_member_call_binding.scoop",
            r#"
package fixtures.materialize

import scoop.core.*

object Helper {
    fun run(seed: Int): Int {
        println(seed)
        return seed + 1
    }
}

fun main(): Int {
    val result: Int = Helper.run(41)
    println(result)
    return 0
}
"#,
        );

        let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let builtins = inputs.typecheck_types.intern_builtins();

        let println_type_args = call_bindings
            .iter()
            .filter(|((site_path, _), binding)| {
                *site_path == source.path() && binding.fqn == "scoop.core.println"
            })
            .map(|(_, binding)| {
                assert_eq!(binding.type_args.len(), 1);
                binding.type_args[0]
            })
            .collect::<Vec<_>>();
        assert_eq!(
            println_type_args.len(),
            2,
            "object member call 场景中应记录 2 个用户 println 调用"
        );
        assert!(
            println_type_args.iter().all(|&ty| ty == builtins.int),
            "object member call 场景中的 println binding 不应退回 Any：{println_type_args:?}"
        );

        let println_monomorph_type_args = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "scoop.core.println")
            .map(|request| {
                assert_eq!(request.key.type_args.len(), 1);
                request.key.type_args[0]
            })
            .collect::<Vec<_>>();
        assert!(
            !println_monomorph_type_args.is_empty(),
            "object member call 场景中至少应保留 request-root 上的 println monomorph key"
        );
        assert!(
            println_monomorph_type_args
                .iter()
                .all(|&ty| ty == builtins.int),
            "object member call 场景中的 println monomorph key 不应退回 Any：{println_monomorph_type_args:?}"
        );

        let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
            &compilation_unit,
            &[source.path().to_path_buf()],
            &inputs.index,
            Some(&inputs.env),
            &inputs.typecheck_types,
            &inputs.monomorph_requests,
        )
        .unwrap();
        let materialized_printlns = materialized
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) if fun.fqn.starts_with("scoop.core.println::<") => {
                    Some(fun.fqn.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            materialized_printlns
                .iter()
                .filter(|fqn| *fqn == "scoop.core.println::<Int>")
                .count()
                >= 1,
            "object member call 场景中应 materialize 出 println::<Int>：{materialized_printlns:#?}"
        );
        assert!(
            !materialized_printlns
                .iter()
                .any(|fqn| fqn == "scoop.core.println::<Any>"),
            "object member call 场景中不应 materialize 出 println::<Any>：{materialized_printlns:#?}"
        );
    }

    #[test]
    fn dump_materialization_inputs_keep_precise_type_args_for_chained_member_access_call_args() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_chained_member_access_binding.scoop",
            r#"
package fixtures.materialize

import scoop.core.*

struct Tag(val label: String, val score: Int)

class Node(val name: String, val tag: Tag, val value: Int)

class Holder(val node: Node)

fun makeHolder(): Holder {
    val node: Node = Node("root", Tag { label: "alpha", score: 7 }, 42)
    return Holder(node)
}

fun main() {
    val holder: Holder = makeHolder()
    val label: String = holder.node.tag.label
    println(label)
    println(holder.node.tag.label)
    println(holder.node.tag.score)
}
"#,
        );

        let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let compilation_unit = inputs
            .prepared_files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
        let builtins = inputs.typecheck_types.intern_builtins();

        let println_type_args = call_bindings
            .iter()
            .filter(|((site_path, _), binding)| {
                *site_path == source.path() && binding.fqn == "scoop.core.println"
            })
            .map(|(_, binding)| {
                assert_eq!(binding.type_args.len(), 1);
                binding.type_args[0]
            })
            .collect::<Vec<_>>();

        assert_eq!(println_type_args.len(), 3);
        assert!(
            !println_type_args.contains(&builtins.any),
            "链式成员访问作为实参时，println 不应退回到 `Any` 实例"
        );
        assert_eq!(
            println_type_args
                .iter()
                .filter(|&&ty| ty == builtins.string)
                .count(),
            2,
            "label 与 holder.node.tag.label 都应绑定到 println::<String>"
        );
        assert_eq!(
            println_type_args
                .iter()
                .filter(|&&ty| ty == builtins.int)
                .count(),
            1,
            "holder.node.tag.score 应绑定到 println::<Int>"
        );

        let println_monomorph_type_args = inputs
            .monomorph_requests
            .iter()
            .filter(|request| request.key.symbol.fqn == "scoop.core.println")
            .map(|request| {
                assert_eq!(request.key.type_args.len(), 1);
                request.key.type_args[0]
            })
            .collect::<Vec<_>>();
        assert!(
            !println_monomorph_type_args.contains(&builtins.any),
            "链式成员访问作为实参时，println 的 monomorph key 不应退回到 `Any`"
        );

        let template_catalog = collect_generic_template_infos(&compilation_unit);
        let callable_body_infos = collect_callable_body_infos(&compilation_unit);
        let (top_level_fun_value_refs, top_level_fun_call_bindings) =
            collect_site_instance_bindings(&compilation_unit);
        let mut lowered_hir =
            crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
                &inputs.index,
                &compilation_unit,
                &compilation_unit,
                Some(&inputs.env),
                &inputs.typecheck_types,
            )
            .unwrap();
        let request_root_fun_keys = collect_request_root_fun_keys(
            &lowered_hir,
            &[source.path().to_path_buf()],
            &inputs.index,
            crate::mir::MaterializeRequestRootMode::RequestSources,
        );
        let request_sources = [source.path().to_path_buf()]
            .into_iter()
            .collect::<HashSet<_>>();
        let callable_signatures = collect_callable_signature_infos(&lowered_hir);
        let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
            &mut lowered_hir,
            &inputs.typecheck_types,
            &call_bindings,
        );
        let hir_direct_instance_keys = hir_direct_instance_keys_by_fun
            .values()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        let hir_direct_println_requests = hir_direct_instance_keys
            .iter()
            .filter(|key| key.template.fqn == "scoop.core.println")
            .map(|key| {
                (
                    key.template.source_path.clone(),
                    key.template.decl_span,
                    key.type_args.clone(),
                    key.eff_args.clone(),
                )
            })
            .collect::<Vec<_>>();
        let known_receiver_subclasses =
            crate::devirtualize::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
        let class_vtables = lowered_hir.class_vtables.clone();
        let interfaces = lowered_hir.interfaces.clone();
        let class_itables = lowered_hir.class_itables.clone();
        let builtins = lowered_hir.types.intern_builtins();
        let facts = MirLoweringFacts::from_lowered_hir(&lowered_hir);
        let generic_file = lower_hir_file_for_dump_with_facts(
            builtins,
            &mut lowered_hir.types,
            &lowered_hir.file,
            &lowered_hir.member_funs,
            &facts,
        );
        let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
        let types = lowered_hir.types;
        let mut materializer = MirInstanceMaterializer::new(
            generic_file,
            types,
            builtins,
            MaterializerConstructionInputs {
                typecheck_types: &inputs.typecheck_types,
                template_infos: template_catalog,
                callable_body_infos,
                callable_signatures,
                known_receiver_subclasses,
                class_vtables,
                interfaces,
                class_itables,
                top_level_fun_value_refs,
                top_level_fun_call_bindings,
                top_level_immutable_values,
                request_sources,
                request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
                request_root_fun_keys,
            },
            OptLevel::O2,
            true,
            true,
        )
        .unwrap();
        materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;
        let request_root_println_bindings = materializer
            .request_root_funs
            .iter()
            .flat_map(|reachable_fun| {
                reachable_fun
                    .fun
                    .body
                    .iter()
                    .flat_map(|body| body.blocks.iter())
                    .flat_map(|block| block.stmts.iter())
                    .filter_map(|stmt| match &stmt.kind {
                        StatementKind::Assign {
                            value:
                                Rvalue::Call {
                                    kind: CallKind::Direct { callee_fqn },
                                    ..
                                },
                            ..
                        } if callee_fqn == "scoop.core.println" => Some((
                            reachable_fun.source_path.clone(),
                            stmt.span,
                            materializer
                                .lookup_site_instance_binding(&reachable_fun.source_path, stmt.span)
                                .map(|binding| binding.type_args.clone()),
                        )),
                        _ => None,
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            request_root_println_bindings.len(),
            3,
            "request root 中应恰好看到 3 个用户 println 调用：{request_root_println_bindings:#?}"
        );
        assert!(
            request_root_println_bindings
                .iter()
                .all(|(_, _, binding)| binding.is_some()),
            "request root 的 println 调用必须全部命中 site binding：{request_root_println_bindings:#?}"
        );
        assert!(
            request_root_println_bindings.iter().all(|(_, _, binding)| {
                binding
                    .as_ref()
                    .is_some_and(|type_args| !type_args.contains(&builtins.any))
            }),
            "request root 的 println 调用命中的 binding 不应含 Any：{request_root_println_bindings:#?}"
        );
        let mut reachable_generic_calls = Vec::new();
        let mut visited_non_generic = std::collections::HashSet::new();
        let mut stack = materializer.request_root_funs.clone();
        while let Some(reachable_fun) = stack.pop() {
            let scan_key = (reachable_fun.source_path.clone(), reachable_fun.fun.span);
            if !visited_non_generic.insert(scan_key) {
                continue;
            }
            let Some(body) = &reachable_fun.fun.body else {
                continue;
            };
            for block in &body.blocks {
                for stmt in &block.stmts {
                    let StatementKind::Assign {
                        value:
                            Rvalue::Call {
                                kind: CallKind::Direct { callee_fqn },
                                args,
                                ..
                            },
                        ..
                    } = &stmt.kind
                    else {
                        continue;
                    };
                    if let Some(instance_key) =
                        materializer.infer_direct_call_instance(DirectCallInferenceInput {
                            template_source_path: &reachable_fun.source_path,
                            call_span: stmt.span,
                            callee_fqn,
                            args,
                            result_ty: None,
                            locals: &body.locals,
                            substitution: &InstanceSubstitution::default(),
                        })
                    {
                        reachable_generic_calls.push((
                            reachable_fun.fun.fqn.clone(),
                            reachable_fun.source_path.clone(),
                            stmt.span,
                            materializer.instance_fqn(&instance_key),
                        ));
                        continue;
                    }
                    if let Some(reachable_callee) = materializer.resolve_non_generic_direct_callee(
                        &reachable_fun.source_path,
                        stmt.span,
                        callee_fqn,
                    ) {
                        stack.push(reachable_callee);
                    }
                }
            }
        }
        let reachable_println_calls = reachable_generic_calls
            .iter()
            .filter(|(_, _, _, instance_fqn)| instance_fqn.starts_with("scoop.core.println::<"))
            .collect::<Vec<_>>();
        assert!(
            !reachable_println_calls
                .iter()
                .any(|(_, _, _, instance_fqn)| instance_fqn == "scoop.core.println::<Any>"),
            "request-root 可达扫描不应推导出 println::<Any>：{reachable_println_calls:#?}"
        );
        let mut initial_requests = materializer
            .seed_requests(&inputs.typecheck_types, &inputs.monomorph_requests)
            .unwrap();
        let initial_println_requests = initial_requests
            .iter()
            .filter(|key| key.template.fqn == "scoop.core.println")
            .map(|key| {
                (
                    key.template.source_path.clone(),
                    key.template.decl_span,
                    materializer.instance_fqn(key),
                )
            })
            .collect::<Vec<_>>();
        initial_requests.extend(hir_direct_instance_keys);
        assert!(
            !initial_requests.iter().any(|key| {
                key.template.fqn == "scoop.core.println" && key.type_args == vec![builtins.any]
            }),
            "精确 monomorph key 与 call binding 存在时，seed_requests 不应额外加入 println::<Any>：seed={initial_println_requests:#?}, hir={hir_direct_println_requests:#?}"
        );

        let materialized = materializer.run(initial_requests).unwrap();
        let materialized_printlns = materialized
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) if fun.fqn.starts_with("scoop.core.println::<") => {
                    Some(fun.fqn.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            !materialized_printlns
                .iter()
                .any(|fqn| fqn == "scoop.core.println::<Any>"),
            "精确 call binding 存在时，materialize 后不应额外产出 println::<Any>：{materialized_printlns:#?}"
        );
    }

    #[test]
    fn materialize_for_dump_handles_type_body_generic_member_fun_roots() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_member_root_generic.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

class Box() {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}

fun entry(): Int / Boom {
    return wrap<eff Boom>(Box())
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let forward_instances = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.Box.forward")
            .collect::<Vec<_>>();
        assert_eq!(forward_instances.len(), 1);
        assert!(forward_instances[0].type_args.is_empty());
        assert_eq!(forward_instances[0].eff_args.len(), 1);
        assert!(
            !forward_instances[0].eff_args[0].is_pure(),
            "type-body generic member fun 的实例 key 应保留非 Pure eff_args"
        );
        assert!(
            materialized.file.items.iter().any(|item| matches!(
                item,
                Item::Fun(fun)
                    if fun.fqn.starts_with("fixtures.materialize.Box.forward::<")
                        && fun.fqn.contains("eff fixtures.materialize.Boom")
            )),
            "materialize_for_dump 应产出 Box.forward 的 concrete MIR root"
        );
    }

    #[test]
    fn materialize_for_dump_distinguishes_companion_member_fun_effect_instances() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_companion_member_root_generic.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

effect Zap {
    fun pong(): Unit
}

class Box() {
    companion object {
        fun <eff E = Pure> forward(): Int / E {
            return 1
        }
    }
}

fun <eff E = Pure> wrap(): Int / E {
    return Box.forward<eff E>()
}

fun use_boom(): Int / Boom {
    return wrap<eff Boom>()
}

fun use_zap(): Int / Zap {
    return wrap<eff Zap>()
}
"#,
        );

        let materialized = materialize_for_dump(&sess, &source).unwrap();
        let forward_instances = materialized
            .instance_keys
            .iter()
            .filter(|key| key.template.fqn == "fixtures.materialize.Box.Companion.forward")
            .collect::<Vec<_>>();
        assert_eq!(forward_instances.len(), 2);
        let mut effect_rows = forward_instances
            .iter()
            .map(|key| {
                assert!(key.type_args.is_empty());
                assert_eq!(key.eff_args.len(), 1);
                assert_eq!(key.eff_args[0].terms.len(), 1);
                materialized
                    .types
                    .display(key.eff_args[0].terms[0])
                    .to_string()
            })
            .collect::<Vec<_>>();
        effect_rows.sort();
        assert_eq!(
            effect_rows,
            vec![
                "fixtures.materialize.Boom".to_string(),
                "fixtures.materialize.Zap".to_string()
            ]
        );
        assert!(materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn == "fixtures.materialize.Box.Companion.forward::<eff fixtures.materialize.Boom>"
        )));
        assert!(materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn == "fixtures.materialize.Box.Companion.forward::<eff fixtures.materialize.Zap>"
        )));
    }
}
