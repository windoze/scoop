//! generic MIR template -> monomorphic MIR instance materialization（当前先服务 dump-ir）。
//!
//! 当前阶段的目标边界：
//! - 在 MIR 层定义稳定的 `TemplateKey` / `InstanceKey`；
//! - 用 typecheck 收集到的“实例请求”作为初始种子；
//! - 基于 generic MIR template 做单态实例物化，而不是对每个实例重新回到 HIR lowering；
//! - 先覆盖 dump/调试路径需要的最小闭环：standalone direct-call fixed-point、nested closure family
//!   的 FQN/fn_ptr 重写，以及 per-instance cache。

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
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
use crate::stable_id::{
    AbiMangler, NoTypeParamResolver, StableConeKey, StableDefKey, StableDefNamespace,
    StableInstanceKey, StableTemplateKey, canonical_callable_signature_key,
    stable_template_symbol_suffix,
};
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};
use crate::typecheck;
use crate::typecheck::{
    AnnotationError, ExprTypeError, StructDeclError, TypeEnv, TypeEnvError, TypeHeaderError,
    TypeLowerError,
};

use super::{
    AggregateTransportField, AggregateTransportMetadata, ArrayElementTransportMetadata,
    BasicBlockId, Body, CallArg, CallKind, CallTransportMetadata, ClosureCaptureTransportMetadata,
    ClosureEnvTransportMetadata, ConstValue, DeclMemberMetadata, DeclOnlySummaryInput,
    DeclTypeParamMetadata, ExtensionPropertyMetadata, ExternGlobalRoot, FieldMetadata, File,
    FunDecl, GcIntrinsicTransportMetadata, HandleMetadata, HandlerArm, InitializerRoot,
    InstanceRootSummaryInput, InterpolatedStringPart, Item, LocalDecl, LocalId, LocalSourceKind,
    MaterializedCallableFamilies, MaterializedCallableFamilyInput, MaterializedMirPassArtifacts,
    MaterializedMirSummaries, MemberAccessMetadata, MemberFunMetadata, MemberTarget, MetadataRoot,
    MirPlaceholderCategory, NominalMetadata, ObjectMetadata, Operand, Param, Pattern, PerformArg,
    PerformMetadata, PropertyMetadata, RuntimeCastFailure, RuntimeCastMetadata, RuntimeCastResult,
    RuntimePatternTypeTestMetadata, RuntimeTypeDescriptorKey, RuntimeTypeParameterizedMatch,
    RuntimeTypeTestMetadata, Rvalue, Statement, StatementKind, StructLitField, SupertypeMetadata,
    Terminator, TerminatorKind, TopLevelRef, TypeAliasMetadata, TypeMetadataLiteral, UnwindAction,
    ValueTransportMetadata, build_materialized_summary_table,
};

/// 一个 generic MIR template 的内部实现键。
///
/// 说明：
/// - `fqn` 给出语言级声明身份；
/// - `source_path + decl_span` 只用于当前 materialization 过程内定位 AST/HIR 根；
/// - exported identity 必须改走 `stable_id::StableTemplateKey`，而不是直接复用这里的 path/span。
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

/// 一个 monomorphic MIR instance 的内部实现身份。
///
/// exported identity 必须改走 `stable_id::StableInstanceKey`，而不是把 `TypeId` 直接外露。
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
    pub(super) top_level_value_tys: HashMap<String, TypeId>,
    pub(super) stable_cone_key: StableConeKey,
    pub(super) stable_instance_keys: HashMap<InstanceKey, StableInstanceKey>,
    pub(super) stable_template_keys: HashMap<TemplateKey, StableTemplateKey>,
    pub(super) nongeneric_callable_signature_keys: HashMap<TemplateKey, String>,
    pub(super) opt_level: OptLevel,
    pub(super) callable_families: MaterializedCallableFamilies,
    pub(super) pass_artifacts: MaterializedMirPassArtifacts,
    pub(super) caller_side_pass_candidates: Vec<FunDecl>,
}

impl MaterializedMir {
    /// Stable text surface for `dump-ir` and materialized MIR regression checks.
    pub fn stable_dump(&self) -> String {
        super::stable_dump_materialized(self)
    }

    /// Validate the canonical materialized MIR handoff before it can be consumed by later stages.
    pub fn validate_materialized(&self) -> Result<(), Box<MirMaterializeError>> {
        validation::validate_materialized_mir(self)
    }

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

    /// 返回某个 materialized instance 对应的 authoritative stable instance key。
    pub fn stable_instance_key(&self, instance: &InstanceKey) -> Option<&StableInstanceKey> {
        self.stable_instance_keys.get(instance)
    }

    pub fn stable_instance_keys(&self) -> &HashMap<InstanceKey, StableInstanceKey> {
        &self.stable_instance_keys
    }

    pub(crate) fn top_level_value_tys(&self) -> &HashMap<String, TypeId> {
        &self.top_level_value_tys
    }

