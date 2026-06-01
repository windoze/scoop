//! generic MIR template -> monomorphic MIR instance materialization（当前先服务 dump-ir）。
//!
//! 当前阶段的目标边界：
//! - 使用 base identity 层定义的 `TemplateKey` / `InstanceKey`；
//! - 用 typecheck 收集到的“实例请求”作为初始种子；
//! - 基于 generic MIR template 做单态实例物化，而不是对每个实例重新回到 HIR lowering；
//! - 先覆盖 dump/调试路径需要的最小闭环：standalone direct-call fixed-point、nested closure family
//!   的 FQN/fn_ptr 重写，以及 per-instance cache。

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use scoopc_ids::{InstanceKey, StableCanonicalKey, TemplateKey};
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
    AbiMangler, CanonicalTextKey, EffectRowTemplate, NoTypeParamResolver, StableConeKey,
    StableDefKey, StableDefNamespace, StableInstanceKey, StableTemplateKey,
    canonical_callable_signature_key, canonical_type_text, stable_template_symbol_suffix,
};
use crate::ty::{
    BuiltinTypes, EFFECT_ROW_PARAM_DECL_FILE, EffectRow, NominalType, RefTypeKind, TypeId,
    TypeKind, TypeStore, ValueTypeKind,
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
    DeclTypeParamMetadata, DispatchDevirtualizationFacts, DispatchDevirtualizationTargetKey,
    DispatchMetadata, ExtensionPropertyMetadata, ExternGlobalRoot, FieldMetadata, File, FunDecl,
    GcIntrinsicTransportMetadata, HandleMetadata, HandlerArm, InitializerRoot,
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

/// `dump-ir` / tests 使用的 monomorphic MIR 输出。
#[derive(Debug)]

pub struct MaterializedMir {
    pub file: File,
    pub types: TypeStore,
    pub instance_keys: Vec<InstanceKey>,
    pub summaries: MaterializedMirSummaries,
    pub(super) backend_contracts: MaterializedBackendContracts,
    pub(super) top_level_value_tys: HashMap<String, TypeId>,
    pub(super) stable_cone_key: StableConeKey,
    pub(super) stable_instance_keys: HashMap<InstanceKey, StableInstanceKey>,
    pub(super) stable_template_keys: HashMap<TemplateKey, StableTemplateKey>,
    pub(super) nongeneric_callable_stable_template_keys: HashMap<TemplateKey, StableTemplateKey>,
    pub(super) opt_level: OptLevel,
    pub(super) callable_families: MaterializedCallableFamilies,
    pub(super) pass_artifacts: MaterializedMirPassArtifacts,
    pub(super) dispatch_devirtualization_facts: super::DispatchDevirtualizationFacts,
    pub(super) caller_side_pass_candidates: Vec<FunDecl>,
    pub(super) source_callable_signatures: Vec<MaterializedCallableSignature>,
    pub(super) source_callable_effects: Vec<MaterializedCallableEffectTemplate>,
}

#[derive(Debug, Clone)]
pub struct MaterializedCallableSignature {
    pub fqn: String,
    pub param_names: Vec<String>,
    pub param_tys: Vec<TypeId>,
    pub return_ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct MaterializedCallableEffectTemplate {
    pub fqn: String,
    pub declared_surface_row: Option<EffectRowTemplate>,
    pub actual_surface_row_template: EffectRowTemplate,
    pub published_surface_row_template: EffectRowTemplate,
}

/// Data-only backend contracts captured at materialization time for LIR facts.
#[derive(Debug, Clone, Default)]
pub struct MaterializedBackendContracts {
    pub enum_layouts: crate::hir::EnumLayoutIndex,
    pub class_inits: crate::hir::ClassInitIndex,
    pub class_vtables: crate::vtable::ClassVtableIndex,
    pub interfaces: crate::itable::InterfaceIndex,
    pub class_itables: crate::itable::ClassItableIndex,
    pub extern_funs: crate::hir::ExternFunIndex,
    pub native_callable_funs: crate::hir::NativeCallableFunIndex,
    pub top_level_vars: crate::hir::TopLevelVarIndex,
    pub top_level_immutable_values: crate::hir::TopLevelImmutableValueIndex,
    pub object_inits: crate::hir::ObjectInitIndex,
}

impl MaterializedBackendContracts {
    /// Return class init payloads through the MIR-owned surface expected by LIR.
    pub fn class_init_payloads(&self) -> impl Iterator<Item = super::MonoClassInit> + '_ {
        self.class_inits
            .values()
            .map(super::MonoClassInit::from_hir)
    }
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

    pub fn top_level_value_tys(&self) -> &HashMap<String, TypeId> {
        &self.top_level_value_tys
    }

    pub fn dispatch_devirtualization_facts(&self) -> &super::DispatchDevirtualizationFacts {
        &self.dispatch_devirtualization_facts
    }

    pub fn backend_contracts(&self) -> &MaterializedBackendContracts {
        &self.backend_contracts
    }

    pub fn stable_cone_key(&self) -> &StableConeKey {
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
                self.nongeneric_callable_stable_template_keys
                    .get(&instance.template)
                    .cloned()
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
    pub fn caller_side_pass_candidate_bodies(&self) -> &[FunDecl] {
        &self.caller_side_pass_candidates
    }

    pub fn source_callable_signatures(&self) -> &[MaterializedCallableSignature] {
        &self.source_callable_signatures
    }

    pub fn source_callable_effects(&self) -> &[MaterializedCallableEffectTemplate] {
        &self.source_callable_effects
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
        reason: String,
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
#[cfg(test)]
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
#[cfg(test)]
use hir_calls::*;
use inputs::*;
use seed::*;
use templates::*;
use utils::*;

#[cfg(test)]
mod tests;
