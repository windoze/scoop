//! typed/lowered HIR → generic early MIR / ANF template lowering。
//!
//! 说明：
//! - 当前入口仍主要服务 `scoop dump-mir` 与 `tests/fixtures/mir/**` 的回归；
//! - lowering 显式消费 `HirFacts` 派生的 source-site contracts，把 dispatch / resume /
//!   perform / pattern 等语言级事实收口到 MIR；
//! - 这里不负责 materialize monomorphic instance，也不编码 LLVM/backend-specific 细节；
//! - 未覆盖的非 typed-contract 表达式/语句继续以 `Todo(...)` 占位；缺失 source-site
//!   contract 表示 HIR barrier 失效，必须在 MIR lowering 前暴露为内部错误。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast;
use crate::expr_facts::HirFactResolver;
use crate::hir;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::stable_id::{
    EffectRowTemplate as StableEffectRowTemplate, EffectTerm as StableEffectTerm,
    NoTypeParamResolver, StableDefKey, StableInstanceKey, StableTemplateKey, canonical_type_text,
};
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
    is_builtin_scalar_nominal_value_type,
};
use scoopc_hir_facts::{HirFacts, source_sites as hir_site_facts};
use scoopc_ids::StableCanonicalKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationResumeReceiverRoute {
    CallArg { index: usize },
    MemberReceiver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelInitRootContract {
    pub(crate) fqn: String,
    pub(crate) source_path: PathBuf,
    pub(crate) span: Span,
    pub(crate) kind: TopLevelInitRootKind,
    pub(crate) ty: Option<TypeId>,
    pub(crate) initializer_ty: Option<TypeId>,
    pub(crate) has_initializer: bool,
    pub(crate) dependencies: Vec<TopLevelInitDependency>,
}

impl TopLevelInitRootContract {
    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn kind(&self) -> TopLevelInitRootKind {
        self.kind
    }

    pub fn ty(&self) -> Option<TypeId> {
        self.ty
    }

    pub fn initializer_ty(&self) -> Option<TypeId> {
        self.initializer_ty
    }

    pub fn has_initializer(&self) -> bool {
        self.has_initializer
    }

    pub fn dependencies(&self) -> &[TopLevelInitDependency] {
        &self.dependencies
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopLevelInitRootKind {
    RuntimeImmutableVal,
    RuntimeMutableVar { storage: hir::TopLevelVarStorage },
    ObjectSingleton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelInitDependency {
    pub(crate) fqn: String,
    pub(crate) kind: TopLevelInitDependencyKind,
}

impl TopLevelInitDependency {
    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn kind(&self) -> TopLevelInitDependencyKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopLevelInitDependencyKind {
    TopLevelValue,
    ObjectSingleton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternGlobalContract {
    pub(crate) fqn: String,
    pub(crate) source_path: PathBuf,
    pub(crate) span: Span,
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
    pub(crate) symbol: String,
    pub(crate) linkage: hir::ExternGlobalLinkage,
    pub(crate) storage: hir::TopLevelVarStorage,
    pub(crate) initializer_absent: bool,
    pub(crate) unsafe_required: bool,
}

impl ExternGlobalContract {
    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn mutable(&self) -> bool {
        self.mutable
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn linkage(&self) -> hir::ExternGlobalLinkage {
        self.linkage
    }

    pub fn storage(&self) -> hir::TopLevelVarStorage {
        self.storage
    }

    pub fn initializer_absent(&self) -> bool {
        self.initializer_absent
    }

    pub fn unsafe_required(&self) -> bool {
        self.unsafe_required
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedIntrinsicKind {
    Reflection {
        name: String,
    },
    Platform {
        name: String,
    },
    Gc {
        name: String,
    },
    Runtime {
        name: String,
    },
    Compiler {
        name: String,
    },
    NamedTable {
        entry_name: String,
        uses_runtime_call: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgBindingContract {
    params: Vec<CallArgParamContract>,
}

impl CallArgBindingContract {
    pub fn new(params: Vec<CallArgParamContract>) -> Self {
        Self { params }
    }

    pub fn params(&self) -> &[CallArgParamContract] {
        &self.params
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArgParamContract {
    Receiver,
    Explicit(CallArgElementContract),
    Default,
    Vararg(Vec<CallArgElementContract>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgElementContract {
    arg_index: usize,
    spread: bool,
}

impl CallArgElementContract {
    pub fn new(arg_index: usize, spread: bool) -> Self {
        Self { arg_index, spread }
    }

    pub fn arg_index(&self) -> usize {
        self.arg_index
    }

    pub fn spread(&self) -> bool {
        self.spread
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTargetContract {
    pub(crate) fqn: String,
    pub(crate) decl_file: Option<PathBuf>,
    pub(crate) decl_span: Option<Span>,
    pub(crate) abi_identity: hir::CallableAbiIdentity,
    pub(crate) param_tys: Vec<TypeId>,
    pub(crate) return_ty: Option<TypeId>,
    pub(crate) stable_template_key: Option<StableTemplateKey>,
    pub(crate) stable_instance_key: Option<StableInstanceKey>,
    pub(crate) intrinsic_entry_name: Option<String>,
    pub(crate) type_args: Vec<TypeId>,
    pub(crate) eff_args: Vec<EffectRow>,
    pub(crate) arg_binding: Option<CallArgBindingContract>,
}

impl FunctionTargetContract {
    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn decl_span(&self) -> Option<Span> {
        self.decl_span
    }

    pub fn type_args(&self) -> &[TypeId] {
        &self.type_args
    }

    pub fn eff_args(&self) -> &[EffectRow] {
        &self.eff_args
    }

    pub fn param_tys(&self) -> &[TypeId] {
        &self.param_tys
    }

    pub fn return_ty(&self) -> Option<TypeId> {
        self.return_ty
    }

    pub fn stable_template_key(&self) -> Option<&StableTemplateKey> {
        self.stable_template_key.as_ref()
    }

    pub fn stable_instance_key(&self) -> Option<&StableInstanceKey> {
        self.stable_instance_key.as_ref()
    }

    pub fn intrinsic_entry_name(&self) -> Option<&str> {
        self.intrinsic_entry_name.as_deref()
    }

    pub fn arg_binding(&self) -> Option<&CallArgBindingContract> {
        self.arg_binding.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCallTargetContract {
    pub(crate) owner_fqn: String,
    pub(crate) member_name: String,
    pub(crate) member_fqn: String,
    pub(crate) receiver_ty: TypeId,
    pub(crate) function: FunctionTargetContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSiteBindingContract {
    pub(crate) stable_template_key: StableTemplateKey,
    pub(crate) type_args: Vec<TypeId>,
    pub(crate) eff_args: Vec<EffectRow>,
}

impl TemplateSiteBindingContract {
    pub fn stable_template_key(&self) -> &StableTemplateKey {
        &self.stable_template_key
    }

    pub fn type_args(&self) -> &[TypeId] {
        &self.type_args
    }

    pub fn eff_args(&self) -> &[EffectRow] {
        &self.eff_args
    }
}

impl MemberCallTargetContract {
    pub fn owner_fqn(&self) -> &str {
        &self.owner_fqn
    }

    pub fn member_name(&self) -> &str {
        &self.member_name
    }

    pub fn member_fqn(&self) -> &str {
        &self.member_fqn
    }

    pub fn receiver_ty(&self) -> TypeId {
        self.receiver_ty
    }

    pub fn function(&self) -> &FunctionTargetContract {
        &self.function
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorCallTargetContract {
    pub(crate) owner_fqn: String,
    pub(crate) ctor_span: Option<Span>,
    pub(crate) result_ty: TypeId,
    pub(crate) arg_mapping: Vec<Option<usize>>,
}

impl ConstructorCallTargetContract {
    pub fn owner_fqn(&self) -> &str {
        &self.owner_fqn
    }

    pub fn ctor_span(&self) -> Option<Span> {
        self.ctor_span
    }

    pub fn arg_mapping(&self) -> &[Option<usize>] {
        &self.arg_mapping
    }

    pub fn result_ty(&self) -> TypeId {
        self.result_ty
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedCallSiteContract {
    DirectTopLevel(FunctionTargetContract),
    MemberDirect(MemberCallTargetContract),
    Extension {
        receiver_ty: TypeId,
        function: FunctionTargetContract,
    },
    Constructor(ConstructorCallTargetContract),
    Closure {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi_identity: hir::CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunValue {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi_identity: hir::CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunPtr {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi_identity: hir::CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    },
    Virtual(MemberCallTargetContract),
    Interface(MemberCallTargetContract),
    Intrinsic {
        kind: TypedIntrinsicKind,
        function: FunctionTargetContract,
    },
    EffectOp(()),
    ContinuationResume(()),
}

use super::{
    AccessorMetadata, AggregateTransportField, AggregateTransportKind, AggregateTransportMetadata,
    ArrayElementTransportMetadata, ArrayTransportOperation, BasicBlock, BasicBlockId, Body,
    CallAbiHandoffMetadata, CallArg, CallKind, CallTransportMetadata, ClassCtorCallMetadata,
    ClosureCaptureTransportMetadata, ClosureEnvTransportMetadata, ConstValue, CtorMetadata,
    CtorParamMetadata, DeclMemberMetadata, DeclTypeParamMetadata, DispatchMetadata,
    EnumVariantMetadata, ExtensionPropertyMetadata, ExternGlobalRoot, FieldMetadata, File, FunDecl,
    GcIntrinsicOperation, GcIntrinsicPairing, GcIntrinsicTransportMetadata, GcRootLifetime,
    HandleMetadata, HandlerArm, HandlerArmKind, InitializerDependency, InitializerDependencyKind,
    InitializerRoot, InitializerRootKind, Item, LocalDecl, LocalId, LocalSourceKind,
    MemberAccessMetadata, MemberFunMetadata, MemberTarget, MetadataRoot, MirBoxingIntent,
    MirBoxingReason, MirTransportKind, MirTransportRequirements, MirValidationError,
    NominalMetadata, ObjectMetadata, Operand, Param, Pattern, PatternBindingStep, PerformArg,
    PerformMetadata, PropertyMetadata, ResumeMetadata, RuntimeCastFailure, RuntimeCastMetadata,
    RuntimeCastResult, RuntimePatternTypeTestKind, RuntimePatternTypeTestMetadata,
    RuntimeTypeDescriptorKey, RuntimeTypeDescriptorKind, RuntimeTypeParameterizedMatch,
    RuntimeTypeStaticFold, RuntimeTypeTestMetadata, Rvalue, SiteId, Statement, StatementKind,
    StoredContinuationRoutePublication, StoredContinuationValueRoute, SupertypeMetadata,
    Terminator, TerminatorKind, TopLevelRef, TypeAliasMetadata, TypeMetadataLiteral,
    TypeMetadataLiteralKind, UnwindAction, ValueTransportMetadata,
};

/// MIR lowering 需要消费的最小共享事实。
///
/// 目标：
/// - 把 HIR/typecheck 已确认的调用语义收口成 MIR lowering 可直接查询的 backend-agnostic 输入；
/// - 避免 MIR 阶段重新回到 LLVM vtable/itable 细节或 `Continuation.resume` 名字推断。
#[derive(Debug, Clone, Default)]
pub struct MirLoweringFacts {
    dispatch_call_sites: HashMap<hir::DispatchCallSite, DispatchTargetKind>,
    call_arg_bindings: HashMap<hir::CallSite, CallArgBindingContract>,
    resume_sites: HashMap<hir::CallSite, ResumeCallInfo>,
    perform_sites: HashMap<hir::CallSite, PerformMetadata>,
    handle_sites: HashMap<hir::CallSite, HandleSiteInfo>,
    call_sites: HashMap<hir::CallSite, TypedCallSiteContract>,
    template_value_bindings: HashMap<hir::CallSite, TemplateSiteBindingContract>,
    dispatch_candidate_keys: HashMap<hir::CallSite, Vec<StableInstanceKey>>,
    assign_places: HashMap<hir::CallSite, hir::AssignPlaceContract>,
    class_ctor_call_sites: HashMap<hir::CallSite, hir::CtorCallInfo>,
    class_ctor_hidden_effects: HashMap<hir::CallSite, EffectRow>,
    object_member_hidden_effects: HashMap<String, EffectRow>,
    top_level_ref_hidden_effects: HashMap<String, EffectRow>,
    top_level_init_roots: Vec<TopLevelInitRootContract>,
    extern_global_contracts: Vec<ExternGlobalContract>,
    when_pat_binding_tys: HashMap<Span, TypeId>,
    nominal_kinds: HashMap<String, ast::TypeKind>,
    enum_has_payload: HashMap<String, bool>,
    enum_variant_owner_fqns: HashMap<String, String>,
    top_level_fun_call_fqns: HashMap<hir::CallSite, String>,
    member_value_tys: HashMap<String, TypeId>,
    continuation_identity_return_funs: HashMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchTargetKind {
    Virtual,
    Interface,
}

#[derive(Debug, Clone)]
struct HandleSiteInfo {
    metadata: HandleMetadata,
    arms: Vec<HandlerArm>,
}

#[derive(Debug, Clone)]
struct ResumeCallInfo {
    receiver_route: ContinuationResumeReceiverRoute,
    payload_arg_indices: Vec<usize>,
    metadata: ResumeMetadata,
}

fn call_arg_expr(arg: &hir::CallArg) -> &hir::Expr {
    match arg {
        hir::CallArg::Positional(expr) => expr,
        hir::CallArg::Named { value, .. } => value,
    }
}

fn call_arg_binding_has_receiver(binding: &CallArgBindingContract) -> bool {
    binding
        .params()
        .iter()
        .any(|param| matches!(param, CallArgParamContract::Receiver))
}

fn call_arg_binding_without_receiver(
    binding: Option<&CallArgBindingContract>,
) -> Option<CallArgBindingContract> {
    let binding = binding?;
    if !call_arg_binding_has_receiver(binding) {
        return Some(binding.clone());
    }
    Some(CallArgBindingContract::new(
        binding
            .params()
            .iter()
            .filter(|param| !matches!(param, CallArgParamContract::Receiver))
            .cloned()
            .collect(),
    ))
}

mod entry;
mod fn_lowering_basic;
mod fn_lowering_call;
mod fn_lowering_effect;
mod fn_lowering_expr;
mod hidden_init;
mod mir_lowering_facts;
mod post_helpers;
#[cfg(test)]
mod tests;
mod transport;

#[allow(unused_imports)]
pub use {
    entry::*, fn_lowering_basic::*, fn_lowering_call::*, fn_lowering_effect::*,
    fn_lowering_expr::*, hidden_init::*, mir_lowering_facts::*, post_helpers::*, transport::*,
};