    pub(crate) fn stable_cone_key(&self) -> &StableConeKey {
        &self.stable_cone_key
    }

    pub fn authoritative_stable_instance_key(
        &self,
        instance: &InstanceKey,
    ) -> Option<StableInstanceKey> {
        if let Some(stable_key) = self.stable_instance_keys.get(instance) {
            return Some(stable_key.clone());
        }
        let stable_template_key = self
            .stable_template_keys
            .get(&instance.template)
            .cloned()
            .or_else(|| {
                self.nongeneric_callable_signature_keys
                    .get(&instance.template)
                    .map(|signature_key| {
                        StableTemplateKey::new(StableDefKey::new(
                            self.stable_cone_key.clone(),
                            StableDefNamespace::Fun,
                            &instance.template.fqn,
                            "non_generic_callable",
                            Some(signature_key.clone()),
                        ))
                    })
            })?;
        StableInstanceKey::from_type_arguments(
            stable_template_key,
            &self.types,
            &instance.type_args,
            &instance.eff_args,
            &NoTypeParamResolver,
        )
        .ok()
    }

    /// 返回某个 materialized instance 后续应使用的 exported function symbol。
    pub fn instance_exported_fun_symbol(&self, instance: &InstanceKey) -> Option<String> {
        self.authoritative_stable_instance_key(instance)
            .map(|stable_key| AbiMangler.fun_symbol(&stable_key))
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

    #[error(
        "实例请求找不到对应的 generic template：{fqn}@{file}:{span:?}，调用点 {call_file:?}:{call_site:?}"
    )]
    #[diagnostic(code(scoop::mir::materialize::missing_generic_template))]
    MissingGenericTemplate {
        fqn: String,
        file: String,
        span: Span,
        call_file: Option<String>,
        call_site: Option<Span>,
    },

    #[error(
        "generic template 没有匹配的 MIR 根函数：{fqn}@{file}:{span:?}，调用点 {call_file:?}:{call_site:?}"
    )]
    #[diagnostic(code(scoop::mir::materialize::missing_mir_root_for_template))]
    MissingMirRootForTemplate {
        fqn: String,
        file: String,
        span: Span,
        call_file: Option<String>,
        call_site: Option<Span>,
    },

    #[error(
        "实例化的 type args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个，调用点 {call_site:?}"
    )]
    #[diagnostic(code(scoop::mir::materialize::type_arg_arity_mismatch))]
    TypeArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
        call_site: Option<Span>,
        #[label("模板声明在这里")]
        decl_span: miette::SourceSpan,
    },

    #[error(
        "实例化的 effect args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个，调用点 {call_site:?}"
    )]
    #[diagnostic(code(scoop::mir::materialize::effect_arg_arity_mismatch))]
    EffectArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
        call_site: Option<Span>,
        #[label("模板声明在这里")]
        decl_span: miette::SourceSpan,
    },

    #[error(
        "materialized MIR `{fqn}` contains {category} todo `{reason}` in {block:?} at {span:?}"
    )]
    #[diagnostic(code(scoop::mir::materialize::todo_in_materialized_mir))]
    MaterializedTodo {
        fqn: String,
        block: Option<BasicBlockId>,
        span: Span,
        category: MirPlaceholderCategory,
        reason: &'static str,
    },

    #[error("materialized MIR `{fqn}` failed structural validation: {error}")]
    #[diagnostic(code(scoop::mir::materialize::invalid_materialized_mir))]
    MaterializedMirValidation {
        fqn: String,
        #[source]
        error: super::MirValidationError,
    },

    #[error(
        "materialized MIR `{fqn}` contains unresolved generic parameter in {surface} at {span:?}: {ty}"
    )]
    #[diagnostic(code(scoop::mir::materialize::unresolved_generic_param))]
    MaterializedUnresolvedGenericParam {
        fqn: String,
        block: Option<BasicBlockId>,
        span: Span,
        surface: &'static str,
        ty: String,
    },

    #[error(
        "materialized MIR `{fqn}` has unresolved generic direct call target `{callee_fqn}` in {block:?} at {span:?}"
    )]
    #[diagnostic(code(scoop::mir::materialize::missing_materialized_call_target))]
    MaterializedMissingCallTarget {
        fqn: String,
        block: Option<BasicBlockId>,
        span: Span,
        callee_fqn: String,
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

// ----- submodules -----

mod dispatch;
mod entry;
mod generic_mir;
mod hir_calls;
mod inputs;
mod instance;
mod output;
mod reachable;
mod rewrite;
mod run;
mod seed;
mod templates;
mod utils;
mod validation;

// Re-export the public entry functions and any private helper types/free
// functions that sibling submodules need to resolve via `use super::*;`.
pub use entry::*;
use generic_mir::*;
use hir_calls::*;
use inputs::*;
use templates::*;
use utils::*;

#[cfg(test)]
mod tests;
