use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::ast;
use crate::hir::{
    AssignPlaceContract, AssignPlaceKind, Block, CallArg, CallSite, CallableAbiIdentity, ClassCtor,
    ClassInitStep, Decl, DispatchCallKind, Expr, ExprKind, FunDecl, HandleArmKind, HirLowerError,
    HirStageError, InterpolatedStringPart, Item, LoweredHir, MemberRef, MonoClassInit, ObjectInit,
    ObjectInitStep, Stmt, StmtKind, ValDecl, ValueRef,
};
use crate::intrinsics::{NamedIntrinsicLoweringMode, named_intrinsic_audit_entry};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::stable_id::{
    CanonicalEncodingError, CanonicalTextKey, EffectRowTemplate as StableEffectRowTemplate,
    EffectTerm as StableEffectTerm, NoTypeParamResolver, SiteId, StableCanonicalKey, StableDefKey,
    StableEffectParamKey, StableHashScope, StableInstanceKey, StableTemplateKey, stable_hash64,
};
use crate::ty::{
    EFFECT_ROW_PARAM_DECL_FILE, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind,
    TypeParamType, TypeStore, ValueTypeKind,
};
use scoopc_hir_facts::{
    HirFacts,
    common::FactIdentity,
    declarations::{
        CallableBodyFact, CallableDeclarationFact, DeclarationFacts, DispatchSlotFact,
        DispatchTableFact, EnumVariantDeclarationFact, FieldDeclarationFact, FieldOwnerKind,
        GenericTemplateFact, NominalDeclarationFact, NominalKind as HirFactNominalKind,
        TypeParameterFact, Variance as HirFactVariance,
    },
    globals::{
        GlobalRootFact, GlobalRootKind, GlobalStoragePolicy, InitializerFact, InitializerFieldFact,
    },
    native::{ExternFunctionFact, ExternGlobalFact, ExternLibraryFact, NativeCallableFact},
    source_sites as hir_site_facts,
    type_context::{SourceConeFact, StableTypeParamFact, TypeContextReference},
};

use super::hir_completeness::HirCompletenessVerifier;

/// MIR lowering should not rediscover the continuation receiver from callee syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationResumeReceiverRoute {
    /// The receiver is carried as a canonical call argument at this index.
    CallArg { index: usize },
    /// The receiver is still the member-call receiver expression.
    MemberReceiver,
}

/// 单个 `Continuation.resume(...)` 调用点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationResumeSiteContract {
    receiver_route: ContinuationResumeReceiverRoute,
    payload_arg_indices: Vec<usize>,
    receiver_ty: TypeId,
    resume_ty: TypeId,
    answer_ty: TypeId,
    return_ty: TypeId,
    out_effects: EffectRow,
    runtime_error_effect_ty: Option<TypeId>,
}

impl ContinuationResumeSiteContract {
    pub fn receiver_route(&self) -> ContinuationResumeReceiverRoute {
        self.receiver_route
    }

    pub fn payload_arg_indices(&self) -> &[usize] {
        &self.payload_arg_indices
    }

    pub fn receiver_ty(&self) -> TypeId {
        self.receiver_ty
    }

    pub fn resume_ty(&self) -> TypeId {
        self.resume_ty
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn return_ty(&self) -> TypeId {
        self.return_ty
    }

    pub fn out_effects(&self) -> &EffectRow {
        &self.out_effects
    }

    pub fn runtime_error_effect_ty(&self) -> Option<TypeId> {
        self.runtime_error_effect_ty
    }

    pub fn required_effects_include_runtime_error(&self) -> bool {
        self.runtime_error_effect_ty.is_some()
    }
}

/// 单个函数在 typed HIR stage 中对外暴露的 allowed-row / required-effects contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionEffectContract {
    source_path: PathBuf,
    span: Span,
    fqn: String,
    return_ty: TypeId,
    allowed_effects: EffectRow,
    effects_closed: bool,
}

impl FunctionEffectContract {
    fn new(
        source_path: PathBuf,
        span: Span,
        fqn: String,
        return_ty: TypeId,
        allowed_effects: EffectRow,
        effects_closed: bool,
    ) -> Self {
        Self {
            source_path,
            span,
            fqn,
            return_ty,
            allowed_effects,
            effects_closed,
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn return_ty(&self) -> TypeId {
        self.return_ty
    }

    pub fn allowed_effects(&self) -> &EffectRow {
        &self.allowed_effects
    }

    pub fn effects_closed(&self) -> bool {
        self.effects_closed
    }
}

/// Typed HIR handoff root for top-level initialization/storage ordering.
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

    #[allow(dead_code)]
    #[allow(dead_code)]
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
    RuntimeMutableVar {
        storage: crate::hir::TopLevelVarStorage,
    },
    ObjectSingleton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelInitDependency {
    pub(crate) fqn: String,
    pub(crate) kind: TopLevelInitDependencyKind,
}

impl TopLevelInitDependency {
    fn new(fqn: String, kind: TopLevelInitDependencyKind) -> Self {
        Self { fqn, kind }
    }

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

/// Typed HIR handoff contract for an `@Extern` top-level variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternGlobalContract {
    pub(crate) fqn: String,
    pub(crate) source_path: PathBuf,
    pub(crate) span: Span,
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
    pub(crate) symbol: String,
    pub(crate) linkage: crate::hir::ExternGlobalLinkage,
    pub(crate) storage: crate::hir::TopLevelVarStorage,
    pub(crate) initializer_absent: bool,
    pub(crate) unsafe_required: bool,
}

impl ExternGlobalContract {
    fn from_hir(global: &crate::hir::ExternGlobal) -> Self {
        Self {
            fqn: global.fqn.clone(),
            source_path: global.source_path.clone(),
            span: global.span,
            ty: global.ty,
            mutable: global.mutable,
            symbol: global.symbol.clone(),
            linkage: global.linkage,
            storage: global.storage,
            initializer_absent: global.initializer_absent,
            unsafe_required: global.unsafe_required,
        }
    }

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

    pub fn linkage(&self) -> crate::hir::ExternGlobalLinkage {
        self.linkage
    }

    pub fn storage(&self) -> crate::hir::TopLevelVarStorage {
        self.storage
    }

    pub fn initializer_absent(&self) -> bool {
        self.initializer_absent
    }

    pub fn unsafe_required(&self) -> bool {
        self.unsafe_required
    }
}

/// `perform` / `handle` payload 的结构化 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadTypeContract {
    ty: Option<TypeId>,
    components: Vec<TypeId>,
}

impl PayloadTypeContract {
    fn new(ty: Option<TypeId>, components: Vec<TypeId>) -> Self {
        Self { ty, components }
    }

    #[allow(dead_code)]
    pub fn ty(&self) -> Option<TypeId> {
        self.ty
    }

    pub fn components(&self) -> &[TypeId] {
        &self.components
    }

    fn display(&self, types: &TypeStore) -> String {
        if let Some(ty) = self.ty {
            return types.display(ty).to_string();
        }

        if self.components.is_empty() {
            return "<missing>".to_string();
        }

        let mut rendered = String::from("(");
        for (index, ty) in self.components.iter().enumerate() {
            if index > 0 {
                rendered.push_str(", ");
            }
            rendered.push_str(&types.display(*ty).to_string());
        }
        rendered.push(')');
        rendered
    }
}

/// 单个 `perform` 站点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformSiteContract {
    effect_ty: TypeId,
    op_fqn: String,
    result_ty: TypeId,
    payload: PayloadTypeContract,
    arg_mapping: Vec<usize>,
}

impl PerformSiteContract {
    fn new(
        effect_ty: TypeId,
        op_fqn: String,
        result_ty: TypeId,
        payload: PayloadTypeContract,
        arg_mapping: Vec<usize>,
    ) -> Self {
        Self {
            effect_ty,
            op_fqn,
            result_ty,
            payload,
            arg_mapping,
        }
    }

    pub fn effect_ty(&self) -> TypeId {
        self.effect_ty
    }

    pub fn op_fqn(&self) -> &str {
        &self.op_fqn
    }

    pub fn result_ty(&self) -> TypeId {
        self.result_ty
    }

    pub fn payload(&self) -> &PayloadTypeContract {
        &self.payload
    }

    pub fn arg_mapping(&self) -> &[usize] {
        &self.arg_mapping
    }
}

/// `handle` arm 的语义 kind 在 typed HIR contract 中的稳定枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleArmContractKind {
    NonResuming,
    EscapeContinuation,
}

/// 单个 `handle` arm 的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleArmSiteContract {
    handled_effect_ty: TypeId,
    op_fqn: String,
    payload: PayloadTypeContract,
    body_ty: TypeId,
    kind: HandleArmContractKind,
}

impl HandleArmSiteContract {
    fn new(
        handled_effect_ty: TypeId,
        op_fqn: String,
        payload: PayloadTypeContract,
        body_ty: TypeId,
        kind: HandleArmContractKind,
    ) -> Self {
        Self {
            handled_effect_ty,
            op_fqn,
            payload,
            body_ty,
            kind,
        }
    }

    pub fn handled_effect_ty(&self) -> TypeId {
        self.handled_effect_ty
    }

    pub fn op_fqn(&self) -> &str {
        &self.op_fqn
    }

    pub fn payload(&self) -> &PayloadTypeContract {
        &self.payload
    }

    pub fn body_ty(&self) -> TypeId {
        self.body_ty
    }

    pub fn kind(&self) -> HandleArmContractKind {
        self.kind
    }
}

/// 单个 `handle { ... } on { ... }` 站点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleSiteContract {
    result_ty: TypeId,
    body_result_ty: TypeId,
    arm_contracts: Vec<HandleArmSiteContract>,
    finally_result_ty: Option<TypeId>,
}

impl HandleSiteContract {
    fn new(
        result_ty: TypeId,
        body_result_ty: TypeId,
        arm_contracts: Vec<HandleArmSiteContract>,
        finally_result_ty: Option<TypeId>,
    ) -> Self {
        Self {
            result_ty,
            body_result_ty,
            arm_contracts,
            finally_result_ty,
        }
    }

    pub fn result_ty(&self) -> TypeId {
        self.result_ty
    }

    pub fn body_result_ty(&self) -> TypeId {
        self.body_result_ty
    }

    pub fn arm_contracts(&self) -> &[HandleArmSiteContract] {
        &self.arm_contracts
    }

    pub fn finally_result_ty(&self) -> Option<TypeId> {
        self.finally_result_ty
    }
}

/// P2 typed HIR 已显式区分出的调用点 kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedCallSiteKind {
    DirectTopLevel,
    MemberDirect,
    Extension,
    Constructor,
    Closure,
    FunValue,
    FunPtr,
    Virtual,
    Interface,
    Intrinsic,
    EffectOp,
    ContinuationResume,
}

/// 编译器/运行时 intrinsic 在 typed HIR call contract 中的稳定分类。
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicAllowedContext {
    RuntimeOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicRuntimeFallback {
    NormalRuntimeCall,
    PlatformQuery,
    RuntimeIntrinsic,
    CompilerLowered,
}

impl TypedIntrinsicKind {
    fn from_call_binding(binding: &ast::TopLevelFunCallBinding) -> Self {
        let Some(entry_name) = binding.intrinsic_entry_name.as_deref() else {
            return Self::from_fqn(&binding.fqn);
        };
        let entry = named_intrinsic_audit_entry(entry_name)
            .expect("typecheck should only publish named intrinsic entries from the shared table");
        Self::NamedTable {
            entry_name: entry_name.to_string(),
            uses_runtime_call: matches!(
                entry.lowering_mode,
                NamedIntrinsicLoweringMode::RuntimeCall
            ),
        }
    }

    fn from_fqn(fqn: &str) -> Self {
        let name = fqn.rsplit('.').next().unwrap_or(fqn).to_string();
        match fqn {
            "scoop.core.nameOf"
            | "scoop.core.sizeOf"
            | "scoop.core.alignOf"
            | "scoop.core.kindOf"
            | "scoop.core.descOf"
            | "scoop.core.fieldsOf"
            | "scoop.core.variantsOf"
            | "scoop.core.superTypesOf"
            | "scoop.core.paramsOf" => Self::Reflection { name },
            "scoop.core.getPlatform" => Self::Platform { name },
            "scoop.core.GC.pin"
            | "scoop.core.GC.unpin"
            | "scoop.core.GC.handleNew"
            | "scoop.core.GC.handleGet"
            | "scoop.core.GC.handleDrop" => Self::Gc { name },
            _ if fqn.starts_with("scoop.core.__") => Self::Runtime { name },
            _ => Self::Compiler { name },
        }
    }

    pub fn allowed_context(&self) -> IntrinsicAllowedContext {
        IntrinsicAllowedContext::RuntimeOnly
    }

    pub fn runtime_fallback(&self) -> IntrinsicRuntimeFallback {
        match self {
            Self::Reflection { .. } => IntrinsicRuntimeFallback::NormalRuntimeCall,
            Self::Platform { .. } => IntrinsicRuntimeFallback::PlatformQuery,
            Self::Gc { .. } | Self::Runtime { .. } => IntrinsicRuntimeFallback::RuntimeIntrinsic,
            Self::Compiler { .. } => IntrinsicRuntimeFallback::CompilerLowered,
            Self::NamedTable {
                uses_runtime_call, ..
            } => {
                if *uses_runtime_call {
                    IntrinsicRuntimeFallback::RuntimeIntrinsic
                } else {
                    IntrinsicRuntimeFallback::CompilerLowered
                }
            }
        }
    }
}

/// 一个实参 slot 与源码实参/默认值/receiver 的归一化绑定关系。
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

/// 一个 resolved function target 的声明身份与实例化参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTargetContract {
    pub(crate) fqn: String,
    pub(crate) decl_file: Option<PathBuf>,
    pub(crate) decl_span: Option<Span>,
    pub(crate) abi_identity: CallableAbiIdentity,
    pub(crate) param_tys: Vec<TypeId>,
    pub(crate) return_ty: Option<TypeId>,
    pub(crate) stable_template_key: Option<StableTemplateKey>,
    pub(crate) stable_instance_key: Option<StableInstanceKey>,
    pub(crate) type_args: Vec<TypeId>,
    pub(crate) eff_args: Vec<EffectRow>,
    pub(crate) arg_binding: Option<CallArgBindingContract>,
}

struct SyntheticStableFunctionTarget {
    fqn: String,
    decl_file: PathBuf,
    decl_span: Span,
    abi_identity: CallableAbiIdentity,
    stable_template_key: StableTemplateKey,
    type_args: Vec<TypeId>,
    eff_args: Vec<EffectRow>,
    param_tys: Vec<TypeId>,
    return_ty: Option<TypeId>,
    arg_binding: Option<CallArgBindingContract>,
}

impl FunctionTargetContract {
    fn from_binding(
        types: &TypeStore,
        binding: &ast::TopLevelFunCallBinding,
        abi_identity: CallableAbiIdentity,
        stable_template_key: Option<StableTemplateKey>,
        arg_binding: Option<CallArgBindingContract>,
    ) -> Self {
        let type_args = binding
            .type_args
            .iter()
            .copied()
            .filter(|ty| type_id_in_store(types, *ty))
            .collect::<Vec<_>>();
        let eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                EffectRow::new(
                    row.terms
                        .iter()
                        .copied()
                        .filter(|ty| type_id_in_store(types, *ty))
                        .collect(),
                )
            })
            .collect::<Vec<_>>();
        let stable_instance_key = stable_template_key.as_ref().and_then(|template| {
            instance_args_are_concrete(types, &type_args, &eff_args)
                .then(|| {
                    StableInstanceKey::from_type_arguments(
                        template.clone(),
                        types,
                        &type_args,
                        &eff_args,
                        &NoTypeParamResolver,
                    )
                    .ok()
                })
                .flatten()
        });
        Self {
            fqn: binding.fqn.clone(),
            decl_file: Some(binding.decl_file.clone()),
            decl_span: Some(binding.decl_span),
            abi_identity,
            param_tys: binding
                .param_tys
                .iter()
                .copied()
                .filter(|ty| type_id_in_store(types, *ty))
                .collect(),
            return_ty: binding.return_ty.filter(|ty| type_id_in_store(types, *ty)),
            stable_template_key,
            stable_instance_key,
            type_args,
            eff_args,
            arg_binding,
        }
    }

    fn synthetic_with_arg_binding(
        fqn: String,
        abi_identity: CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    ) -> Self {
        Self {
            fqn,
            decl_file: None,
            decl_span: None,
            abi_identity,
            param_tys: Vec::new(),
            return_ty: None,
            stable_template_key: None,
            stable_instance_key: None,
            type_args: Vec::new(),
            eff_args: Vec::new(),
            arg_binding,
        }
    }

    fn synthetic_with_stable_args(
        types: &TypeStore,
        target: SyntheticStableFunctionTarget,
    ) -> Self {
        let SyntheticStableFunctionTarget {
            fqn,
            decl_file,
            decl_span,
            abi_identity,
            stable_template_key,
            type_args,
            eff_args,
            param_tys,
            return_ty,
            arg_binding,
        } = target;
        let stable_instance_key = instance_args_are_concrete(types, &type_args, &eff_args)
            .then(|| {
                StableInstanceKey::from_type_arguments(
                    stable_template_key.clone(),
                    types,
                    &type_args,
                    &eff_args,
                    &NoTypeParamResolver,
                )
                .ok()
            })
            .flatten();

        Self {
            fqn,
            decl_file: Some(decl_file),
            decl_span: Some(decl_span),
            abi_identity,
            param_tys,
            return_ty,
            stable_template_key: Some(stable_template_key),
            stable_instance_key,
            type_args,
            eff_args,
            arg_binding,
        }
    }

    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    #[allow(dead_code)]
    pub fn decl_file(&self) -> Option<&Path> {
        self.decl_file.as_deref()
    }

    pub fn decl_span(&self) -> Option<Span> {
        self.decl_span
    }

    #[allow(dead_code)]
    pub fn abi_identity(&self) -> CallableAbiIdentity {
        self.abi_identity
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

    pub fn type_args(&self) -> &[TypeId] {
        &self.type_args
    }

    pub fn eff_args(&self) -> &[EffectRow] {
        &self.eff_args
    }

    pub fn arg_binding(&self) -> Option<&CallArgBindingContract> {
        self.arg_binding.as_ref()
    }
}

/// 成员调用的结构化 owner/member 绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCallTargetContract {
    pub(crate) owner_fqn: String,
    pub(crate) member_name: String,
    pub(crate) member_fqn: String,
    pub(crate) receiver_ty: TypeId,
    pub(crate) function: FunctionTargetContract,
}

impl MemberCallTargetContract {
    fn new(
        owner_fqn: String,
        member_name: String,
        member_fqn: String,
        receiver_ty: TypeId,
        function: FunctionTargetContract,
    ) -> Self {
        Self {
            owner_fqn,
            member_name,
            member_fqn,
            receiver_ty,
            function,
        }
    }

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

/// constructor 调用的 typed HIR provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorCallTargetContract {
    pub(crate) owner_fqn: String,
    pub(crate) ctor_span: Option<Span>,
    pub(crate) result_ty: TypeId,
    pub(crate) arg_mapping: Vec<Option<usize>>,
}

impl ConstructorCallTargetContract {
    fn new(
        owner_fqn: String,
        ctor_span: Option<Span>,
        result_ty: TypeId,
        arg_mapping: Vec<Option<usize>>,
    ) -> Self {
        Self {
            owner_fqn,
            ctor_span,
            result_ty,
            arg_mapping,
        }
    }

    pub fn owner_fqn(&self) -> &str {
        &self.owner_fqn
    }

    pub fn ctor_span(&self) -> Option<Span> {
        self.ctor_span
    }

    pub fn result_ty(&self) -> TypeId {
        self.result_ty
    }

    pub fn arg_mapping(&self) -> &[Option<usize>] {
        &self.arg_mapping
    }
}

/// 每个 call-like HIR site 对下游暴露的结构化 provenance。
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
        abi_identity: CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunValue {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi_identity: CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunPtr {
        callee_ty: TypeId,
        return_ty: TypeId,
        abi_identity: CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    },
    Virtual(MemberCallTargetContract),
    Interface(MemberCallTargetContract),
    Intrinsic {
        kind: TypedIntrinsicKind,
        function: FunctionTargetContract,
    },
    EffectOp(PerformSiteContract),
    ContinuationResume(ContinuationResumeSiteContract),
}

impl TypedCallSiteContract {
    pub fn kind(&self) -> TypedCallSiteKind {
        match self {
            Self::DirectTopLevel(_) => TypedCallSiteKind::DirectTopLevel,
            Self::MemberDirect(_) => TypedCallSiteKind::MemberDirect,
            Self::Extension { .. } => TypedCallSiteKind::Extension,
            Self::Constructor(_) => TypedCallSiteKind::Constructor,
            Self::Closure { .. } => TypedCallSiteKind::Closure,
            Self::FunValue { .. } => TypedCallSiteKind::FunValue,
            Self::FunPtr { .. } => TypedCallSiteKind::FunPtr,
            Self::Virtual(_) => TypedCallSiteKind::Virtual,
            Self::Interface(_) => TypedCallSiteKind::Interface,
            Self::Intrinsic { .. } => TypedCallSiteKind::Intrinsic,
            Self::EffectOp(_) => TypedCallSiteKind::EffectOp,
            Self::ContinuationResume(_) => TypedCallSiteKind::ContinuationResume,
        }
    }
}

/// HIR lowering 内部收集器产物。
///
/// 这些 contract 会立即转换成 `scoopc_hir_facts::HirFacts`；后续 stage 不再直接消费本类型。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CollectedHirContracts {
    function_effects: Vec<FunctionEffectContract>,
    continuation_resume_sites: HashMap<CallSite, ContinuationResumeSiteContract>,
    perform_sites: HashMap<CallSite, PerformSiteContract>,
    handle_sites: HashMap<CallSite, HandleSiteContract>,
    call_site_kinds: HashMap<CallSite, TypedCallSiteKind>,
    call_site_contracts: HashMap<CallSite, TypedCallSiteContract>,
    with_update_contracts: HashMap<CallSite, ast::WithUpdateContract>,
    assign_place_contracts: HashMap<CallSite, AssignPlaceContract>,
    top_level_init_roots: Vec<TopLevelInitRootContract>,
    extern_global_contracts: Vec<ExternGlobalContract>,
}

impl CollectedHirContracts {
    pub(crate) fn from_lowered_hir(
        lowered_hir: &LoweredHir,
        source_path: &Path,
    ) -> Result<Self, HirStageError> {
        ContractCollector::new(lowered_hir).collect(source_path)
    }

    pub(crate) fn from_lowered_hir_source_path(
        lowered_hir: &LoweredHir,
        source_path: &Path,
    ) -> Result<Self, HirStageError> {
        ContractCollector::new(lowered_hir).collect_source_path(source_path)
    }

    #[allow(dead_code)]
    pub const fn is_placeholder(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.function_effects.is_empty()
            && self.continuation_resume_sites.is_empty()
            && self.perform_sites.is_empty()
            && self.handle_sites.is_empty()
            && self.call_site_kinds.is_empty()
            && self.call_site_contracts.is_empty()
            && self.with_update_contracts.is_empty()
            && self.assign_place_contracts.is_empty()
            && self.top_level_init_roots.is_empty()
            && self.extern_global_contracts.is_empty()
    }

    #[allow(dead_code)]
    pub fn function_effects(&self) -> &[FunctionEffectContract] {
        &self.function_effects
    }

    #[allow(dead_code)]
    pub fn continuation_resume_sites(&self) -> &HashMap<CallSite, ContinuationResumeSiteContract> {
        &self.continuation_resume_sites
    }

    #[allow(dead_code)]
    pub fn continuation_resume_site(
        &self,
        call_site: &CallSite,
    ) -> Option<&ContinuationResumeSiteContract> {
        self.continuation_resume_sites.get(call_site)
    }

    #[allow(dead_code)]
    pub fn perform_sites(&self) -> &HashMap<CallSite, PerformSiteContract> {
        &self.perform_sites
    }

    #[allow(dead_code)]
    pub fn perform_site(&self, call_site: &CallSite) -> Option<&PerformSiteContract> {
        self.perform_sites.get(call_site)
    }

    #[allow(dead_code)]
    pub fn handle_sites(&self) -> &HashMap<CallSite, HandleSiteContract> {
        &self.handle_sites
    }

    #[allow(dead_code)]
    pub fn handle_site(&self, call_site: &CallSite) -> Option<&HandleSiteContract> {
        self.handle_sites.get(call_site)
    }

    #[allow(dead_code)]
    pub fn call_site_kinds(&self) -> &HashMap<CallSite, TypedCallSiteKind> {
        &self.call_site_kinds
    }

    #[allow(dead_code)]
    pub fn call_site_kind(&self, call_site: &CallSite) -> Option<TypedCallSiteKind> {
        self.call_site_kinds.get(call_site).copied()
    }

    #[allow(dead_code)]
    pub fn call_site_contracts(&self) -> &HashMap<CallSite, TypedCallSiteContract> {
        &self.call_site_contracts
    }

    #[allow(dead_code)]
    pub fn call_site_contract(&self, call_site: &CallSite) -> Option<&TypedCallSiteContract> {
        self.call_site_contracts.get(call_site)
    }

    #[allow(dead_code)]
    pub fn with_update_contracts(&self) -> &HashMap<CallSite, ast::WithUpdateContract> {
        &self.with_update_contracts
    }

    #[allow(dead_code)]
    pub fn assign_place_contracts(&self) -> &HashMap<CallSite, AssignPlaceContract> {
        &self.assign_place_contracts
    }

    #[allow(dead_code)]
    pub fn top_level_init_roots(&self) -> &[TopLevelInitRootContract] {
        &self.top_level_init_roots
    }

    #[allow(dead_code)]
    pub fn extern_global_contracts(&self) -> &[ExternGlobalContract] {
        &self.extern_global_contracts
    }

    /// 以稳定顺序渲染内部 collector；只作为单测调试辅助，正式 dump 使用 `HirFacts`。
    #[allow(dead_code)]
    pub fn stable_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "source_site_contracts {{");

        let _ = writeln!(out, "    function_effects: [");
        for contract in &self.function_effects {
            let _ = writeln!(out, "        FunctionEffectContract {{");
            let _ = writeln!(out, "            span: {:?},", contract.span());
            let _ = writeln!(out, "            fqn: {:?},", contract.fqn());
            let _ = writeln!(
                out,
                "            return_ty: {},",
                types.display(contract.return_ty())
            );
            let _ = writeln!(
                out,
                "            allowed_effects: {},",
                format_effect_row(types, contract.allowed_effects())
            );
            let _ = writeln!(
                out,
                "            effects_closed: {},",
                contract.effects_closed()
            );
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut call_site_kinds = self.call_site_kinds.iter().collect::<Vec<_>>();
        call_site_kinds.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    call_site_kinds: [");
        for (call_site, kind) in call_site_kinds {
            let _ = writeln!(out, "        TypedCallSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(out, "            kind: {:?},", kind);
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut call_site_contracts = self.call_site_contracts.iter().collect::<Vec<_>>();
        call_site_contracts.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    call_site_contracts: [");
        for (call_site, contract) in call_site_contracts {
            format_call_site_contract(&mut out, types, call_site, contract);
        }
        let _ = writeln!(out, "    ],");

        let mut with_update_contracts = self.with_update_contracts.iter().collect::<Vec<_>>();
        with_update_contracts.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    with_update_contracts: [");
        for (call_site, contract) in with_update_contracts {
            format_with_update_contract(&mut out, types, call_site, contract);
        }
        let _ = writeln!(out, "    ],");

        let mut assign_place_contracts = self.assign_place_contracts.iter().collect::<Vec<_>>();
        assign_place_contracts.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    assign_place_contracts: [");
        for (call_site, contract) in assign_place_contracts {
            format_assign_place_contract(&mut out, types, call_site, contract);
        }
        let _ = writeln!(out, "    ],");

        if !self.top_level_init_roots.is_empty() {
            let _ = writeln!(out, "    top_level_init_roots: [");
            for root in &self.top_level_init_roots {
                format_top_level_init_root(&mut out, types, root);
            }
            let _ = writeln!(out, "    ],");
        }

        if !self.extern_global_contracts.is_empty() {
            let _ = writeln!(out, "    extern_global_contracts: [");
            for contract in &self.extern_global_contracts {
                format_extern_global_contract(&mut out, types, contract);
            }
            let _ = writeln!(out, "    ],");
        }

        let mut continuation_resume_sites =
            self.continuation_resume_sites.iter().collect::<Vec<_>>();
        continuation_resume_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    continuation_resume_sites: [");
        for (call_site, contract) in continuation_resume_sites {
            let _ = writeln!(out, "        ContinuationResumeSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(
                out,
                "            receiver_route: {:?},",
                contract.receiver_route()
            );
            let _ = writeln!(
                out,
                "            payload_arg_indices: {:?},",
                contract.payload_arg_indices()
            );
            let _ = writeln!(
                out,
                "            receiver_ty: {},",
                types.display(contract.receiver_ty())
            );
            let _ = writeln!(
                out,
                "            resume_ty: {},",
                types.display(contract.resume_ty())
            );
            let _ = writeln!(
                out,
                "            answer_ty: {},",
                types.display(contract.answer_ty())
            );
            let _ = writeln!(
                out,
                "            return_ty: {},",
                types.display(contract.return_ty())
            );
            let _ = writeln!(
                out,
                "            out_effects: {},",
                format_effect_row(types, contract.out_effects())
            );
            let _ = writeln!(
                out,
                "            required_effects: {},",
                format_required_effects(
                    types,
                    contract.out_effects(),
                    contract.runtime_error_effect_ty(),
                )
            );
            let _ = writeln!(
                out,
                "            includes_runtime_error_effect: {},",
                contract.required_effects_include_runtime_error()
            );
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut perform_sites = self.perform_sites.iter().collect::<Vec<_>>();
        perform_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    perform_sites: [");
        for (call_site, contract) in perform_sites {
            let _ = writeln!(out, "        PerformSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(
                out,
                "            effect_ty: {},",
                types.display(contract.effect_ty())
            );
            let _ = writeln!(out, "            op_fqn: {:?},", contract.op_fqn());
            let _ = writeln!(
                out,
                "            result_ty: {},",
                types.display(contract.result_ty())
            );
            let _ = writeln!(
                out,
                "            payload_ty: {},",
                contract.payload().display(types)
            );
            let _ = writeln!(out, "            payload_components: [");
            for ty in contract.payload().components() {
                let _ = writeln!(out, "                {},", types.display(*ty));
            }
            let _ = writeln!(out, "            ],");
            let _ = writeln!(
                out,
                "            arg_mapping: {:?},",
                contract.arg_mapping()
            );
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut handle_sites = self.handle_sites.iter().collect::<Vec<_>>();
        handle_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    handle_sites: [");
        for (call_site, contract) in handle_sites {
            let _ = writeln!(out, "        HandleSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(
                out,
                "            result_ty: {},",
                types.display(contract.result_ty())
            );
            let _ = writeln!(
                out,
                "            body_result_ty: {},",
                types.display(contract.body_result_ty())
            );
            let _ = writeln!(out, "            arm_contracts: [");
            for arm in contract.arm_contracts() {
                let _ = writeln!(out, "                HandleArmSiteContract {{");
                let _ = writeln!(out, "                    op_fqn: {:?},", arm.op_fqn());
                let _ = writeln!(
                    out,
                    "                    handled_effect_ty: {},",
                    types.display(arm.handled_effect_ty())
                );
                let _ = writeln!(
                    out,
                    "                    payload_ty: {},",
                    arm.payload().display(types)
                );
                let _ = writeln!(out, "                    payload_components: [");
                for ty in arm.payload().components() {
                    let _ = writeln!(out, "                        {},", types.display(*ty));
                }
                let _ = writeln!(out, "                    ],");
                let _ = writeln!(
                    out,
                    "                    body_ty: {},",
                    types.display(arm.body_ty())
                );
                let _ = writeln!(out, "                    kind: {:?},", arm.kind());
                let _ = writeln!(out, "                }},");
            }
            let _ = writeln!(out, "            ],");
            match contract.finally_result_ty() {
                Some(finally_ty) => {
                    let _ = writeln!(
                        out,
                        "            finally_result_ty: Some({}),",
                        types.display(finally_ty)
                    );
                }
                None => {
                    let _ = writeln!(out, "            finally_result_ty: None,");
                }
            }
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let _ = write!(out, "}}");
        out
    }
}

fn stable_dump_source_site_contracts(
    facts: &hir_site_facts::SourceSiteFacts,
    types: &TypeStore,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "source_site_contracts {{");

    let mut function_effects = facts.function_effects.iter().collect::<Vec<_>>();
    function_effects.sort_by(|lhs, rhs| {
        lhs.fqn
            .cmp(&rhs.fqn)
            .then(lhs.source_path.cmp(&rhs.source_path))
            .then(lhs.span.start.cmp(&rhs.span.start))
            .then(lhs.span.end.cmp(&rhs.span.end))
    });
    let _ = writeln!(out, "    function_effects: [");
    for contract in function_effects {
        let _ = writeln!(out, "        FunctionEffectContract {{");
        let _ = writeln!(out, "            span: {:?},", contract.span);
        let _ = writeln!(out, "            fqn: {:?},", contract.fqn);
        let _ = writeln!(
            out,
            "            return_ty: {},",
            format_type_id_lossy(types, contract.return_ty)
        );
        let _ = writeln!(
            out,
            "            allowed_effects: {},",
            format_effect_row(types, &contract.allowed_effects)
        );
        let _ = writeln!(
            out,
            "            effects_closed: {},",
            contract.effects_closed
        );
        let _ = writeln!(out, "        }},");
    }
    let _ = writeln!(out, "    ],");

    let mut argument_bindings = facts.argument_bindings.iter().collect::<Vec<_>>();
    argument_bindings
        .sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    argument_bindings: [");
    for contract in argument_bindings {
        let _ = writeln!(out, "        ArgumentBindingContract {{");
        let _ = writeln!(out, "            span: {:?},", contract.identity.span);
        let _ = writeln!(out, "            params: {:?},", contract.binding.params);
        let _ = writeln!(out, "        }},");
    }
    let _ = writeln!(out, "    ],");

    let mut call_sites = facts.call_sites.iter().collect::<Vec<_>>();
    call_sites.sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    call_site_contracts: [");
    for contract in call_sites {
        format_hir_fact_call_site_contract(&mut out, types, contract);
    }
    let _ = writeln!(out, "    ],");

    let mut with_updates = facts.with_updates.iter().collect::<Vec<_>>();
    with_updates.sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    with_update_contracts: [");
    for contract in with_updates {
        format_hir_fact_with_update_contract(&mut out, types, contract);
    }
    let _ = writeln!(out, "    ],");

    let mut assignments = facts.assignments.iter().collect::<Vec<_>>();
    assignments.sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    assign_place_contracts: [");
    for contract in assignments {
        format_hir_fact_assignment_contract(&mut out, types, contract);
    }
    let _ = writeln!(out, "    ],");

    if !facts.top_level_init_roots.is_empty() {
        let mut roots = facts.top_level_init_roots.iter().collect::<Vec<_>>();
        roots.sort_by(|lhs, rhs| {
            lhs.fqn
                .cmp(&rhs.fqn)
                .then(lhs.span.start.cmp(&rhs.span.start))
                .then(lhs.span.end.cmp(&rhs.span.end))
        });
        let _ = writeln!(out, "    top_level_init_roots: [");
        for root in roots {
            format_hir_fact_top_level_init_root(&mut out, types, root);
        }
        let _ = writeln!(out, "    ],");
    }

    if !facts.extern_globals.is_empty() {
        let mut globals = facts.extern_globals.iter().collect::<Vec<_>>();
        globals.sort_by(|lhs, rhs| {
            lhs.fqn
                .cmp(&rhs.fqn)
                .then(lhs.span.start.cmp(&rhs.span.start))
                .then(lhs.span.end.cmp(&rhs.span.end))
        });
        let _ = writeln!(out, "    extern_global_contracts: [");
        for contract in globals {
            format_hir_fact_extern_global_contract(&mut out, types, contract);
        }
        let _ = writeln!(out, "    ],");
    }

    let mut resumes = facts.continuation_resumes.iter().collect::<Vec<_>>();
    resumes.sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    continuation_resume_sites: [");
    for contract in resumes {
        format_hir_fact_resume_contract(&mut out, types, contract);
    }
    let _ = writeln!(out, "    ],");

    let mut performs = facts.perform_sites.iter().collect::<Vec<_>>();
    performs.sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    perform_sites: [");
    for contract in performs {
        format_hir_fact_perform_contract(&mut out, types, contract);
    }
    let _ = writeln!(out, "    ],");

    let mut handles = facts.handle_sites.iter().collect::<Vec<_>>();
    handles.sort_by(|lhs, rhs| compare_source_site_identity(&lhs.identity, &rhs.identity));
    let _ = writeln!(out, "    handle_sites: [");
    for contract in handles {
        format_hir_fact_handle_contract(&mut out, types, contract);
    }
    let _ = writeln!(out, "    ],");

    let mut patterns = facts.pattern_bindings.iter().collect::<Vec<_>>();
    patterns.sort_by(|lhs, rhs| {
        compare_source_site_identity(&lhs.identity, &rhs.identity)
            .then(lhs.binding_name.cmp(&rhs.binding_name))
    });
    let _ = writeln!(out, "    pattern_bindings: [");
    for contract in patterns {
        let _ = writeln!(out, "        PatternBindingContract {{");
        let _ = writeln!(out, "            span: {:?},", contract.identity.span);
        let _ = writeln!(out, "            name: {:?},", contract.binding_name);
        let _ = writeln!(
            out,
            "            binding_ty: {},",
            format_type_id_lossy(types, contract.binding_ty)
        );
        let _ = writeln!(out, "        }},");
    }
    let _ = writeln!(out, "    ],");

    let _ = write!(out, "}}");
    out
}

fn compare_source_site_identity(
    lhs: &hir_site_facts::SourceSiteIdentity,
    rhs: &hir_site_facts::SourceSiteIdentity,
) -> Ordering {
    lhs.source_path
        .cmp(&rhs.source_path)
        .then(lhs.span.start.cmp(&rhs.span.start))
        .then(lhs.span.end.cmp(&rhs.span.end))
        .then(lhs.site.as_u32().cmp(&rhs.site.as_u32()))
}

fn format_hir_fact_call_site_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::CallSiteContract,
) {
    let _ = writeln!(out, "        CallSiteContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.identity.span);
    let _ = writeln!(out, "            kind: {:?},", contract.kind);
    match &contract.contract {
        hir_site_facts::CallSiteContractKind::DirectTopLevel(function) => {
            format_hir_fact_function_target(out, types, function, "target");
        }
        hir_site_facts::CallSiteContractKind::MemberDirect(member) => {
            format_hir_fact_member_target(out, types, member, "member");
        }
        hir_site_facts::CallSiteContractKind::Extension {
            receiver_ty,
            function,
        } => {
            let _ = writeln!(
                out,
                "            receiver_ty: {},",
                format_type_id_lossy(types, *receiver_ty)
            );
            format_hir_fact_function_target(out, types, function, "target");
        }
        hir_site_facts::CallSiteContractKind::Constructor(ctor) => {
            let _ = writeln!(out, "            owner_fqn: {:?},", ctor.owner_fqn);
            let _ = writeln!(out, "            ctor_span: {:?},", ctor.ctor_span);
            let _ = writeln!(
                out,
                "            result_ty: {},",
                format_type_id_lossy(types, ctor.result_ty)
            );
            let _ = writeln!(out, "            arg_mapping: {:?},", ctor.arg_mapping);
        }
        hir_site_facts::CallSiteContractKind::Closure {
            callee_ty,
            return_ty,
            abi,
            arg_binding,
        }
        | hir_site_facts::CallSiteContractKind::FunValue {
            callee_ty,
            return_ty,
            abi,
            arg_binding,
        }
        | hir_site_facts::CallSiteContractKind::FunPtr {
            callee_ty,
            return_ty,
            abi,
            arg_binding,
        } => {
            let _ = writeln!(
                out,
                "            callee_ty: {},",
                format_type_id_lossy(types, *callee_ty)
            );
            let _ = writeln!(
                out,
                "            return_ty: {},",
                format_type_id_lossy(types, *return_ty)
            );
            let _ = writeln!(out, "            abi: {:?},", abi);
            if let Some(binding) = arg_binding {
                let _ = writeln!(out, "            arg_binding: {:?},", binding.params);
            }
        }
        hir_site_facts::CallSiteContractKind::Virtual(member)
        | hir_site_facts::CallSiteContractKind::Interface(member) => {
            format_hir_fact_member_target(out, types, member, "dispatch");
        }
        hir_site_facts::CallSiteContractKind::Intrinsic { kind, function } => {
            let _ = writeln!(out, "            intrinsic_kind: {:?},", kind);
            let _ = writeln!(out, "            intrinsic_allowed_context: RuntimeOnly,");
            let _ = writeln!(
                out,
                "            intrinsic_runtime_fallback: {},",
                hir_fact_intrinsic_runtime_fallback(kind)
            );
            format_hir_fact_function_target(out, types, function, "target");
        }
        hir_site_facts::CallSiteContractKind::EffectOp(perform) => {
            format_hir_fact_perform_fields(out, types, perform);
        }
        hir_site_facts::CallSiteContractKind::ContinuationResume(resume) => {
            format_hir_fact_resume_fields(out, types, resume);
        }
    }
    let _ = writeln!(out, "        }},");
}

fn hir_fact_intrinsic_runtime_fallback(kind: &hir_site_facts::IntrinsicKind) -> &'static str {
    match kind {
        hir_site_facts::IntrinsicKind::Reflection { .. } => "NormalRuntimeCall",
        hir_site_facts::IntrinsicKind::Platform { .. } => "PlatformQuery",
        hir_site_facts::IntrinsicKind::Gc { .. }
        | hir_site_facts::IntrinsicKind::Runtime { .. } => "RuntimeIntrinsic",
        hir_site_facts::IntrinsicKind::Compiler { .. } => "CompilerLowered",
        hir_site_facts::IntrinsicKind::NamedTable {
            uses_runtime_call, ..
        } => {
            if *uses_runtime_call {
                "RuntimeIntrinsic"
            } else {
                "CompilerLowered"
            }
        }
    }
}

fn format_hir_fact_function_target(
    out: &mut String,
    types: &TypeStore,
    function: &hir_site_facts::FunctionTarget,
    label: &str,
) {
    let _ = writeln!(out, "            {label}_fqn: {:?},", function.fqn);
    let _ = writeln!(
        out,
        "            {label}_decl_span: {:?},",
        function.decl_span
    );
    let _ = writeln!(
        out,
        "            {label}_type_args: [{}],",
        format_type_args(types, &function.type_args)
    );
    let _ = writeln!(
        out,
        "            {label}_eff_args: [{}],",
        format_eff_args(types, &function.eff_args)
    );
    if let Some(binding) = &function.arg_binding {
        let _ = writeln!(
            out,
            "            {label}_arg_binding: {:?},",
            binding.params
        );
    }
}

fn format_hir_fact_member_target(
    out: &mut String,
    types: &TypeStore,
    member: &hir_site_facts::MemberCallTarget,
    label: &str,
) {
    let _ = writeln!(
        out,
        "            {label}_owner_fqn: {:?},",
        member.owner_fqn
    );
    let _ = writeln!(
        out,
        "            {label}_member_name: {:?},",
        member.member_name
    );
    let _ = writeln!(
        out,
        "            {label}_member_fqn: {:?},",
        member.member_fqn
    );
    let _ = writeln!(
        out,
        "            receiver_ty: {},",
        format_type_id_lossy(types, member.receiver_ty)
    );
    format_hir_fact_function_target(out, types, &member.function, "target");
}

fn format_hir_fact_with_update_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::WithUpdateContract,
) {
    let _ = writeln!(out, "        WithUpdateContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.identity.span);
    let _ = writeln!(
        out,
        "            base_ty: {},",
        format_type_id_lossy(types, contract.base_ty)
    );
    let _ = writeln!(
        out,
        "            result_ty: {},",
        format_type_id_lossy(types, contract.result_ty)
    );
    let _ = writeln!(
        out,
        "            aggregates: {},",
        contract.aggregates.len()
    );
    let _ = writeln!(out, "            updates: [");
    for update in &contract.updates {
        let _ = writeln!(out, "                WithUpdateUpdateContract {{");
        let _ = writeln!(out, "                    path: {:?},", update.path);
        let _ = writeln!(
            out,
            "                    target_ty: {},",
            format_type_id_lossy(types, update.target_ty)
        );
        let _ = writeln!(
            out,
            "                    value_ty: {},",
            format_type_id_lossy(types, update.value_ty)
        );
        let _ = writeln!(
            out,
            "                    segments: {},",
            update.segments.len()
        );
        let _ = writeln!(out, "                }},");
    }
    let _ = writeln!(out, "            ],");
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_assignment_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::AssignmentContract,
) {
    let _ = writeln!(out, "        AssignPlaceContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.identity.span);
    let _ = writeln!(out, "            kind: {:?},", contract.kind);
    let _ = writeln!(
        out,
        "            place_ty: {},",
        format_type_id_lossy(types, contract.place_ty)
    );
    let _ = writeln!(
        out,
        "            value_ty: {},",
        format_type_id_lossy(types, contract.value_ty)
    );
    let _ = writeln!(out, "            mutable: {},", contract.mutable);
    let _ = writeln!(
        out,
        "            write_barrier: {:?},",
        contract.write_barrier
    );
    let _ = writeln!(
        out,
        "            unsafe_required: {},",
        contract.unsafe_required
    );
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_top_level_init_root(
    out: &mut String,
    types: &TypeStore,
    root: &hir_site_facts::TopLevelInitRootContract,
) {
    let _ = writeln!(out, "        TopLevelInitRootContract {{");
    let _ = writeln!(out, "            span: {:?},", root.span);
    let _ = writeln!(out, "            fqn: {:?},", root.fqn);
    let _ = writeln!(out, "            kind: {:?},", root.kind);
    let _ = writeln!(
        out,
        "            ty: {},",
        root.ty
            .map(|ty| format_type_id_lossy(types, ty))
            .unwrap_or_else(|| "None".to_string())
    );
    let _ = writeln!(
        out,
        "            initializer_ty: {},",
        root.initializer_ty
            .map(|ty| format_type_id_lossy(types, ty))
            .unwrap_or_else(|| "None".to_string())
    );
    let _ = writeln!(
        out,
        "            has_initializer: {},",
        root.has_initializer
    );
    let _ = writeln!(out, "            dependencies: {:?},", root.dependencies);
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_extern_global_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::ExternGlobalContract,
) {
    let _ = writeln!(out, "        ExternGlobalContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.span);
    let _ = writeln!(out, "            fqn: {:?},", contract.fqn);
    let _ = writeln!(out, "            symbol: {:?},", contract.symbol);
    let _ = writeln!(out, "            linkage: {:?},", contract.linkage);
    let _ = writeln!(out, "            storage: {:?},", contract.storage);
    let _ = writeln!(
        out,
        "            ty: {},",
        format_type_id_lossy(types, contract.ty)
    );
    let _ = writeln!(out, "            mutable: {},", contract.mutable);
    let _ = writeln!(
        out,
        "            initializer_absent: {},",
        contract.initializer_absent
    );
    let _ = writeln!(
        out,
        "            unsafe_required: {},",
        contract.unsafe_required
    );
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_resume_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::ContinuationResumeContract,
) {
    let _ = writeln!(out, "        ContinuationResumeSiteContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.identity.span);
    format_hir_fact_resume_fields(out, types, contract);
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_resume_fields(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::ContinuationResumeContract,
) {
    let _ = writeln!(
        out,
        "            receiver_route: {:?},",
        contract.receiver_route
    );
    let _ = writeln!(
        out,
        "            payload_arg_indices: {:?},",
        contract.payload_arg_indices
    );
    let _ = writeln!(
        out,
        "            receiver_ty: {},",
        format_type_id_lossy(types, contract.receiver_ty)
    );
    let _ = writeln!(
        out,
        "            resume_ty: {},",
        format_type_id_lossy(types, contract.resume_ty)
    );
    let _ = writeln!(
        out,
        "            answer_ty: {},",
        format_type_id_lossy(types, contract.answer_ty)
    );
    let _ = writeln!(
        out,
        "            return_ty: {},",
        format_type_id_lossy(types, contract.return_ty)
    );
    let _ = writeln!(
        out,
        "            out_effects: {},",
        format_effect_row(types, &contract.out_effects)
    );
    let _ = writeln!(
        out,
        "            required_effects: {},",
        format_required_effects(
            types,
            &contract.out_effects,
            contract.runtime_error_effect_ty,
        )
    );
    let _ = writeln!(
        out,
        "            includes_runtime_error_effect: {},",
        contract.runtime_error_effect_ty.is_some()
    );
}

fn format_hir_fact_perform_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::PerformSiteContract,
) {
    let _ = writeln!(out, "        PerformSiteContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.identity.span);
    format_hir_fact_perform_fields(out, types, contract);
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_perform_fields(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::PerformSiteContract,
) {
    let _ = writeln!(
        out,
        "            effect_ty: {},",
        format_type_id_lossy(types, contract.effect_ty)
    );
    let _ = writeln!(out, "            op_fqn: {:?},", contract.op_fqn);
    let _ = writeln!(
        out,
        "            result_ty: {},",
        format_type_id_lossy(types, contract.result_ty)
    );
    format_hir_fact_payload(out, types, &contract.payload, "            ");
    let _ = writeln!(out, "            arg_mapping: {:?},", contract.arg_mapping);
}

fn format_hir_fact_handle_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &hir_site_facts::HandleSiteContract,
) {
    let _ = writeln!(out, "        HandleSiteContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.identity.span);
    let _ = writeln!(
        out,
        "            result_ty: {},",
        format_type_id_lossy(types, contract.result_ty)
    );
    let _ = writeln!(
        out,
        "            body_result_ty: {},",
        format_type_id_lossy(types, contract.body_result_ty)
    );
    let _ = writeln!(out, "            arm_contracts: [");
    for arm in &contract.arm_contracts {
        let _ = writeln!(out, "                HandleArmSiteContract {{");
        let _ = writeln!(out, "                    op_fqn: {:?},", arm.op_fqn);
        let _ = writeln!(
            out,
            "                    handled_effect_ty: {},",
            format_type_id_lossy(types, arm.handled_effect_ty)
        );
        format_hir_fact_payload(out, types, &arm.payload, "                    ");
        let _ = writeln!(
            out,
            "                    body_ty: {},",
            format_type_id_lossy(types, arm.body_ty)
        );
        let _ = writeln!(out, "                    kind: {:?},", arm.kind);
        let _ = writeln!(out, "                }},");
    }
    let _ = writeln!(out, "            ],");
    let _ = writeln!(
        out,
        "            finally_result_ty: {},",
        contract
            .finally_result_ty
            .map(|ty| format_type_id_lossy(types, ty))
            .unwrap_or_else(|| "None".to_string())
    );
    let _ = writeln!(out, "        }},");
}

fn format_hir_fact_payload(
    out: &mut String,
    types: &TypeStore,
    payload: &hir_site_facts::PayloadTypeContract,
    indent: &str,
) {
    let _ = writeln!(
        out,
        "{indent}payload_ty: {},",
        payload
            .ty
            .map(|ty| format_type_id_lossy(types, ty))
            .unwrap_or_else(|| "None".to_string())
    );
    let _ = writeln!(out, "{indent}payload_components: [");
    for ty in &payload.components {
        let _ = writeln!(out, "{indent}    {},", format_type_id_lossy(types, *ty));
    }
    let _ = writeln!(out, "{indent}],");
}

/// HIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P2/P3 及后续阶段直接消费：
/// - 输出已经过 resolver + typecheck，可直接视为 typed HIR handoff；
/// - 对外 handoff 由 HIR 本体与完整 `hir_facts` 组成，后续 stage 只能经由
///   `hir_facts()` 入口审计和消费 HIR semantic facts；
/// - `dump-hir` 必须优先消费这一 stage 输出，而不是 legacy
///   `hir::lower_for_dump(...)`；
/// - 内部 `CollectedHirContracts` 只用于构建/测试 `HirFacts`，不能作为公开 stage output。
#[derive(Debug)]
pub struct HirStageOutput {
    lowered_hir: LoweredHir,
    hir_facts: HirFacts,
    source_path: PathBuf,
}

impl HirStageOutput {
    pub fn new(lowered_hir: LoweredHir, source_path: &Path) -> Result<Self, HirStageError> {
        HirCompletenessVerifier::new(&lowered_hir, source_path).verify()?;
        Self::new_checked(lowered_hir, source_path)
    }

    fn new_checked(mut lowered_hir: LoweredHir, source_path: &Path) -> Result<Self, HirStageError> {
        ensure_raise_runtime_error_effect(&mut lowered_hir.types);
        let collected_contracts =
            CollectedHirContracts::from_lowered_hir(&lowered_hir, source_path)?;
        let hir_facts = build_hir_facts(&lowered_hir, &collected_contracts, source_path)?;
        Ok(Self {
            lowered_hir,
            hir_facts,
            source_path: source_path.to_path_buf(),
        })
    }

    pub fn hir_file(&self) -> &crate::hir::File {
        &self.lowered_hir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_hir.types
    }

    pub fn lowered_hir(&self) -> &LoweredHir {
        &self.lowered_hir
    }

    pub fn hir_facts(&self) -> &HirFacts {
        &self.hir_facts
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// 以稳定文本渲染 HIR stage dump：先打印 HIR `File`，再追加 `HirFacts` 摘要与详细
    /// source-site facts。
    pub fn stable_dump(&self) -> String {
        let mut out =
            crate::hir::stable_dump_file(self.hir_file(), self.types(), self.source_path());
        out.push('\n');
        out.push('\n');
        out.push_str(&self.hir_facts.dump());
        out.push('\n');
        out.push('\n');
        out.push_str(&stable_dump_source_site_contracts(
            &self.hir_facts.source_sites,
            self.types(),
        ));
        out.push('\n');
        out
    }

    pub fn into_lowered_hir(self) -> LoweredHir {
        self.lowered_hir
    }
}

pub fn run(session: &Session, source: &SourceFile) -> Result<HirStageOutput, HirLowerError> {
    let lowered_hir = crate::hir::lower_typed_for_dump(session, source)?;
    HirStageOutput::new(lowered_hir, source.path()).map_err(HirLowerError::from)
}

fn build_hir_facts(
    lowered_hir: &LoweredHir,
    collected_contracts: &CollectedHirContracts,
    source_path: &Path,
) -> Result<HirFacts, HirStageError> {
    let mut facts = build_hir_declaration_facts_core(lowered_hir);
    populate_source_site_facts(&mut facts.source_sites, lowered_hir, collected_contracts)?;
    verify_built_hir_facts(&facts, source_path)?;
    Ok(facts)
}

pub fn build_hir_declaration_facts_from_lowered_hir(
    lowered_hir: &LoweredHir,
    source_path: &Path,
) -> Result<HirFacts, HirStageError> {
    let collected_contracts =
        CollectedHirContracts::from_lowered_hir_source_path(lowered_hir, source_path)?;
    let mut facts = build_hir_declaration_facts_core(lowered_hir);
    populate_source_site_facts(&mut facts.source_sites, lowered_hir, &collected_contracts)?;
    verify_built_hir_facts(&facts, source_path)?;
    Ok(facts)
}

pub fn build_hir_facts_from_lowered_hir(
    lowered_hir: &LoweredHir,
    source_path: &Path,
) -> Result<HirFacts, HirStageError> {
    let collected_contracts = CollectedHirContracts::from_lowered_hir(lowered_hir, source_path)?;
    let mut facts = build_hir_declaration_facts_core(lowered_hir);
    populate_source_site_facts(&mut facts.source_sites, lowered_hir, &collected_contracts)?;
    verify_built_hir_facts(&facts, source_path)?;
    Ok(facts)
}

fn build_hir_declaration_facts_core(lowered_hir: &LoweredHir) -> HirFacts {
    let mut facts = HirFacts::new();
    facts.type_context.type_universe = Some(TypeContextReference {
        label: "hir-stage-type-store".to_string(),
        type_count: lowered_hir.types.len(),
        builtins: Some(lowered_hir.builtins),
    });
    populate_type_context_facts(&mut facts, lowered_hir);
    populate_declaration_facts(&mut facts.declarations, lowered_hir);
    populate_global_root_facts(&mut facts, lowered_hir);
    populate_native_extern_facts(&mut facts, lowered_hir);
    facts
}

fn populate_source_site_facts(
    facts: &mut hir_site_facts::SourceSiteFacts,
    lowered_hir: &LoweredHir,
    contracts: &CollectedHirContracts,
) -> Result<(), HirStageError> {
    facts.function_effects = contracts
        .function_effects
        .iter()
        .map(function_effect_fact)
        .collect();

    let mut call_site_contracts = contracts.call_site_contracts.iter().collect::<Vec<_>>();
    call_site_contracts.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
    for (call_site, contract) in call_site_contracts {
        facts
            .call_sites
            .push(call_site_contract_fact(call_site, contract));
        if let Some(instance_fact) = call_site_instance_fact(call_site, contract) {
            facts.call_site_instances.push(instance_fact);
        }
        if let Some(binding_fact) = template_site_binding_fact(call_site, contract) {
            facts.template_site_bindings.push(binding_fact);
        }
        if let Some(candidate_fact) = dispatch_candidate_fact(lowered_hir, call_site, contract) {
            facts.dispatch_candidates.push(candidate_fact);
        }
        if let Some(binding) = call_site_contract_arg_binding(contract) {
            facts
                .argument_bindings
                .push(hir_site_facts::ArgumentBindingContract {
                    identity: source_site_identity(call_site, "argument"),
                    binding: call_arg_binding_fact(binding),
                });
        }
    }

    facts
        .template_site_bindings
        .extend(template_value_binding_facts(lowered_hir));
    facts.template_site_bindings.sort_by(|lhs, rhs| {
        lhs.identity
            .source_path
            .cmp(&rhs.identity.source_path)
            .then(lhs.identity.span.start.cmp(&rhs.identity.span.start))
            .then(lhs.identity.span.end.cmp(&rhs.identity.span.end))
            .then((lhs.kind as u8).cmp(&(rhs.kind as u8)))
    });

    let mut assign_place_contracts = contracts.assign_place_contracts.iter().collect::<Vec<_>>();
    assign_place_contracts.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
    facts.assignments = assign_place_contracts
        .into_iter()
        .map(|(call_site, contract)| assignment_contract_fact(call_site, contract))
        .collect();

    let mut with_update_contracts = contracts.with_update_contracts.iter().collect::<Vec<_>>();
    with_update_contracts.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
    facts.with_updates = with_update_contracts
        .into_iter()
        .map(|(call_site, contract)| with_update_contract_fact(call_site, contract))
        .collect();

    let mut perform_sites = contracts.perform_sites.iter().collect::<Vec<_>>();
    perform_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
    facts.perform_sites = perform_sites
        .into_iter()
        .map(|(call_site, contract)| perform_site_fact(call_site, contract))
        .collect();

    let mut handle_sites = contracts.handle_sites.iter().collect::<Vec<_>>();
    handle_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
    facts.handle_sites = handle_sites
        .into_iter()
        .map(|(call_site, contract)| handle_site_fact(call_site, contract))
        .collect();

    let mut continuation_resumes = contracts
        .continuation_resume_sites
        .iter()
        .collect::<Vec<_>>();
    continuation_resumes.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
    facts.continuation_resumes = continuation_resumes
        .into_iter()
        .map(|(call_site, contract)| continuation_resume_fact(call_site, contract))
        .collect();

    let pattern_binding_names = collect_when_pat_binding_names(lowered_hir);
    facts.pattern_bindings = lowered_hir
        .when_pat_binding_tys
        .iter()
        .map(|(site, ty)| hir_site_facts::PatternBindingContract {
            identity: source_site_identity_from_parts(&site.source_path, site.decl_span, "pattern"),
            binding_name: pattern_binding_names
                .get(site)
                .cloned()
                .unwrap_or_else(|| format!("{}..{}", site.decl_span.start, site.decl_span.end)),
            binding_ty: *ty,
        })
        .collect();
    facts.pattern_bindings.sort_by(|lhs, rhs| {
        lhs.identity
            .source_path
            .cmp(&rhs.identity.source_path)
            .then(lhs.identity.span.start.cmp(&rhs.identity.span.start))
            .then(lhs.binding_name.cmp(&rhs.binding_name))
    });

    facts.top_level_init_roots = contracts
        .top_level_init_roots
        .iter()
        .map(top_level_init_root_fact)
        .collect();
    facts.extern_globals = contracts
        .extern_global_contracts
        .iter()
        .map(extern_global_contract_fact)
        .collect();

    let source_effect_facts = SourceEffectFactBuilder::new(lowered_hir, contracts)?;
    facts.callable_source_effects = source_effect_facts.callable_source_effects()?;
    facts.semantic_operations = source_effect_facts.semantic_operations()?;
    facts.hidden_initializers = source_effect_facts.hidden_initializers()?;

    Ok(())
}

#[derive(Clone)]
struct ComputedCallableEffect {
    direct: EffectRow,
    inferred: EffectRow,
    published: EffectRow,
    row_is_closed: bool,
    status: hir_site_facts::EffectInferenceStatus,
}

struct SourceEffectFactBuilder<'a> {
    lowered_hir: &'a LoweredHir,
    contracts: &'a CollectedHirContracts,
    functions: Vec<&'a FunDecl>,
    functions_by_fqn: HashMap<String, &'a FunDecl>,
    effect_param_keys: HashMap<TypeParamType, StableEffectParamKey>,
}

impl<'a> SourceEffectFactBuilder<'a> {
    fn new(
        lowered_hir: &'a LoweredHir,
        contracts: &'a CollectedHirContracts,
    ) -> Result<Self, HirStageError> {
        let mut functions = Vec::new();
        let mut functions_by_fqn = HashMap::new();
        for item in &lowered_hir.file.items {
            if let Item::Fun(fun) = item {
                functions_by_fqn.insert(fun.fqn.clone(), fun);
                functions.push(fun);
            }
        }
        for fun in &lowered_hir.member_funs {
            functions_by_fqn.insert(fun.fqn.clone(), fun);
            functions.push(fun);
        }

        Ok(Self {
            lowered_hir,
            contracts,
            functions,
            functions_by_fqn,
            effect_param_keys: stable_effect_param_keys(lowered_hir)?,
        })
    }

    fn callable_source_effects(
        &self,
    ) -> Result<Vec<hir_site_facts::CallableSourceEffectFacts>, HirStageError> {
        let mut computer = SourceEffectComputer::new(self);
        let mut facts = Vec::with_capacity(self.functions.len());
        for fun in &self.functions {
            let computed = computer.compute_callable(fun);
            facts.push(hir_site_facts::CallableSourceEffectFacts {
                fqn: fun.fqn.clone(),
                source_path: fun.source_path.clone(),
                span: fun.span,
                return_ty: fun.return_ty,
                declared_surface_row: fun
                    .declared_effects
                    .as_ref()
                    .map(|row| self.effect_row_template(row, fun.effects_closed, fun))
                    .transpose()?,
                direct_effect_row: self.effect_row_template(&computed.direct, false, fun)?,
                inferred_surface_row_template: self.effect_row_template(
                    &computed.inferred,
                    false,
                    fun,
                )?,
                published_surface_row_template: self.effect_row_template(
                    &computed.published,
                    computed.row_is_closed,
                    fun,
                )?,
                row_is_closed: computed.row_is_closed,
                inference_status: computed.status,
            });
        }
        facts.sort_by(|lhs, rhs| {
            lhs.source_path
                .cmp(&rhs.source_path)
                .then(lhs.span.start.cmp(&rhs.span.start))
                .then(lhs.fqn.cmp(&rhs.fqn))
        });
        Ok(facts)
    }

    fn semantic_operations(
        &self,
    ) -> Result<Vec<hir_site_facts::CanonicalSemanticOperationFact>, HirStageError> {
        let mut computer = SourceEffectComputer::new(self);
        let mut call_sites = self
            .contracts
            .call_site_contracts
            .iter()
            .collect::<Vec<_>>();
        call_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let mut facts = Vec::with_capacity(call_sites.len());
        for (call_site, contract) in call_sites {
            let surface = computer.call_contract_surface_row(contract);
            facts.push(hir_site_facts::CanonicalSemanticOperationFact {
                identity: source_site_identity(call_site, "semantic"),
                kind: semantic_operation_kind(contract),
                surface_row: self.effect_row_template_for_site(&surface, false, call_site)?,
            });
        }
        Ok(facts)
    }

    fn hidden_initializers(
        &self,
    ) -> Result<Vec<hir_site_facts::HiddenInitializerEffectFact>, HirStageError> {
        let mut computer = SourceEffectComputer::new(self);
        let mut facts = Vec::new();

        let mut classes = self.lowered_hir.class_inits.values().collect::<Vec<_>>();
        classes.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for class in classes {
            if class.ctors.is_empty() {
                let row = computer.class_ctor_effect_row(&class.fqn, None);
                facts.push(hir_site_facts::HiddenInitializerEffectFact {
                    kind: hir_site_facts::HiddenInitializerKind::ClassCtor,
                    fqn: class.fqn.clone(),
                    source_path: class.source_path.clone(),
                    span: None,
                    effect_row: self.effect_row_template_for_parts(
                        &row,
                        false,
                        &class.source_path,
                        Span::new(0, 0),
                        &class.fqn,
                    )?,
                });
            }
            for ctor in &class.ctors {
                let row = computer.class_ctor_effect_row(&class.fqn, Some(ctor.span));
                facts.push(hir_site_facts::HiddenInitializerEffectFact {
                    kind: hir_site_facts::HiddenInitializerKind::ClassCtor,
                    fqn: class.fqn.clone(),
                    source_path: class.source_path.clone(),
                    span: Some(ctor.span),
                    effect_row: self.effect_row_template_for_parts(
                        &row,
                        false,
                        &class.source_path,
                        ctor.span,
                        &class.fqn,
                    )?,
                });
            }
        }

        let mut objects = self.lowered_hir.object_inits.values().collect::<Vec<_>>();
        objects.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for object in objects {
            let row = computer.object_init_effect_row(&object.fqn);
            facts.push(hir_site_facts::HiddenInitializerEffectFact {
                kind: hir_site_facts::HiddenInitializerKind::ObjectInit,
                fqn: object.fqn.clone(),
                source_path: object.source_path.clone(),
                span: Some(object.span),
                effect_row: self.effect_row_template_for_parts(
                    &row,
                    false,
                    &object.source_path,
                    object.span,
                    &object.fqn,
                )?,
            });
        }

        let mut values = self
            .lowered_hir
            .top_level_immutable_values
            .values()
            .collect::<Vec<_>>();
        values.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for value in values {
            let row = computer.top_level_immutable_value_effect_row(&value.fqn);
            facts.push(hir_site_facts::HiddenInitializerEffectFact {
                kind: hir_site_facts::HiddenInitializerKind::TopLevelInit,
                fqn: value.fqn.clone(),
                source_path: value.source_path.clone(),
                span: Some(value.span),
                effect_row: self.effect_row_template_for_parts(
                    &row,
                    false,
                    &value.source_path,
                    value.span,
                    &value.fqn,
                )?,
            });
        }

        Ok(facts)
    }

    fn effect_row_template(
        &self,
        row: &EffectRow,
        closed: bool,
        fun: &FunDecl,
    ) -> Result<hir_site_facts::EffectRowTemplate, HirStageError> {
        self.effect_row_template_for_parts(row, closed, &fun.source_path, fun.span, &fun.fqn)
    }

    fn effect_row_template_for_site(
        &self,
        row: &EffectRow,
        closed: bool,
        call_site: &CallSite,
    ) -> Result<hir_site_facts::EffectRowTemplate, HirStageError> {
        self.effect_row_template_for_parts(
            row,
            closed,
            &call_site.source_path,
            call_site.span,
            "source semantic operation",
        )
    }

    fn effect_row_template_for_parts(
        &self,
        row: &EffectRow,
        closed: bool,
        source_path: &Path,
        span: Span,
        owner: &str,
    ) -> Result<hir_site_facts::EffectRowTemplate, HirStageError> {
        let template = StableEffectRowTemplate::from_effect_row(
            &self.lowered_hir.types,
            row,
            &self.lowered_hir.stable_type_param_keys,
            &self.effect_param_keys,
            closed,
        )
        .map_err(|err| hir_stage_effect_template_error(source_path, span, owner, err))?;
        Ok(effect_row_template_fact(&template))
    }
}

struct SourceEffectComputer<'a, 'b> {
    builder: &'a SourceEffectFactBuilder<'b>,
    cache: HashMap<String, ComputedCallableEffect>,
    visiting_callables: HashSet<String>,
    visiting_hidden: HashSet<String>,
}

impl<'a, 'b> SourceEffectComputer<'a, 'b> {
    fn new(builder: &'a SourceEffectFactBuilder<'b>) -> Self {
        Self {
            builder,
            cache: HashMap::new(),
            visiting_callables: HashSet::new(),
            visiting_hidden: HashSet::new(),
        }
    }

    fn compute_callable(&mut self, fun: &FunDecl) -> ComputedCallableEffect {
        if let Some(cached) = self.cache.get(&fun.fqn).cloned() {
            return cached;
        }
        if !self.visiting_callables.insert(fun.fqn.clone()) {
            let fallback = fun
                .declared_effects
                .clone()
                .or_else(|| {
                    function_effect_contract(&self.builder.lowered_hir.types, fun.ty)
                        .map(|row| row.0)
                })
                .unwrap_or_else(EffectRow::pure);
            return ComputedCallableEffect {
                direct: EffectRow::pure(),
                inferred: fallback.clone(),
                published: fallback,
                row_is_closed: fun.effects_closed,
                status: hir_site_facts::EffectInferenceStatus::ExplicitContract,
            };
        }

        let direct = fun
            .body
            .as_ref()
            .map(|body| self.block_direct_row(body, fun.source_path.as_path()))
            .unwrap_or_else(EffectRow::pure);
        let inferred = fun
            .body
            .as_ref()
            .map(|body| self.block_surface_row(body, fun.source_path.as_path()))
            .or_else(|| fun.declared_effects.clone())
            .unwrap_or_else(EffectRow::pure);
        let (published, row_is_closed, status) =
            if let Some(declared) = fun.declared_effects.clone() {
                (
                    declared,
                    fun.effects_closed,
                    hir_site_facts::EffectInferenceStatus::ExplicitContract,
                )
            } else if fun.body.is_some() {
                (
                    inferred.clone(),
                    false,
                    hir_site_facts::EffectInferenceStatus::InferredFromBody,
                )
            } else {
                (
                    EffectRow::pure(),
                    false,
                    hir_site_facts::EffectInferenceStatus::MissingBodyAssumedPure,
                )
            };

        let out = ComputedCallableEffect {
            direct,
            inferred,
            published,
            row_is_closed,
            status,
        };
        self.visiting_callables.remove(&fun.fqn);
        self.cache.insert(fun.fqn.clone(), out.clone());
        out
    }

    fn block_surface_row(&mut self, block: &Block, source_path: &Path) -> EffectRow {
        let mut terms = Vec::new();
        for stmt in &block.stmts {
            terms.extend(self.stmt_surface_row(stmt, source_path).terms);
        }
        EffectRow::new(terms)
    }

    fn stmt_surface_row(&mut self, stmt: &Stmt, source_path: &Path) -> EffectRow {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.expr_surface_row(expr, source_path),
            StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .map(|expr| self.expr_surface_row(expr, source_path))
                .unwrap_or_else(EffectRow::pure),
            StmtKind::Assign { lhs, rhs, .. } => union_rows([
                self.expr_surface_row(lhs, source_path),
                self.expr_surface_row(rhs, source_path),
            ]),
            StmtKind::While { cond, body } => union_rows([
                self.expr_surface_row(cond, source_path),
                self.block_surface_row(body, source_path),
            ]),
            StmtKind::Return { value } => value
                .as_ref()
                .map(|expr| self.expr_surface_row(expr, source_path))
                .unwrap_or_else(EffectRow::pure),
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => EffectRow::pure(),
        }
    }

    fn expr_surface_row(&mut self, expr: &Expr, source_path: &Path) -> EffectRow {
        match &expr.kind {
            ExprKind::Missing
            | ExprKind::Literal(_)
            | ExprKind::ClassLiteral(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::Closure(_)
            | ExprKind::Todo(_) => EffectRow::pure(),
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => union_rows([
                self.object_init_effect_row(fqn),
                self.top_level_immutable_value_effect_row(fqn),
            ]),
            ExprKind::VarRef(_) => EffectRow::pure(),
            ExprKind::Block(block) => self.block_surface_row(block, source_path),
            ExprKind::Unary { expr: operand, .. } => union_rows([
                self.expr_surface_row(operand, source_path),
                self.semantic_surface_row(source_path, expr.span),
            ]),
            ExprKind::Cast { expr, .. } | ExprKind::TypeCheck { expr, .. } => {
                self.expr_surface_row(expr, source_path)
            }
            ExprKind::Binary { lhs, rhs, .. } => union_rows([
                self.expr_surface_row(lhs, source_path),
                self.expr_surface_row(rhs, source_path),
                self.semantic_surface_row(source_path, expr.span),
            ]),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut rows = vec![
                    self.expr_surface_row(cond, source_path),
                    self.expr_surface_row(then_branch, source_path),
                ];
                if let Some(else_branch) = else_branch.as_deref() {
                    rows.push(self.expr_surface_row(else_branch, source_path));
                }
                union_rows(rows)
            }
            ExprKind::When { subject, arms } => {
                let mut rows = vec![self.expr_surface_row(subject, source_path)];
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        rows.push(self.expr_surface_row(guard, source_path));
                    }
                    rows.push(self.expr_surface_row(&arm.body, source_path));
                }
                union_rows(rows)
            }
            ExprKind::MemberAccess { receiver, member } => {
                let mut rows = vec![self.expr_surface_row(receiver, source_path)];
                if let Some(MemberRef::Value { fqn, .. }) = member.resolved.as_ref()
                    && let Some((owner_fqn, _)) = fqn.rsplit_once('.')
                {
                    rows.push(self.object_init_effect_row(owner_fqn));
                }
                rows.push(self.semantic_surface_row(source_path, expr.span));
                union_rows(rows)
            }
            ExprKind::StructLit { fields, .. } => union_rows(
                fields
                    .iter()
                    .map(|field| self.expr_surface_row(&field.value, source_path)),
            ),
            ExprKind::TupleLit { elements } => union_rows(
                elements
                    .iter()
                    .map(|element| self.expr_surface_row(element, source_path)),
            ),
            ExprKind::InterpolatedString { parts, .. } => {
                union_rows(parts.iter().filter_map(|part| {
                    if let InterpolatedStringPart::Expr { expr } = part {
                        Some(self.expr_surface_row(expr, source_path))
                    } else {
                        None
                    }
                }))
            }
            ExprKind::Call { callee, args } => {
                let mut rows = vec![
                    self.expr_surface_row(callee, source_path),
                    self.call_args_surface_row(args, source_path),
                    self.semantic_surface_row(source_path, expr.span),
                ];
                if let Some(info) = self
                    .builder
                    .lowered_hir
                    .ctor_call_sites
                    .get(&CallSite::new(source_path.to_path_buf(), expr.span))
                {
                    rows.push(self.class_ctor_effect_row(&info.class_fqn, info.ctor_span));
                } else if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) =
                    self.builder.lowered_hir.types.kind(callee.ty)
                {
                    rows.push(fun_ty.effects.clone());
                }
                union_rows(rows)
            }
            ExprKind::Perform {
                effect_ty, args, ..
            } => union_rows([
                self.call_args_surface_row(args, source_path),
                EffectRow::new(vec![*effect_ty]),
            ]),
            ExprKind::Handle(handle) => {
                let handled = EffectRow::new(
                    handle
                        .arms
                        .iter()
                        .map(|arm| arm.op.effect_ty)
                        .collect::<Vec<_>>(),
                );
                let body =
                    subtract_row(self.block_surface_row(&handle.body, source_path), &handled);
                let mut rows = vec![body];
                for arm in &handle.arms {
                    rows.push(self.expr_surface_row(&arm.body, source_path));
                }
                if let Some(finally) = handle.finally.as_ref() {
                    rows.push(self.block_surface_row(finally, source_path));
                }
                union_rows(rows)
            }
        }
    }

    fn block_direct_row(&mut self, block: &Block, source_path: &Path) -> EffectRow {
        let mut terms = Vec::new();
        for stmt in &block.stmts {
            terms.extend(self.stmt_direct_row(stmt, source_path).terms);
        }
        EffectRow::new(terms)
    }

    fn stmt_direct_row(&mut self, stmt: &Stmt, source_path: &Path) -> EffectRow {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.expr_direct_row(expr, source_path),
            StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .map(|expr| self.expr_direct_row(expr, source_path))
                .unwrap_or_else(EffectRow::pure),
            StmtKind::Assign { lhs, rhs, .. } => union_rows([
                self.expr_direct_row(lhs, source_path),
                self.expr_direct_row(rhs, source_path),
            ]),
            StmtKind::While { cond, body } => union_rows([
                self.expr_direct_row(cond, source_path),
                self.block_direct_row(body, source_path),
            ]),
            StmtKind::Return { value } => value
                .as_ref()
                .map(|expr| self.expr_direct_row(expr, source_path))
                .unwrap_or_else(EffectRow::pure),
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => EffectRow::pure(),
        }
    }

    fn expr_direct_row(&mut self, expr: &Expr, source_path: &Path) -> EffectRow {
        match &expr.kind {
            ExprKind::Perform {
                effect_ty, args, ..
            } => union_rows([
                self.call_args_direct_row(args, source_path),
                EffectRow::new(vec![*effect_ty]),
            ]),
            ExprKind::Closure(_) => EffectRow::pure(),
            ExprKind::Block(block) => self.block_direct_row(block, source_path),
            ExprKind::Unary { expr, .. }
            | ExprKind::Cast { expr, .. }
            | ExprKind::TypeCheck { expr, .. } => self.expr_direct_row(expr, source_path),
            ExprKind::Binary { lhs, rhs, .. } => union_rows([
                self.expr_direct_row(lhs, source_path),
                self.expr_direct_row(rhs, source_path),
            ]),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut rows = vec![
                    self.expr_direct_row(cond, source_path),
                    self.expr_direct_row(then_branch, source_path),
                ];
                if let Some(else_branch) = else_branch.as_deref() {
                    rows.push(self.expr_direct_row(else_branch, source_path));
                }
                union_rows(rows)
            }
            ExprKind::When { subject, arms } => {
                let mut rows = vec![self.expr_direct_row(subject, source_path)];
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        rows.push(self.expr_direct_row(guard, source_path));
                    }
                    rows.push(self.expr_direct_row(&arm.body, source_path));
                }
                union_rows(rows)
            }
            ExprKind::MemberAccess { receiver, .. } => self.expr_direct_row(receiver, source_path),
            ExprKind::StructLit { fields, .. } => union_rows(
                fields
                    .iter()
                    .map(|field| self.expr_direct_row(&field.value, source_path)),
            ),
            ExprKind::TupleLit { elements } => union_rows(
                elements
                    .iter()
                    .map(|element| self.expr_direct_row(element, source_path)),
            ),
            ExprKind::InterpolatedString { parts, .. } => {
                union_rows(parts.iter().filter_map(|part| {
                    if let InterpolatedStringPart::Expr { expr } = part {
                        Some(self.expr_direct_row(expr, source_path))
                    } else {
                        None
                    }
                }))
            }
            ExprKind::Call { callee, args } => union_rows([
                self.expr_direct_row(callee, source_path),
                self.call_args_direct_row(args, source_path),
            ]),
            ExprKind::Handle(handle) => {
                let mut rows = vec![self.block_direct_row(&handle.body, source_path)];
                for arm in &handle.arms {
                    rows.push(self.expr_direct_row(&arm.body, source_path));
                }
                if let Some(finally) = handle.finally.as_ref() {
                    rows.push(self.block_direct_row(finally, source_path));
                }
                union_rows(rows)
            }
            ExprKind::Missing
            | ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::ClassLiteral(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::Todo(_) => EffectRow::pure(),
        }
    }

    fn call_args_surface_row(&mut self, args: &[CallArg], source_path: &Path) -> EffectRow {
        union_rows(args.iter().map(|arg| match arg {
            CallArg::Positional(expr) => self.expr_surface_row(expr, source_path),
            CallArg::Named { value, .. } => self.expr_surface_row(value, source_path),
        }))
    }

    fn call_args_direct_row(&mut self, args: &[CallArg], source_path: &Path) -> EffectRow {
        union_rows(args.iter().map(|arg| match arg {
            CallArg::Positional(expr) => self.expr_direct_row(expr, source_path),
            CallArg::Named { value, .. } => self.expr_direct_row(value, source_path),
        }))
    }

    fn semantic_surface_row(&mut self, source_path: &Path, span: Span) -> EffectRow {
        let call_site = CallSite::new(source_path.to_path_buf(), span);
        self.builder
            .contracts
            .call_site_contracts
            .get(&call_site)
            .map(|contract| self.call_contract_surface_row(contract))
            .unwrap_or_else(EffectRow::pure)
    }

    fn call_contract_surface_row(&mut self, contract: &TypedCallSiteContract) -> EffectRow {
        match contract {
            TypedCallSiteContract::DirectTopLevel(function) => {
                self.function_target_surface_row(function)
            }
            TypedCallSiteContract::MemberDirect(member)
            | TypedCallSiteContract::Virtual(member)
            | TypedCallSiteContract::Interface(member) => {
                self.function_target_surface_row(member.function())
            }
            TypedCallSiteContract::Extension { function, .. }
            | TypedCallSiteContract::Intrinsic { function, .. } => {
                self.function_target_surface_row(function)
            }
            TypedCallSiteContract::Constructor(ctor) => {
                self.class_ctor_effect_row(ctor.owner_fqn(), ctor.ctor_span())
            }
            TypedCallSiteContract::Closure { callee_ty, .. }
            | TypedCallSiteContract::FunValue { callee_ty, .. }
            | TypedCallSiteContract::FunPtr { callee_ty, .. } => {
                self.function_type_surface_row(*callee_ty)
            }
            TypedCallSiteContract::EffectOp(perform) => EffectRow::new(vec![perform.effect_ty()]),
            TypedCallSiteContract::ContinuationResume(resume) => {
                let mut terms = resume.out_effects().terms.clone();
                if let Some(effect) = resume.runtime_error_effect_ty() {
                    terms.push(effect);
                }
                EffectRow::new(terms)
            }
        }
    }

    fn function_target_surface_row(&mut self, target: &FunctionTargetContract) -> EffectRow {
        let row = if let Some(fun) = self.builder.functions_by_fqn.get(target.fqn()).copied() {
            self.compute_callable(fun).published
        } else {
            self.function_row_for_fqn(target.fqn())
                .unwrap_or_else(EffectRow::pure)
        };
        self.substitute_target_effect_args(row, target.eff_args())
    }

    fn function_row_for_fqn(&self, fqn: &str) -> Option<EffectRow> {
        self.builder
            .functions_by_fqn
            .get(fqn)
            .and_then(|fun| function_effect_contract(&self.builder.lowered_hir.types, fun.ty))
            .map(|(row, _)| row)
    }

    fn function_type_surface_row(&self, ty: TypeId) -> EffectRow {
        match self.builder.lowered_hir.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => fun_ty.effects.clone(),
            _ => EffectRow::pure(),
        }
    }

    fn substitute_target_effect_args(&self, row: EffectRow, eff_args: &[EffectRow]) -> EffectRow {
        if eff_args.is_empty() || row.terms.is_empty() {
            return row;
        }
        let mut markers = row
            .terms
            .iter()
            .filter_map(|term| match self.builder.lowered_hir.types.kind(*term) {
                TypeKind::Param(param)
                    if param.decl_file.as_os_str() == EFFECT_ROW_PARAM_DECL_FILE =>
                {
                    Some((
                        *term,
                        self.builder
                            .lowered_hir
                            .stable_type_param_keys
                            .get(param)
                            .map(|key| key.index())
                            .unwrap_or(usize::MAX),
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        markers.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1).then(lhs.0.cmp(&rhs.0)));
        markers.dedup_by_key(|(ty, _)| *ty);
        let marker_to_arg = markers
            .into_iter()
            .enumerate()
            .filter_map(|(idx, (marker, _))| eff_args.get(idx).map(|row| (marker, row)))
            .collect::<HashMap<_, _>>();

        let mut terms = Vec::new();
        for term in row.terms {
            if let Some(arg_row) = marker_to_arg.get(&term) {
                terms.extend(arg_row.terms.iter().copied());
            } else {
                terms.push(term);
            }
        }
        EffectRow::new(terms)
    }

    fn class_ctor_effect_row(&mut self, class_fqn: &str, ctor_span: Option<Span>) -> EffectRow {
        let mut visiting = std::mem::take(&mut self.visiting_hidden);
        let terms = self.class_ctor_effect_terms(class_fqn, ctor_span, &mut visiting);
        self.visiting_hidden = visiting;
        EffectRow::new(terms)
    }

    fn object_init_effect_row(&mut self, object_fqn: &str) -> EffectRow {
        let mut visiting = std::mem::take(&mut self.visiting_hidden);
        let terms = self.object_init_effect_terms(object_fqn, &mut visiting);
        self.visiting_hidden = visiting;
        EffectRow::new(terms)
    }

    fn top_level_immutable_value_effect_row(&mut self, value_fqn: &str) -> EffectRow {
        let mut visiting = std::mem::take(&mut self.visiting_hidden);
        let terms = self.top_level_immutable_value_effect_terms(value_fqn, &mut visiting);
        self.visiting_hidden = visiting;
        EffectRow::new(terms)
    }

    fn class_ctor_effect_terms(
        &mut self,
        class_fqn: &str,
        ctor_span: Option<Span>,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(class) = self.lookup_class_init(class_fqn) else {
            return Vec::new();
        };
        let key = format!("class:{}:{:?}", class.fqn, ctor_span);
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let mut terms = Vec::new();
        if let Some(super_fqn) = class.super_class_fqn.as_deref() {
            terms.extend(self.class_ctor_effect_terms(super_fqn, None, visiting));
        }
        terms.extend(self.scan_call_args_hidden(
            &class.super_ctor_args,
            class.source_path.as_path(),
            visiting,
        ));

        let selected_ctor = ctor_span
            .and_then(|span| class.ctors.iter().find(|ctor| ctor.span == span))
            .or_else(|| {
                if ctor_span.is_none() && class.ctors.len() == 1 {
                    class.ctors.first()
                } else {
                    None
                }
            });
        if let Some(ctor) = selected_ctor {
            for param in &ctor.params {
                if let Some(default_value) = param.default_value.as_ref() {
                    terms.extend(self.scan_expr_hidden(
                        default_value,
                        class.source_path.as_path(),
                        visiting,
                    ));
                }
            }
            if let Some(delegation) = ctor.delegation.as_ref() {
                terms.extend(self.scan_call_args_hidden(
                    &delegation.args,
                    class.source_path.as_path(),
                    visiting,
                ));
            }
        }

        for step in &class.steps {
            match step {
                ClassInitStep::PropertyInit { init, .. } => {
                    terms.extend(self.scan_expr_hidden(
                        init,
                        class.source_path.as_path(),
                        visiting,
                    ));
                }
                ClassInitStep::InitBlock { block } => {
                    terms.extend(self.scan_block_hidden(
                        block,
                        class.source_path.as_path(),
                        visiting,
                    ));
                }
            }
        }
        if let Some(ctor) = selected_ctor
            && let Some(body) = ctor.body.as_ref()
        {
            terms.extend(self.scan_block_hidden(body, class.source_path.as_path(), visiting));
        }

        visiting.remove(&key);
        terms
    }

    fn object_init_effect_terms(
        &mut self,
        object_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(object_init) = self.builder.lowered_hir.object_inits.get(object_fqn) else {
            return Vec::new();
        };
        let key = format!("object:{object_fqn}");
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let mut terms = Vec::new();
        for step in &object_init.steps {
            match step {
                ObjectInitStep::PropertyInit { init, .. } => {
                    terms.extend(self.scan_expr_hidden(
                        init,
                        object_init.source_path.as_path(),
                        visiting,
                    ));
                }
                ObjectInitStep::InitBlock { block } => {
                    terms.extend(self.scan_block_hidden(
                        block,
                        object_init.source_path.as_path(),
                        visiting,
                    ));
                }
            }
        }

        visiting.remove(&key);
        terms
    }

    fn top_level_immutable_value_effect_terms(
        &mut self,
        value_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(value) = self
            .builder
            .lowered_hir
            .top_level_immutable_values
            .get(value_fqn)
        else {
            return Vec::new();
        };
        let key = format!("top-level-val:{value_fqn}");
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let terms = value
            .init
            .as_ref()
            .map(|init| self.scan_expr_hidden(init, value.source_path.as_path(), visiting))
            .unwrap_or_default();
        visiting.remove(&key);
        terms
    }

    fn lookup_class_init(&self, class_fqn: &str) -> Option<&'b MonoClassInit> {
        self.builder
            .lowered_hir
            .class_inits
            .values()
            .find(|class| class.fqn == class_fqn)
    }

    fn scan_block_hidden(
        &mut self,
        block: &Block,
        source_path: &Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let mut terms = Vec::new();
        for stmt in &block.stmts {
            terms.extend(self.scan_stmt_hidden(stmt, source_path, visiting));
        }
        terms
    }

    fn scan_stmt_hidden(
        &mut self,
        stmt: &Stmt,
        source_path: &Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.scan_expr_hidden(expr, source_path, visiting),
            StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .map(|expr| self.scan_expr_hidden(expr, source_path, visiting))
                .unwrap_or_default(),
            StmtKind::Assign { lhs, rhs, .. } => {
                let mut terms = self.scan_expr_hidden(lhs, source_path, visiting);
                terms.extend(self.scan_expr_hidden(rhs, source_path, visiting));
                terms
            }
            StmtKind::While { cond, body } => {
                let mut terms = self.scan_expr_hidden(cond, source_path, visiting);
                terms.extend(self.scan_block_hidden(body, source_path, visiting));
                terms
            }
            StmtKind::Return { value } => value
                .as_ref()
                .map(|expr| self.scan_expr_hidden(expr, source_path, visiting))
                .unwrap_or_default(),
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => Vec::new(),
        }
    }

    fn scan_expr_hidden(
        &mut self,
        expr: &Expr,
        source_path: &Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        self.expr_surface_row(expr, source_path)
            .terms
            .into_iter()
            .chain(match &expr.kind {
                ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                    let mut terms = self.object_init_effect_terms(fqn, visiting);
                    terms.extend(self.top_level_immutable_value_effect_terms(fqn, visiting));
                    terms
                }
                _ => Vec::new(),
            })
            .collect()
    }

    fn scan_call_args_hidden(
        &mut self,
        args: &[CallArg],
        source_path: &Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let mut terms = Vec::new();
        for arg in args {
            match arg {
                CallArg::Positional(expr) => {
                    terms.extend(self.scan_expr_hidden(expr, source_path, visiting));
                }
                CallArg::Named { value, .. } => {
                    terms.extend(self.scan_expr_hidden(value, source_path, visiting));
                }
            }
        }
        terms
    }
}

fn union_rows(rows: impl IntoIterator<Item = EffectRow>) -> EffectRow {
    EffectRow::new(rows.into_iter().flat_map(|row| row.terms).collect())
}

fn subtract_row(row: EffectRow, handled: &EffectRow) -> EffectRow {
    EffectRow::new(
        row.terms
            .into_iter()
            .filter(|term| !handled.terms.contains(term))
            .collect(),
    )
}

fn stable_effect_param_keys(
    lowered_hir: &LoweredHir,
) -> Result<HashMap<TypeParamType, StableEffectParamKey>, HirStageError> {
    let mut out = HashMap::new();
    for (param, key) in &lowered_hir.stable_type_param_keys {
        if param.decl_file.as_os_str() != EFFECT_ROW_PARAM_DECL_FILE {
            continue;
        }
        let owner = StableDefKey::from_canonical_text(key.owner_def_key()).map_err(|err| {
            hir_stage_effect_template_error(Path::new("<hir>"), param.decl_span, &param.name, err)
        })?;
        let ordinal = u32::try_from(key.index()).unwrap_or(u32::MAX);
        out.insert(
            param.clone(),
            StableEffectParamKey::new(owner, ordinal, param.name.clone()),
        );
    }
    Ok(out)
}

fn effect_row_template_fact(
    template: &StableEffectRowTemplate,
) -> hir_site_facts::EffectRowTemplate {
    hir_site_facts::EffectRowTemplate {
        terms: template
            .terms()
            .iter()
            .map(|term| match term {
                StableEffectTerm::Concrete { type_key } => {
                    hir_site_facts::EffectRowTerm::Concrete {
                        type_key: type_key.clone(),
                    }
                }
                StableEffectTerm::Param {
                    owner,
                    ordinal,
                    name,
                } => hir_site_facts::EffectRowTerm::Param {
                    owner: CanonicalTextKey::new(owner.canonical_text()),
                    ordinal: *ordinal,
                    name: name.clone(),
                },
            })
            .collect(),
        closed: template.closed(),
    }
}

fn hir_stage_effect_template_error(
    source_path: &Path,
    span: Span,
    owner: &str,
    err: CanonicalEncodingError,
) -> HirStageError {
    HirStageError::new(
        source_path.to_path_buf(),
        span,
        format!("failed to publish HIR effect row template for `{owner}`: {err}"),
        owner,
    )
}

fn semantic_operation_kind(
    contract: &TypedCallSiteContract,
) -> hir_site_facts::CanonicalSemanticOperationKind {
    match contract {
        TypedCallSiteContract::DirectTopLevel(function) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::DirectTopLevel,
                target: function_target_key(function),
            }
        }
        TypedCallSiteContract::MemberDirect(member) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::MemberDirect,
                target: function_target_key(member.function()),
            }
        }
        TypedCallSiteContract::Extension { function, .. } => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::Extension,
                target: function_target_key(function),
            }
        }
        TypedCallSiteContract::Constructor(ctor) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreConstructorInit {
                owner_fqn: ctor.owner_fqn().to_string(),
                ctor_span: ctor.ctor_span(),
            }
        }
        TypedCallSiteContract::Closure { .. } => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::Closure,
                target: None,
            }
        }
        TypedCallSiteContract::FunValue { .. } => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::FunValue,
                target: None,
            }
        }
        TypedCallSiteContract::FunPtr { .. } => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::FunPtr,
                target: None,
            }
        }
        TypedCallSiteContract::Virtual(member) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::VirtualDispatch,
                target: function_target_key(member.function()),
            }
        }
        TypedCallSiteContract::Interface(member) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::InterfaceDispatch,
                target: function_target_key(member.function()),
            }
        }
        TypedCallSiteContract::Intrinsic { function, .. } => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreCall {
                call_kind: hir_site_facts::CallSiteKind::Intrinsic,
                target: function_target_key(function),
            }
        }
        TypedCallSiteContract::EffectOp(perform) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreEffectOperation {
                effect_ty: perform.effect_ty(),
                op_fqn: perform.op_fqn().to_string(),
            }
        }
        TypedCallSiteContract::ContinuationResume(_) => {
            hir_site_facts::CanonicalSemanticOperationKind::CoreContinuationResume
        }
    }
}

fn function_target_key(target: &FunctionTargetContract) -> Option<CanonicalTextKey> {
    target
        .stable_instance_key()
        .map(|key| CanonicalTextKey::new(key.canonical_text()))
        .or_else(|| {
            target
                .stable_template_key()
                .map(|key| CanonicalTextKey::new(key.canonical_text()))
        })
        .or_else(|| Some(CanonicalTextKey::new(target.fqn())))
}

fn source_site_identity(call_site: &CallSite, role: &str) -> hir_site_facts::SourceSiteIdentity {
    source_site_identity_from_parts(&call_site.source_path, call_site.span, role)
}

fn source_site_identity_from_parts(
    source_path: &Path,
    span: Span,
    role: &str,
) -> hir_site_facts::SourceSiteIdentity {
    let owner = CanonicalTextKey::new(format!("source:{}", source_path.display()));
    let site_key = format!(
        "{}:{}:{}..{}",
        role,
        source_path.display(),
        span.start,
        span.end
    );
    hir_site_facts::SourceSiteIdentity::new(
        owner,
        SiteId::from_raw(stable_hash64(StableHashScope::DumpV0, &site_key) as u32),
        source_path.to_path_buf(),
        span,
    )
}

fn function_effect_fact(
    contract: &FunctionEffectContract,
) -> hir_site_facts::FunctionEffectContract {
    hir_site_facts::FunctionEffectContract {
        fqn: contract.fqn.clone(),
        source_path: contract.source_path.clone(),
        span: contract.span,
        return_ty: contract.return_ty,
        allowed_effects: contract.allowed_effects.clone(),
        effects_closed: contract.effects_closed,
    }
}

fn call_site_contract_fact(
    call_site: &CallSite,
    contract: &TypedCallSiteContract,
) -> hir_site_facts::CallSiteContract {
    let contract = call_site_contract_kind_fact(call_site, contract);
    let kind = match &contract {
        hir_site_facts::CallSiteContractKind::DirectTopLevel(_) => {
            hir_site_facts::CallSiteKind::DirectTopLevel
        }
        hir_site_facts::CallSiteContractKind::MemberDirect(_) => {
            hir_site_facts::CallSiteKind::MemberDirect
        }
        hir_site_facts::CallSiteContractKind::Extension { .. } => {
            hir_site_facts::CallSiteKind::Extension
        }
        hir_site_facts::CallSiteContractKind::Constructor(_) => {
            hir_site_facts::CallSiteKind::Constructor
        }
        hir_site_facts::CallSiteContractKind::Closure { .. } => {
            hir_site_facts::CallSiteKind::Closure
        }
        hir_site_facts::CallSiteContractKind::FunValue { .. } => {
            hir_site_facts::CallSiteKind::FunValue
        }
        hir_site_facts::CallSiteContractKind::FunPtr { .. } => hir_site_facts::CallSiteKind::FunPtr,
        hir_site_facts::CallSiteContractKind::Virtual(_) => {
            hir_site_facts::CallSiteKind::VirtualDispatch
        }
        hir_site_facts::CallSiteContractKind::Interface(_) => {
            hir_site_facts::CallSiteKind::InterfaceDispatch
        }
        hir_site_facts::CallSiteContractKind::Intrinsic { .. } => {
            hir_site_facts::CallSiteKind::Intrinsic
        }
        hir_site_facts::CallSiteContractKind::EffectOp(_) => {
            hir_site_facts::CallSiteKind::EffectOperation
        }
        hir_site_facts::CallSiteContractKind::ContinuationResume(_) => {
            hir_site_facts::CallSiteKind::ContinuationResume
        }
    };
    hir_site_facts::CallSiteContract {
        identity: source_site_identity(call_site, "call"),
        kind,
        contract,
    }
}

fn call_site_contract_kind_fact(
    call_site: &CallSite,
    contract: &TypedCallSiteContract,
) -> hir_site_facts::CallSiteContractKind {
    match contract {
        TypedCallSiteContract::DirectTopLevel(function) => {
            hir_site_facts::CallSiteContractKind::DirectTopLevel(function_target_fact(function))
        }
        TypedCallSiteContract::MemberDirect(member) => {
            hir_site_facts::CallSiteContractKind::MemberDirect(member_call_target_fact(member))
        }
        TypedCallSiteContract::Extension {
            receiver_ty,
            function,
        } => hir_site_facts::CallSiteContractKind::Extension {
            receiver_ty: *receiver_ty,
            function: function_target_fact(function),
        },
        TypedCallSiteContract::Constructor(ctor) => {
            hir_site_facts::CallSiteContractKind::Constructor(constructor_call_target_fact(ctor))
        }
        TypedCallSiteContract::Closure {
            callee_ty,
            return_ty,
            abi_identity,
            arg_binding,
        } => hir_site_facts::CallSiteContractKind::Closure {
            callee_ty: *callee_ty,
            return_ty: *return_ty,
            abi: callable_abi_fact(*abi_identity),
            arg_binding: arg_binding.as_ref().map(call_arg_binding_fact),
        },
        TypedCallSiteContract::FunValue {
            callee_ty,
            return_ty,
            abi_identity,
            arg_binding,
        } => hir_site_facts::CallSiteContractKind::FunValue {
            callee_ty: *callee_ty,
            return_ty: *return_ty,
            abi: callable_abi_fact(*abi_identity),
            arg_binding: arg_binding.as_ref().map(call_arg_binding_fact),
        },
        TypedCallSiteContract::FunPtr {
            callee_ty,
            return_ty,
            abi_identity,
            arg_binding,
        } => hir_site_facts::CallSiteContractKind::FunPtr {
            callee_ty: *callee_ty,
            return_ty: *return_ty,
            abi: callable_abi_fact(*abi_identity),
            arg_binding: arg_binding.as_ref().map(call_arg_binding_fact),
        },
        TypedCallSiteContract::Virtual(member) => {
            hir_site_facts::CallSiteContractKind::Virtual(member_call_target_fact(member))
        }
        TypedCallSiteContract::Interface(member) => {
            hir_site_facts::CallSiteContractKind::Interface(member_call_target_fact(member))
        }
        TypedCallSiteContract::Intrinsic { kind, function } => {
            hir_site_facts::CallSiteContractKind::Intrinsic {
                kind: intrinsic_kind_fact(kind),
                function: function_target_fact(function),
            }
        }
        TypedCallSiteContract::EffectOp(perform) => {
            hir_site_facts::CallSiteContractKind::EffectOp(perform_site_fact(call_site, perform))
        }
        TypedCallSiteContract::ContinuationResume(resume) => {
            hir_site_facts::CallSiteContractKind::ContinuationResume(continuation_resume_fact(
                call_site, resume,
            ))
        }
    }
}

fn call_site_contract_arg_binding(
    contract: &TypedCallSiteContract,
) -> Option<&CallArgBindingContract> {
    match contract {
        TypedCallSiteContract::DirectTopLevel(function)
        | TypedCallSiteContract::Intrinsic { function, .. } => function.arg_binding(),
        TypedCallSiteContract::MemberDirect(member)
        | TypedCallSiteContract::Virtual(member)
        | TypedCallSiteContract::Interface(member) => member.function().arg_binding(),
        TypedCallSiteContract::Extension { function, .. } => function.arg_binding(),
        TypedCallSiteContract::Closure { arg_binding, .. }
        | TypedCallSiteContract::FunValue { arg_binding, .. }
        | TypedCallSiteContract::FunPtr { arg_binding, .. } => arg_binding.as_ref(),
        TypedCallSiteContract::Constructor(_)
        | TypedCallSiteContract::EffectOp(_)
        | TypedCallSiteContract::ContinuationResume(_) => None,
    }
}

fn call_site_instance_fact(
    call_site: &CallSite,
    contract: &TypedCallSiteContract,
) -> Option<hir_site_facts::CallSiteInstanceFact> {
    let function = function_target_from_call_contract(contract)?;
    let template_key = function.stable_template_key.as_ref()?;
    let stable_instance_key = function.stable_instance_key.as_ref()?;
    Some(hir_site_facts::CallSiteInstanceFact {
        identity: source_site_identity(call_site, "call_instance"),
        template_key: CanonicalTextKey::new(template_key.canonical_text()),
        stable_instance_key: CanonicalTextKey::new(stable_instance_key.canonical_text()),
        type_args: function.type_args.clone(),
        eff_args: function.eff_args.clone(),
    })
}

fn template_site_binding_fact(
    call_site: &CallSite,
    contract: &TypedCallSiteContract,
) -> Option<hir_site_facts::TemplateSiteBindingFact> {
    let function = function_target_from_call_contract(contract)?;
    if function.stable_instance_key.is_some()
        || (function.type_args.is_empty() && function.eff_args.is_empty())
    {
        return None;
    }
    let template_key = function.stable_template_key.as_ref()?;
    Some(hir_site_facts::TemplateSiteBindingFact {
        identity: source_site_identity(call_site, "template_call_binding"),
        kind: hir_site_facts::TemplateSiteBindingKind::DirectCall,
        template_key: CanonicalTextKey::new(template_key.canonical_text()),
        type_args: function.type_args.clone(),
        eff_args: function.eff_args.clone(),
    })
}

fn template_value_binding_facts(
    lowered_hir: &LoweredHir,
) -> Vec<hir_site_facts::TemplateSiteBindingFact> {
    let templates_by_request = lowered_hir
        .generic_template_inventory
        .iter()
        .map(|info| {
            (
                info.request_lookup_key.clone(),
                info.stable_template_key.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut out = Vec::new();
    for (site, binding) in &lowered_hir.top_level_fun_value_refs {
        if binding.type_args.is_empty() && binding.eff_args.is_empty() {
            continue;
        }
        let lookup = (
            binding.fqn.clone(),
            binding.decl_file.clone(),
            binding.decl_span,
        );
        let Some(template_key) = templates_by_request.get(&lookup) else {
            continue;
        };
        out.push(hir_site_facts::TemplateSiteBindingFact {
            identity: source_site_identity(site, "template_value_binding"),
            kind: hir_site_facts::TemplateSiteBindingKind::FunValue,
            template_key: CanonicalTextKey::new(template_key.canonical_text()),
            type_args: binding.type_args.clone(),
            eff_args: binding.eff_args.clone(),
        });
    }
    out
}

fn dispatch_candidate_fact(
    lowered_hir: &LoweredHir,
    call_site: &CallSite,
    contract: &TypedCallSiteContract,
) -> Option<hir_site_facts::DispatchCandidateFact> {
    let (dispatch_kind, member) = match contract {
        TypedCallSiteContract::Virtual(member) => {
            (hir_site_facts::CallSiteKind::VirtualDispatch, member)
        }
        TypedCallSiteContract::Interface(member) => {
            (hir_site_facts::CallSiteKind::InterfaceDispatch, member)
        }
        _ => return None,
    };
    let explicit_arg_count = dispatch_explicit_arg_count(lowered_hir, member);
    let candidate_fqns = match dispatch_kind {
        hir_site_facts::CallSiteKind::VirtualDispatch => virtual_dispatch_candidate_fqns(
            lowered_hir,
            member.receiver_ty,
            member.member_name(),
            explicit_arg_count,
        ),
        hir_site_facts::CallSiteKind::InterfaceDispatch => interface_dispatch_candidate_fqns(
            lowered_hir,
            member.receiver_ty,
            member.owner_fqn(),
            member.member_name(),
            explicit_arg_count,
        ),
        _ => Vec::new(),
    };
    let member_has_stable_identity = member.function.stable_template_key.as_ref().is_some();
    let member_template_fqn = generic_template_base_fqn(member.function.fqn());
    let mut stable_instance_keys = candidate_fqns
        .iter()
        .filter(|fqn| {
            !member_has_stable_identity || generic_template_base_fqn(fqn) != member_template_fqn
        })
        .flat_map(|fqn| dispatch_candidate_stable_instance_keys(lowered_hir, fqn))
        .map(|key| CanonicalTextKey::new(key.canonical_text()))
        .collect::<Vec<_>>();
    if let Some(stable_instance_key) = member.function.stable_instance_key.as_ref() {
        stable_instance_keys.push(CanonicalTextKey::new(stable_instance_key.canonical_text()));
    }
    if let Some(stable_template_key) = member.function.stable_template_key.as_ref()
        && instance_args_are_concrete(
            &lowered_hir.types,
            member.function.type_args(),
            member.function.eff_args(),
        )
        && let Ok(stable_instance_key) = StableInstanceKey::from_type_arguments(
            stable_template_key.clone(),
            &lowered_hir.types,
            member.function.type_args(),
            member.function.eff_args(),
            &NoTypeParamResolver,
        )
    {
        stable_instance_keys.push(CanonicalTextKey::new(stable_instance_key.canonical_text()));
    }
    stable_instance_keys.sort_by(|lhs, rhs| lhs.as_str().cmp(rhs.as_str()));
    stable_instance_keys.dedup();
    Some(hir_site_facts::DispatchCandidateFact {
        identity: source_site_identity(call_site, "dispatch_candidate"),
        dispatch_kind,
        receiver_ty: member.receiver_ty,
        stable_instance_keys,
    })
}

fn dispatch_explicit_arg_count(
    lowered_hir: &LoweredHir,
    member: &MemberCallTargetContract,
) -> usize {
    let param_tys = member.function.param_tys();
    if param_tys.first().is_some_and(|&param_ty| {
        same_nominal_type(&lowered_hir.types, param_ty, member.receiver_ty())
    }) {
        param_tys.len().saturating_sub(1)
    } else {
        param_tys.len()
    }
}

fn virtual_dispatch_candidate_fqns(
    lowered_hir: &LoweredHir,
    receiver_ty: TypeId,
    member_name: &str,
    explicit_arg_count: usize,
) -> Vec<String> {
    let Some(receiver_fqn) = nominal_type_fqn(&lowered_hir.types, receiver_ty) else {
        return Vec::new();
    };
    let mut targets = BTreeSet::new();
    for class_fqn in descendants_and_self(lowered_hir, receiver_fqn) {
        if let Some(slot) = lowered_hir
            .class_vtables
            .get(class_fqn.as_str())
            .and_then(|slots| {
                slots.iter().find(|slot| {
                    slot.name == member_name && slot.params_len == explicit_arg_count as u32
                })
            })
        {
            targets.insert(slot.impl_member_fqn.clone());
        } else if class_fqn == receiver_fqn {
            targets.insert(format!("{class_fqn}.{member_name}"));
        }
    }
    targets.into_iter().collect()
}

fn interface_dispatch_candidate_fqns(
    lowered_hir: &LoweredHir,
    receiver_ty: TypeId,
    owner_fqn: &str,
    member_name: &str,
    explicit_arg_count: usize,
) -> Vec<String> {
    let Some(interface) = lowered_hir.interfaces.get(owner_fqn) else {
        return Vec::new();
    };
    let mut matching_slots = interface
        .method_slots
        .iter()
        .filter(|slot| slot.name == member_name && slot.params_len == explicit_arg_count as u32);
    let Some(slot) = matching_slots.next() else {
        return Vec::new();
    };
    if matching_slots.next().is_some() {
        return Vec::new();
    }

    let mut targets = BTreeSet::new();
    if let Some(receiver_fqn) = nominal_type_fqn(&lowered_hir.types, receiver_ty)
        && let Some(entries) = lowered_hir.class_itables.get(receiver_fqn)
    {
        collect_interface_slot_targets(entries, owner_fqn, slot.slot as usize, &mut targets);
    }
    if targets.is_empty() {
        for entries in lowered_hir.class_itables.values() {
            collect_interface_slot_targets(entries, owner_fqn, slot.slot as usize, &mut targets);
        }
    }
    targets.into_iter().collect()
}

fn collect_interface_slot_targets(
    entries: &[crate::itable::ClassItableEntry],
    owner_fqn: &str,
    slot: usize,
    targets: &mut BTreeSet<String>,
) {
    for entry in entries
        .iter()
        .filter(|entry| entry.interface_fqn == owner_fqn)
    {
        if let Some(target) = entry.method_impl_fqns.get(slot)
            && !target.is_empty()
        {
            targets.insert(target.clone());
        }
    }
}

fn dispatch_candidate_stable_instance_keys(
    lowered_hir: &LoweredHir,
    candidate_fqn: &str,
) -> Vec<StableInstanceKey> {
    let template_fqn = generic_template_base_fqn(candidate_fqn);
    let Some((owner_fqn, _)) = template_fqn.rsplit_once('.') else {
        return Vec::new();
    };
    let templates = lowered_hir
        .generic_template_inventory
        .iter()
        .filter(|info| info.template.fqn == template_fqn)
        .collect::<Vec<_>>();
    let mut out = Vec::new();
    for template in templates {
        if !template.function_type_param_names.is_empty()
            || template.function_eff_param_name.is_some()
        {
            continue;
        }
        for nominal in concrete_nominal_instances_for_owner(lowered_hir, owner_fqn) {
            if nominal.args.len() != template.owner_type_param_names.len() {
                continue;
            }
            let eff_args = match (&template.owner_eff_param_name, nominal.eff.as_ref()) {
                (Some(_), Some(row)) => vec![row.clone()],
                (Some(_), None) => continue,
                (None, _) => Vec::new(),
            };
            if !instance_args_are_concrete(&lowered_hir.types, &nominal.args, &eff_args) {
                continue;
            }
            if let Ok(stable_key) = StableInstanceKey::from_type_arguments(
                template.stable_template_key.clone(),
                &lowered_hir.types,
                &nominal.args,
                &eff_args,
                &NoTypeParamResolver,
            ) {
                out.push(stable_key);
            }
        }
    }
    out.sort_by_key(StableInstanceKey::canonical_text);
    out.dedup();
    out
}

fn concrete_nominal_instances_for_owner(
    lowered_hir: &LoweredHir,
    owner_fqn: &str,
) -> Vec<NominalType> {
    lowered_hir
        .types
        .iter_ids()
        .filter_map(|ty| match lowered_hir.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == owner_fqn =>
            {
                Some(nominal.clone())
            }
            _ => None,
        })
        .collect()
}

fn descendants_and_self(lowered_hir: &LoweredHir, root: &str) -> BTreeSet<String> {
    let mut direct_subclasses = HashMap::<String, BTreeSet<String>>::new();
    for (child, supers) in &lowered_hir.direct_supertypes {
        for super_fqn in supers {
            direct_subclasses
                .entry(super_fqn.clone())
                .or_default()
                .insert(child.clone());
        }
    }
    let mut seen = BTreeSet::from([root.to_string()]);
    let mut stack = vec![root.to_string()];
    while let Some(current) = stack.pop() {
        if let Some(children) = direct_subclasses.get(&current) {
            for child in children {
                if seen.insert(child.clone()) {
                    stack.push(child.clone());
                }
            }
        }
    }
    seen
}

fn same_nominal_type(types: &TypeStore, lhs: TypeId, rhs: TypeId) -> bool {
    lhs == rhs || nominal_type_fqn(types, lhs) == nominal_type_fqn(types, rhs)
}

fn nominal_type_fqn(types: &TypeStore, ty: TypeId) -> Option<&str> {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.as_str()),
        _ => None,
    }
}

fn function_target_from_call_contract(
    contract: &TypedCallSiteContract,
) -> Option<&FunctionTargetContract> {
    match contract {
        TypedCallSiteContract::DirectTopLevel(function)
        | TypedCallSiteContract::Intrinsic { function, .. } => Some(function),
        TypedCallSiteContract::MemberDirect(member)
        | TypedCallSiteContract::Virtual(member)
        | TypedCallSiteContract::Interface(member) => Some(member.function()),
        TypedCallSiteContract::Extension { function, .. } => Some(function),
        TypedCallSiteContract::Constructor(_)
        | TypedCallSiteContract::Closure { .. }
        | TypedCallSiteContract::FunValue { .. }
        | TypedCallSiteContract::FunPtr { .. }
        | TypedCallSiteContract::EffectOp(_)
        | TypedCallSiteContract::ContinuationResume(_) => None,
    }
}

fn function_target_fact(function: &FunctionTargetContract) -> hir_site_facts::FunctionTarget {
    hir_site_facts::FunctionTarget {
        fqn: function.fqn.clone(),
        decl_file: function.decl_file.clone(),
        decl_span: function.decl_span,
        abi: callable_abi_fact(function.abi_identity),
        stable_template_key: function
            .stable_template_key
            .as_ref()
            .map(|key| CanonicalTextKey::new(key.canonical_text())),
        stable_instance_key: function
            .stable_instance_key
            .as_ref()
            .map(|key| CanonicalTextKey::new(key.canonical_text())),
        param_tys: function.param_tys.clone(),
        return_ty: function.return_ty,
        type_args: function.type_args.clone(),
        eff_args: function.eff_args.clone(),
        arg_binding: function.arg_binding.as_ref().map(call_arg_binding_fact),
    }
}

fn member_call_target_fact(member: &MemberCallTargetContract) -> hir_site_facts::MemberCallTarget {
    hir_site_facts::MemberCallTarget {
        owner_fqn: member.owner_fqn.clone(),
        member_name: member.member_name.clone(),
        member_fqn: member.member_fqn.clone(),
        receiver_ty: member.receiver_ty,
        function: function_target_fact(&member.function),
    }
}

fn constructor_call_target_fact(
    ctor: &ConstructorCallTargetContract,
) -> hir_site_facts::ConstructorCallTarget {
    hir_site_facts::ConstructorCallTarget {
        owner_fqn: ctor.owner_fqn.clone(),
        ctor_span: ctor.ctor_span,
        result_ty: ctor.result_ty,
        arg_mapping: ctor.arg_mapping.clone(),
    }
}

fn callable_abi_fact(abi: CallableAbiIdentity) -> hir_site_facts::CallableAbi {
    match abi {
        CallableAbiIdentity::ManagedOrdinary => hir_site_facts::CallableAbi::ManagedOrdinary,
        CallableAbiIdentity::NativeExtern => hir_site_facts::CallableAbi::NativeExtern,
        CallableAbiIdentity::ManagedExtern => hir_site_facts::CallableAbi::ManagedExtern,
        CallableAbiIdentity::EffectBridge => hir_site_facts::CallableAbi::EffectBridge,
    }
}

fn intrinsic_kind_fact(kind: &TypedIntrinsicKind) -> hir_site_facts::IntrinsicKind {
    match kind {
        TypedIntrinsicKind::Reflection { name } => {
            hir_site_facts::IntrinsicKind::Reflection { name: name.clone() }
        }
        TypedIntrinsicKind::Platform { name } => {
            hir_site_facts::IntrinsicKind::Platform { name: name.clone() }
        }
        TypedIntrinsicKind::Gc { name } => hir_site_facts::IntrinsicKind::Gc { name: name.clone() },
        TypedIntrinsicKind::Runtime { name } => {
            hir_site_facts::IntrinsicKind::Runtime { name: name.clone() }
        }
        TypedIntrinsicKind::Compiler { name } => {
            hir_site_facts::IntrinsicKind::Compiler { name: name.clone() }
        }
        TypedIntrinsicKind::NamedTable {
            entry_name,
            uses_runtime_call,
        } => hir_site_facts::IntrinsicKind::NamedTable {
            entry_name: entry_name.clone(),
            uses_runtime_call: *uses_runtime_call,
        },
    }
}

fn call_arg_binding_fact(
    binding: &CallArgBindingContract,
) -> hir_site_facts::CallArgBindingContract {
    hir_site_facts::CallArgBindingContract {
        params: binding.params.iter().map(call_arg_param_fact).collect(),
    }
}

fn call_arg_param_fact(param: &CallArgParamContract) -> hir_site_facts::CallArgParamContract {
    match param {
        CallArgParamContract::Receiver => hir_site_facts::CallArgParamContract::Receiver,
        CallArgParamContract::Explicit(element) => {
            hir_site_facts::CallArgParamContract::Explicit(call_arg_element_fact(element))
        }
        CallArgParamContract::Default => hir_site_facts::CallArgParamContract::Default,
        CallArgParamContract::Vararg(elements) => hir_site_facts::CallArgParamContract::Vararg(
            elements.iter().map(call_arg_element_fact).collect(),
        ),
    }
}

fn call_arg_element_fact(
    element: &CallArgElementContract,
) -> hir_site_facts::CallArgElementContract {
    hir_site_facts::CallArgElementContract {
        arg_index: element.arg_index,
        spread: element.spread,
    }
}

fn perform_site_fact(
    call_site: &CallSite,
    contract: &PerformSiteContract,
) -> hir_site_facts::PerformSiteContract {
    hir_site_facts::PerformSiteContract {
        identity: source_site_identity(call_site, "perform"),
        effect_ty: contract.effect_ty,
        op_fqn: contract.op_fqn.clone(),
        result_ty: contract.result_ty,
        payload: payload_fact(&contract.payload),
        arg_mapping: contract.arg_mapping.clone(),
    }
}

fn handle_site_fact(
    call_site: &CallSite,
    contract: &HandleSiteContract,
) -> hir_site_facts::HandleSiteContract {
    hir_site_facts::HandleSiteContract {
        identity: source_site_identity(call_site, "handle"),
        result_ty: contract.result_ty,
        body_result_ty: contract.body_result_ty,
        arm_contracts: contract.arm_contracts.iter().map(handle_arm_fact).collect(),
        finally_result_ty: contract.finally_result_ty,
    }
}

fn handle_arm_fact(arm: &HandleArmSiteContract) -> hir_site_facts::HandleArmSiteContract {
    hir_site_facts::HandleArmSiteContract {
        handled_effect_ty: arm.handled_effect_ty,
        op_fqn: arm.op_fqn.clone(),
        payload: payload_fact(&arm.payload),
        body_ty: arm.body_ty,
        kind: match arm.kind {
            HandleArmContractKind::NonResuming => {
                hir_site_facts::HandleArmContractKind::NonResuming
            }
            HandleArmContractKind::EscapeContinuation => {
                hir_site_facts::HandleArmContractKind::EscapeContinuation
            }
        },
    }
}

fn payload_fact(payload: &PayloadTypeContract) -> hir_site_facts::PayloadTypeContract {
    hir_site_facts::PayloadTypeContract {
        ty: payload.ty,
        components: payload.components.clone(),
    }
}

fn continuation_resume_fact(
    call_site: &CallSite,
    contract: &ContinuationResumeSiteContract,
) -> hir_site_facts::ContinuationResumeContract {
    hir_site_facts::ContinuationResumeContract {
        identity: source_site_identity(call_site, "resume"),
        receiver_route: match contract.receiver_route {
            ContinuationResumeReceiverRoute::CallArg { index } => {
                hir_site_facts::ContinuationResumeReceiverRoute::CallArg { index }
            }
            ContinuationResumeReceiverRoute::MemberReceiver => {
                hir_site_facts::ContinuationResumeReceiverRoute::MemberReceiver
            }
        },
        payload_arg_indices: contract.payload_arg_indices.clone(),
        receiver_ty: contract.receiver_ty,
        resume_ty: contract.resume_ty,
        answer_ty: contract.answer_ty,
        return_ty: contract.return_ty,
        out_effects: contract.out_effects.clone(),
        runtime_error_effect_ty: contract.runtime_error_effect_ty,
    }
}

fn assignment_contract_fact(
    call_site: &CallSite,
    contract: &AssignPlaceContract,
) -> hir_site_facts::AssignmentContract {
    hir_site_facts::AssignmentContract {
        identity: source_site_identity(call_site, "assignment"),
        span: contract.span,
        kind: assign_place_kind_fact(&contract.kind),
        place_ty: contract.place_ty,
        value_ty: contract.value_ty,
        mutable: contract.mutable,
        write_barrier: assign_write_barrier_fact(&contract.write_barrier),
        unsafe_required: contract.unsafe_required,
    }
}

fn assign_place_kind_fact(kind: &AssignPlaceKind) -> hir_site_facts::AssignPlaceKind {
    match kind {
        AssignPlaceKind::Local {
            id,
            name,
            decl_span,
        } => hir_site_facts::AssignPlaceKind::Local {
            symbol_id: id.as_u32(),
            name: name.clone(),
            decl_span: *decl_span,
        },
        AssignPlaceKind::TopLevel { id, fqn } => hir_site_facts::AssignPlaceKind::TopLevel {
            symbol_id: id.as_u32(),
            fqn: fqn.clone(),
        },
        AssignPlaceKind::Member {
            receiver_ty,
            owner_fqn,
            member_fqn,
            member_name,
            member_span,
            resolved,
        } => hir_site_facts::AssignPlaceKind::Member {
            receiver_ty: *receiver_ty,
            owner_fqn: owner_fqn.clone(),
            member_fqn: member_fqn.clone(),
            member_name: member_name.clone(),
            member_span: *member_span,
            resolved: resolved.as_ref().map(member_ref_fact),
        },
    }
}

fn member_ref_fact(member: &crate::hir::MemberRef) -> hir_site_facts::MemberRef {
    match member {
        crate::hir::MemberRef::Value { id, fqn } => hir_site_facts::MemberRef::Value {
            symbol_id: id.as_u32(),
            fqn: fqn.clone(),
        },
        crate::hir::MemberRef::Fun { id, fqn } => hir_site_facts::MemberRef::Fun {
            symbol_id: id.as_u32(),
            fqn: fqn.clone(),
        },
        crate::hir::MemberRef::ExtensionValue { id, fqn } => {
            hir_site_facts::MemberRef::ExtensionValue {
                symbol_id: id.as_u32(),
                fqn: fqn.clone(),
            }
        }
        crate::hir::MemberRef::ExtensionFun { id, fqn } => {
            hir_site_facts::MemberRef::ExtensionFun {
                symbol_id: id.as_u32(),
                fqn: fqn.clone(),
            }
        }
    }
}

fn assign_write_barrier_fact(
    barrier: &ast::AssignWriteBarrierRequirement,
) -> hir_site_facts::AssignWriteBarrierRequirement {
    match barrier {
        ast::AssignWriteBarrierRequirement::NotRequired => {
            hir_site_facts::AssignWriteBarrierRequirement::NotRequired
        }
        ast::AssignWriteBarrierRequirement::StorageSlot { slot_ty } => {
            hir_site_facts::AssignWriteBarrierRequirement::StorageSlot { slot_ty: *slot_ty }
        }
    }
}

fn with_update_contract_fact(
    call_site: &CallSite,
    contract: &ast::WithUpdateContract,
) -> hir_site_facts::WithUpdateContract {
    hir_site_facts::WithUpdateContract {
        identity: source_site_identity(call_site, "with_update"),
        base_ty: contract.base_ty,
        result_ty: contract.result_ty,
        aggregates: contract
            .aggregates
            .iter()
            .map(with_update_aggregate_fact)
            .collect(),
        updates: contract
            .updates
            .iter()
            .map(with_update_update_fact)
            .collect(),
    }
}

fn with_update_aggregate_fact(
    aggregate: &ast::WithUpdateAggregateContract,
) -> hir_site_facts::WithUpdateAggregateContract {
    hir_site_facts::WithUpdateAggregateContract {
        prefix: aggregate.prefix.clone(),
        ty: aggregate.ty,
        kind: match &aggregate.kind {
            ast::WithUpdateAggregateContractKind::Struct { fqn, fields } => {
                hir_site_facts::WithUpdateAggregateContractKind::Struct {
                    fqn: fqn.clone(),
                    fields: fields
                        .iter()
                        .map(|field| hir_site_facts::WithUpdateAggregateFieldContract {
                            name: field.name.clone(),
                            ty: field.ty,
                        })
                        .collect(),
                }
            }
            ast::WithUpdateAggregateContractKind::Tuple { elements } => {
                hir_site_facts::WithUpdateAggregateContractKind::Tuple {
                    elements: elements.clone(),
                }
            }
            ast::WithUpdateAggregateContractKind::Enum { info } => {
                hir_site_facts::WithUpdateAggregateContractKind::Enum {
                    info: with_update_resolved_enum_fact(info),
                }
            }
        },
    }
}

fn with_update_update_fact(
    update: &ast::WithUpdateUpdateContract,
) -> hir_site_facts::WithUpdateUpdateContract {
    hir_site_facts::WithUpdateUpdateContract {
        path: update.path.clone(),
        target_ty: update.target_ty,
        value_ty: update.value_ty,
        segments: update
            .segments
            .iter()
            .map(with_update_path_segment_fact)
            .collect(),
    }
}

fn with_update_path_segment_fact(
    segment: &ast::WithUpdatePathSegmentContract,
) -> hir_site_facts::WithUpdatePathSegmentContract {
    hir_site_facts::WithUpdatePathSegmentContract {
        aggregate_prefix: segment.aggregate_prefix.clone(),
        aggregate_ty: segment.aggregate_ty,
        field_ty: segment.field_ty,
        kind: match &segment.kind {
            ast::WithUpdatePathSegmentKind::StructField { owner_fqn, field } => {
                hir_site_facts::WithUpdatePathSegmentKind::StructField {
                    owner_fqn: owner_fqn.clone(),
                    field: field.clone(),
                }
            }
            ast::WithUpdatePathSegmentKind::TupleElement { index } => {
                hir_site_facts::WithUpdatePathSegmentKind::TupleElement { index: *index }
            }
            ast::WithUpdatePathSegmentKind::EnumVariantField {
                enum_fqn,
                variant,
                field,
            } => hir_site_facts::WithUpdatePathSegmentKind::EnumVariantField {
                enum_fqn: enum_fqn.clone(),
                variant: variant.clone(),
                field: field.clone(),
            },
        },
    }
}

fn with_update_resolved_enum_fact(
    info: &ast::WithUpdateResolvedEnum,
) -> hir_site_facts::WithUpdateResolvedEnum {
    hir_site_facts::WithUpdateResolvedEnum {
        enum_fqn: info.enum_fqn.clone(),
        variants: info
            .variants
            .iter()
            .map(|variant| hir_site_facts::WithUpdateResolvedEnumVariant {
                name: variant.name.clone(),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| hir_site_facts::WithUpdateResolvedEnumField {
                        name: field.name.clone(),
                        ty: field.ty,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn top_level_init_root_fact(
    root: &TopLevelInitRootContract,
) -> hir_site_facts::TopLevelInitRootContract {
    hir_site_facts::TopLevelInitRootContract {
        fqn: root.fqn.clone(),
        source_path: root.source_path.clone(),
        span: root.span,
        kind: match root.kind {
            TopLevelInitRootKind::RuntimeImmutableVal => {
                hir_site_facts::TopLevelInitRootKind::RuntimeImmutableVal
            }
            TopLevelInitRootKind::RuntimeMutableVar { storage } => {
                hir_site_facts::TopLevelInitRootKind::RuntimeMutableVar {
                    storage: global_storage_policy(storage),
                }
            }
            TopLevelInitRootKind::ObjectSingleton => {
                hir_site_facts::TopLevelInitRootKind::ObjectSingleton
            }
        },
        ty: root.ty,
        initializer_ty: root.initializer_ty,
        has_initializer: root.has_initializer,
        dependencies: root
            .dependencies
            .iter()
            .map(|dependency| hir_site_facts::TopLevelInitDependency {
                fqn: dependency.fqn.clone(),
                kind: match dependency.kind {
                    TopLevelInitDependencyKind::TopLevelValue => {
                        hir_site_facts::TopLevelInitDependencyKind::TopLevelValue
                    }
                    TopLevelInitDependencyKind::ObjectSingleton => {
                        hir_site_facts::TopLevelInitDependencyKind::ObjectSingleton
                    }
                },
            })
            .collect(),
    }
}

fn extern_global_contract_fact(
    contract: &ExternGlobalContract,
) -> hir_site_facts::ExternGlobalContract {
    hir_site_facts::ExternGlobalContract {
        fqn: contract.fqn.clone(),
        source_path: contract.source_path.clone(),
        span: contract.span,
        ty: contract.ty,
        mutable: contract.mutable,
        symbol: contract.symbol.clone(),
        linkage: match contract.linkage {
            crate::hir::ExternGlobalLinkage::External => {
                hir_site_facts::ExternGlobalLinkage::External
            }
        },
        storage: global_storage_policy(contract.storage),
        initializer_absent: contract.initializer_absent,
        unsafe_required: contract.unsafe_required,
    }
}

fn verify_built_hir_facts(facts: &HirFacts, source_path: &Path) -> Result<(), HirStageError> {
    facts.verify().map_err(|err| {
        HirStageError::new(
            source_path,
            Span::new(0, 0),
            format!("HIR facts verification failed: {err}"),
            "<hir_facts>",
        )
    })
}

fn populate_type_context_facts(facts: &mut HirFacts, lowered_hir: &LoweredHir) {
    facts.type_context.stable_type_params = lowered_hir
        .stable_type_param_keys
        .iter()
        .map(|(param, key)| StableTypeParamFact {
            owner: CanonicalTextKey::new(key.owner_def_key()),
            index: u32::try_from(key.index()).unwrap_or(u32::MAX),
            key: CanonicalTextKey::new(format!("{}#{}", key.owner_def_key(), key.index())),
            name: param.name.clone(),
            source: None,
        })
        .collect();
    facts.type_context.stable_type_params.sort_by(|lhs, rhs| {
        lhs.owner
            .as_str()
            .cmp(rhs.owner.as_str())
            .then(lhs.index.cmp(&rhs.index))
            .then(lhs.name.cmp(&rhs.name))
    });

    facts.type_context.source_cones = lowered_hir
        .source_cones
        .iter()
        .map(|(path, cone)| SourceConeFact {
            source_id: None,
            source_path: path.clone(),
            cone_id: cone.id,
            stable_key: cone.stable_key.clone(),
        })
        .collect();
    facts.type_context.source_cones.sort_by(|lhs, rhs| {
        lhs.source_path
            .cmp(&rhs.source_path)
            .then(lhs.cone_id.as_u32().cmp(&rhs.cone_id.as_u32()))
    });
}

fn populate_declaration_facts(facts: &mut DeclarationFacts, lowered_hir: &LoweredHir) {
    for decl in &lowered_hir.file.decls {
        collect_decl_declaration_facts(facts, lowered_hir, decl);
    }
    collect_missing_nominal_side_table_facts(facts, lowered_hir);
    collect_callable_declaration_facts(facts, lowered_hir);
    collect_materializer_inventory_facts(facts, lowered_hir);
    collect_layout_field_facts(facts, lowered_hir);
    collect_dispatch_declaration_facts(facts, lowered_hir);

    facts
        .nominals
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts
        .callables
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts.generic_templates.sort_by(|lhs, rhs| {
        lhs.template_source_path
            .cmp(&rhs.template_source_path)
            .then(
                lhs.template_decl_span
                    .start
                    .cmp(&rhs.template_decl_span.start),
            )
            .then(lhs.template_decl_span.end.cmp(&rhs.template_decl_span.end))
            .then(lhs.template_fqn.cmp(&rhs.template_fqn))
    });
    facts.callable_bodies.sort_by(|lhs, rhs| {
        lhs.source_path
            .cmp(&rhs.source_path)
            .then(lhs.body_span.start.cmp(&rhs.body_span.start))
            .then(lhs.body_span.end.cmp(&rhs.body_span.end))
            .then(lhs.fqn.cmp(&rhs.fqn))
    });
    facts.fields.sort_by(|lhs, rhs| {
        lhs.owner
            .as_str()
            .cmp(rhs.owner.as_str())
            .then(lhs.identity.display_name.cmp(&rhs.identity.display_name))
    });
    facts
        .fields
        .dedup_by(|lhs, rhs| lhs.identity.key.as_str() == rhs.identity.key.as_str());
    facts.enum_variants.sort_by(|lhs, rhs| {
        lhs.enum_owner
            .as_str()
            .cmp(rhs.enum_owner.as_str())
            .then(lhs.tag.cmp(&rhs.tag))
            .then(lhs.identity.display_name.cmp(&rhs.identity.display_name))
    });
    facts
        .enum_variants
        .dedup_by(|lhs, rhs| lhs.identity.key.as_str() == rhs.identity.key.as_str());
}

fn collect_decl_declaration_facts(
    facts: &mut DeclarationFacts,
    lowered_hir: &LoweredHir,
    decl: &Decl,
) {
    match decl {
        Decl::Nominal(nominal) => {
            let direct_supertypes = lowered_hir
                .direct_supertypes
                .get(&nominal.fqn)
                .cloned()
                .unwrap_or_else(|| {
                    direct_supertypes_from_decl(&nominal.supertypes, &nominal.interfaces)
                });
            facts.nominals.push(NominalDeclarationFact {
                identity: fact_identity("nominal", &nominal.fqn, lowered_hir),
                kind: nominal_kind_from_ast(nominal.kind),
                type_params: type_parameter_facts(&nominal.type_params),
                direct_supertypes: direct_supertypes
                    .into_iter()
                    .map(CanonicalTextKey::new)
                    .collect(),
            });
            for (member_index, member) in nominal.members.iter().enumerate() {
                collect_member_declaration_facts(
                    facts,
                    lowered_hir,
                    member,
                    &nominal.fqn,
                    field_owner_kind_for_nominal(nominal.kind),
                    member_index,
                );
            }
        }
        Decl::Object(object) => {
            facts.nominals.push(NominalDeclarationFact {
                identity: fact_identity("object", &object.fqn, lowered_hir),
                kind: HirFactNominalKind::Object,
                type_params: Vec::new(),
                direct_supertypes: direct_supertypes_from_decl(
                    &object.supertypes,
                    &object.interfaces,
                )
                .into_iter()
                .map(CanonicalTextKey::new)
                .collect(),
            });
            for (member_index, member) in object.members.iter().enumerate() {
                collect_member_declaration_facts(
                    facts,
                    lowered_hir,
                    member,
                    &object.fqn,
                    Some(FieldOwnerKind::Object),
                    member_index,
                );
            }
        }
        Decl::TypeAlias(_) | Decl::ExtensionProperty(_) => {}
    }
}

fn collect_member_declaration_facts(
    facts: &mut DeclarationFacts,
    lowered_hir: &LoweredHir,
    member: &crate::hir::DeclMember,
    owner_fqn: &str,
    owner_kind: Option<FieldOwnerKind>,
    member_index: usize,
) {
    match member {
        crate::hir::DeclMember::Field(field) => {
            if let Some(owner_kind) = owner_kind {
                facts.fields.push(field_declaration_fact(
                    lowered_hir,
                    field_fact_kind(owner_kind),
                    &field.fqn,
                    owner_fqn,
                    owner_kind,
                    &field.name,
                    field.ty,
                ));
            }
        }
        crate::hir::DeclMember::Property(property) => {
            if property.has_backing_field
                && let Some(owner_kind) = owner_kind
            {
                facts.fields.push(field_declaration_fact(
                    lowered_hir,
                    field_fact_kind(owner_kind),
                    &property.fqn,
                    owner_fqn,
                    owner_kind,
                    &property.name,
                    property.ty,
                ));
            }
        }
        crate::hir::DeclMember::EnumVariant(variant) => {
            facts.enum_variants.push(EnumVariantDeclarationFact {
                identity: fact_identity("enum_variant", &variant.fqn, lowered_hir),
                enum_owner: CanonicalTextKey::new(owner_fqn),
                name: variant.name.clone(),
                tag: member_index as u64,
                source: None,
            });
            for field in &variant.fields {
                facts.fields.push(field_declaration_fact(
                    lowered_hir,
                    "enum_variant_field",
                    &field.fqn,
                    &variant.fqn,
                    FieldOwnerKind::EnumVariant,
                    &field.name,
                    field.ty,
                ));
            }
        }
        crate::hir::DeclMember::Nested(nested) => {
            collect_decl_declaration_facts(facts, lowered_hir, nested)
        }
        crate::hir::DeclMember::Fun(_) | crate::hir::DeclMember::InitBlock { .. } => {}
    }
}

fn collect_missing_nominal_side_table_facts(
    facts: &mut DeclarationFacts,
    lowered_hir: &LoweredHir,
) {
    let mut existing = facts
        .nominals
        .iter()
        .map(|fact| fact.identity.display_name.clone())
        .collect::<HashSet<_>>();
    for (fqn, kind) in &lowered_hir.nominal_kinds {
        if !existing.insert(fqn.clone()) {
            continue;
        }
        let direct_supertypes = lowered_hir
            .direct_supertypes
            .get(fqn)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(CanonicalTextKey::new)
            .collect();
        facts.nominals.push(NominalDeclarationFact {
            identity: fact_identity("nominal", fqn, lowered_hir),
            kind: nominal_kind_from_ast(*kind),
            type_params: Vec::new(),
            direct_supertypes,
        });
    }
}

fn collect_callable_declaration_facts(facts: &mut DeclarationFacts, lowered_hir: &LoweredHir) {
    for fun in lowered_hir.file.items.iter().filter_map(|item| match item {
        Item::Fun(fun) => Some(fun),
        _ => None,
    }) {
        facts
            .callables
            .push(callable_declaration_fact(lowered_hir, fun));
    }
    for fun in &lowered_hir.member_funs {
        facts
            .callables
            .push(callable_declaration_fact(lowered_hir, fun));
    }
}

fn collect_materializer_inventory_facts(facts: &mut DeclarationFacts, lowered_hir: &LoweredHir) {
    facts.generic_templates = lowered_hir
        .generic_template_inventory
        .iter()
        .map(|info| GenericTemplateFact {
            identity: FactIdentity::new(
                CanonicalTextKey::new(format!(
                    "generic_template_inventory:{}:{}:{}..{}",
                    info.stable_template_key.canonical_text(),
                    info.template.source_path.display(),
                    info.template.decl_span.start,
                    info.template.decl_span.end
                )),
                &info.template.fqn,
                lowered_hir.stable_cone_key.clone(),
                None,
            ),
            stable_template_key: CanonicalTextKey::new(info.stable_template_key.canonical_text()),
            canonical_root_key: CanonicalTextKey::new(info.canonical_root_key.clone()),
            template_fqn: info.template.fqn.clone(),
            template_source_path: info.template.source_path.clone(),
            template_decl_span: info.template.decl_span,
            request_fqn: info.request_lookup_key.0.clone(),
            request_source_path: info.request_lookup_key.1.clone(),
            request_span: info.request_lookup_key.2,
            owner_type_param_names: info.owner_type_param_names.clone(),
            function_type_param_names: info.function_type_param_names.clone(),
            owner_eff_param_name: info.owner_eff_param_name.clone(),
            function_eff_param_name: info.function_eff_param_name.clone(),
            signature_key: CanonicalTextKey::new(info.signature_key.clone()),
            has_body: info.has_body,
            body_key: info.body_key.clone().map(CanonicalTextKey::new),
        })
        .collect();
    facts.callable_bodies = lowered_hir
        .callable_body_inventory
        .iter()
        .map(|info| CallableBodyFact {
            identity: FactIdentity::new(
                CanonicalTextKey::new(info.body_key.clone()),
                &info.fqn,
                lowered_hir.stable_cone_key.clone(),
                None,
            ),
            body_key: CanonicalTextKey::new(info.body_key.clone()),
            request_fqn: info.request_lookup_key.0.clone(),
            request_source_path: info.request_lookup_key.1.clone(),
            request_span: info.request_lookup_key.2,
            source_path: info.source_path.clone(),
            fqn: info.fqn.clone(),
            body_span: info.body_span,
            stable_template_key: info
                .stable_template_key
                .as_ref()
                .map(|key| CanonicalTextKey::new(key.canonical_text())),
        })
        .collect();
}

fn collect_layout_field_facts(facts: &mut DeclarationFacts, lowered_hir: &LoweredHir) {
    let mut existing_fields = facts
        .fields
        .iter()
        .map(|fact| fact.identity.canonical_text().to_string())
        .collect::<HashSet<_>>();
    let mut existing_enum_variants = facts
        .enum_variants
        .iter()
        .map(|fact| fact.identity.canonical_text().to_string())
        .collect::<HashSet<_>>();

    for (layout_key, layout) in &lowered_hir.struct_layouts {
        for field in &layout.fields {
            if let Some(ty) = field.ty {
                push_unique_field_fact(
                    &mut facts.fields,
                    &mut existing_fields,
                    field_declaration_fact(
                        lowered_hir,
                        "struct_field",
                        &field.fqn,
                        layout_key,
                        FieldOwnerKind::Struct,
                        &field.name,
                        ty.inner(),
                    ),
                );
            }
        }
    }

    for (enum_key, layout) in &lowered_hir.enum_layouts {
        for variant in &layout.variants {
            let variant_fqn = format!("{enum_key}.{}", variant.name);
            push_unique_enum_variant_fact(
                &mut facts.enum_variants,
                &mut existing_enum_variants,
                EnumVariantDeclarationFact {
                    identity: fact_identity("enum_variant", &variant_fqn, lowered_hir),
                    enum_owner: CanonicalTextKey::new(enum_key),
                    name: variant.name.clone(),
                    tag: variant.tag,
                    source: None,
                },
            );
            for (index, field) in variant.fields.iter().enumerate() {
                if let Some(ty) = field.ty {
                    let field_name = if field.name.is_empty() {
                        format!("_{index}")
                    } else {
                        field.name.clone()
                    };
                    let field_fqn = format!("{variant_fqn}.{field_name}");
                    push_unique_field_fact(
                        &mut facts.fields,
                        &mut existing_fields,
                        field_declaration_fact(
                            lowered_hir,
                            "enum_variant_field",
                            &field_fqn,
                            &variant_fqn,
                            FieldOwnerKind::EnumVariant,
                            &field_name,
                            ty.inner(),
                        ),
                    );
                }
            }
        }
    }

    for (class_key, class) in &lowered_hir.generic_class_decls {
        for field in &class.fields {
            facts.fields.push(field_declaration_fact(
                lowered_hir,
                "class_field",
                &field.fqn,
                class_key,
                FieldOwnerKind::Class,
                &field.name,
                field.ty,
            ));
        }
    }
    for (class_key, class) in &lowered_hir.class_inits {
        if lowered_hir
            .generic_class_decls
            .contains_key(class_key.as_str())
        {
            continue;
        }
        for field in &class.fields {
            facts.fields.push(field_declaration_fact(
                lowered_hir,
                "class_field",
                &field.fqn,
                class_key.as_str(),
                FieldOwnerKind::Class,
                &field.name,
                field.ty.inner(),
            ));
        }
    }

    for (object_key, object) in &lowered_hir.object_inits {
        for (name, property) in &object.properties {
            let property_fqn = format!("{object_key}.{name}");
            facts.fields.push(field_declaration_fact(
                lowered_hir,
                "object_property",
                &property_fqn,
                object_key,
                FieldOwnerKind::Object,
                name,
                property.ty,
            ));
        }
    }
}

fn push_unique_field_fact(
    facts: &mut Vec<FieldDeclarationFact>,
    existing: &mut HashSet<String>,
    fact: FieldDeclarationFact,
) {
    if existing.insert(fact.identity.canonical_text().to_string()) {
        facts.push(fact);
    }
}

fn push_unique_enum_variant_fact(
    facts: &mut Vec<EnumVariantDeclarationFact>,
    existing: &mut HashSet<String>,
    fact: EnumVariantDeclarationFact,
) {
    if existing.insert(fact.identity.canonical_text().to_string()) {
        facts.push(fact);
    }
}

fn populate_global_root_facts(facts: &mut HirFacts, lowered_hir: &LoweredHir) {
    for value in lowered_hir.top_level_immutable_values.values() {
        facts.globals.roots.push(GlobalRootFact {
            identity: fact_identity("top_level_val", &value.fqn, lowered_hir),
            kind: GlobalRootKind::TopLevelVal,
            ty: Some(value.ty),
            storage: None,
            initializer: None,
            monomorphic: true,
        });
    }

    for var in lowered_hir.top_level_vars.values() {
        facts.globals.roots.push(GlobalRootFact {
            identity: fact_identity("top_level_var", &var.fqn, lowered_hir),
            kind: GlobalRootKind::TopLevelVar,
            ty: Some(var.ty),
            storage: Some(global_storage_policy(var.storage)),
            initializer: None,
            monomorphic: true,
        });
    }

    for object in lowered_hir.object_inits.values() {
        facts.globals.roots.push(GlobalRootFact {
            identity: fact_identity("object_singleton", &object.fqn, lowered_hir),
            kind: GlobalRootKind::ObjectSingleton,
            ty: lowered_hir.types.find_nominal_ref_by_fqn(&object.fqn),
            storage: None,
            initializer: None,
            monomorphic: true,
        });
        facts
            .globals
            .object_initializers
            .push(initializer_fact_from_object(lowered_hir, object));
    }

    for class in lowered_hir.generic_class_decls.values() {
        facts
            .globals
            .class_initializers
            .push(initializer_fact_from_class(lowered_hir, class));
    }
    // 单态化实例化的 class（mangled FQN 不在 generic_class_decls 中）补充进 facts，
    // 以保留 split 前的全集可见性。
    for (mangled_fqn, mono) in &lowered_hir.class_inits {
        if lowered_hir
            .generic_class_decls
            .contains_key(mangled_fqn.as_str())
        {
            continue;
        }
        facts
            .globals
            .class_initializers
            .push(initializer_fact_from_mono_class(lowered_hir, mono));
    }

    facts
        .globals
        .roots
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts
        .globals
        .object_initializers
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts
        .globals
        .class_initializers
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
}

fn populate_native_extern_facts(facts: &mut HirFacts, lowered_hir: &LoweredHir) {
    for (fqn, extern_fun) in &lowered_hir.extern_funs {
        let signature = callable_signature_by_fqn(lowered_hir, fqn);
        facts.native.extern_functions.push(ExternFunctionFact {
            identity: fact_identity("extern_fun", fqn, lowered_hir),
            symbol: extern_fun.symbol.clone(),
            calling_convention: extern_fun
                .calling_convention
                .clone()
                .unwrap_or_else(|| extern_fun.abi.name().to_string()),
            parameter_tys: signature.parameter_tys,
            return_ty: signature.return_ty,
            effects: signature.effects,
        });
    }

    for (fqn, native_fun) in &lowered_hir.native_callable_funs {
        let signature = callable_signature_by_fqn(lowered_hir, fqn);
        facts.native.native_callables.push(NativeCallableFact {
            identity: fact_identity("native_callable", fqn, lowered_hir),
            symbol: native_fun.symbol.clone(),
            calling_convention: native_fun.calling_convention.clone(),
            parameter_tys: signature.parameter_tys,
            return_ty: signature.return_ty,
            effects: signature.effects,
        });
    }

    for extern_global in lowered_hir.extern_globals.values() {
        facts.native.extern_globals.push(ExternGlobalFact {
            identity: fact_identity("extern_global", &extern_global.fqn, lowered_hir),
            symbol: extern_global.symbol.clone(),
            ty: extern_global.ty,
            mutable: extern_global.mutable,
        });
    }

    facts.native.extern_libraries = lowered_hir
        .extern_libs
        .iter()
        .cloned()
        .map(|name| ExternLibraryFact { name, source: None })
        .collect();

    facts
        .native
        .extern_functions
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts
        .native
        .native_callables
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts
        .native
        .extern_globals
        .sort_by(|lhs, rhs| lhs.identity.display_name.cmp(&rhs.identity.display_name));
    facts
        .native
        .extern_libraries
        .sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
}

/// Publish source-level dispatch tables without exposing backend vtable/itable types.
fn collect_dispatch_declaration_facts(facts: &mut DeclarationFacts, lowered_hir: &LoweredHir) {
    facts.dispatch.vtables = lowered_hir
        .class_vtables
        .iter()
        .map(|(class_fqn, slots)| DispatchTableFact {
            owner: CanonicalTextKey::new(class_fqn.as_str()),
            slots: slots
                .iter()
                .map(|slot| DispatchSlotFact {
                    index: slot.slot,
                    declaration: CanonicalTextKey::new(slot.impl_member_fqn.as_str()),
                    signature_ty: callable_type_by_dispatch_shape(
                        lowered_hir,
                        &slot.impl_member_fqn,
                        slot.params_len,
                        slot.has_receiver,
                    ),
                })
                .collect(),
        })
        .collect();
    facts
        .dispatch
        .vtables
        .sort_by(|lhs, rhs| lhs.owner.as_str().cmp(rhs.owner.as_str()));

    let mut interface_tables = lowered_hir
        .interfaces
        .iter()
        .map(|(interface_fqn, interface)| DispatchTableFact {
            owner: CanonicalTextKey::new(interface_fqn.as_str()),
            slots: interface
                .method_slots
                .iter()
                .map(|slot| DispatchSlotFact {
                    index: slot.slot,
                    declaration: CanonicalTextKey::new(slot.member_fqn.as_str()),
                    signature_ty: callable_type_by_dispatch_shape(
                        lowered_hir,
                        &slot.member_fqn,
                        slot.params_len,
                        slot.has_receiver,
                    ),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    interface_tables.extend(
        lowered_hir
            .class_itables
            .iter()
            .flat_map(|(class_fqn, entries)| {
                entries.iter().map(move |entry| DispatchTableFact {
                    owner: CanonicalTextKey::new(format!(
                        "{} implements {}",
                        class_fqn, entry.interface_type_name
                    )),
                    slots: entry
                        .method_impl_fqns
                        .iter()
                        .enumerate()
                        .filter(|(_, impl_fqn)| !impl_fqn.is_empty())
                        .map(|(index, impl_fqn)| DispatchSlotFact {
                            index: u32::try_from(index).unwrap_or(u32::MAX),
                            declaration: CanonicalTextKey::new(impl_fqn.as_str()),
                            signature_ty: lowered_hir
                                .interfaces
                                .get(&entry.interface_fqn)
                                .and_then(|interface| interface.method_slots.get(index))
                                .map(|slot| {
                                    callable_type_by_dispatch_shape(
                                        lowered_hir,
                                        impl_fqn,
                                        slot.params_len,
                                        slot.has_receiver,
                                    )
                                })
                                .unwrap_or_else(|| callable_type_by_fqn(lowered_hir, impl_fqn)),
                        })
                        .collect(),
                })
            }),
    );
    interface_tables.sort_by(|lhs, rhs| lhs.owner.as_str().cmp(rhs.owner.as_str()));
    facts.dispatch.interface_tables = interface_tables;
}

#[derive(Clone)]
struct CallableSignature {
    receiver_ty: Option<TypeId>,
    parameter_tys: Vec<TypeId>,
    return_ty: TypeId,
    effects: EffectRow,
}

fn callable_declaration_fact(lowered_hir: &LoweredHir, fun: &FunDecl) -> CallableDeclarationFact {
    let signature = callable_signature_from_fun(lowered_hir, fun);
    CallableDeclarationFact {
        identity: callable_fact_identity(fun, lowered_hir),
        receiver_ty: signature.receiver_ty,
        parameter_tys: signature.parameter_tys,
        return_ty: signature.return_ty,
        effects: signature.effects,
        type_params: Vec::new(),
        has_body: fun.body.is_some(),
    }
}

/// Callable overloads share display FQNs, so the stable fact key includes source position.
fn callable_fact_identity(fun: &FunDecl, lowered_hir: &LoweredHir) -> FactIdentity {
    FactIdentity::new(
        CanonicalTextKey::new(format!(
            "callable:{}:{}:{}..{}",
            fun.source_path.display(),
            fun.fqn,
            fun.span.start,
            fun.span.end
        )),
        &fun.fqn,
        lowered_hir.stable_cone_key.clone(),
        None,
    )
}

fn callable_signature_by_fqn(lowered_hir: &LoweredHir, fqn: &str) -> CallableSignature {
    lowered_hir
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == fqn => Some(callable_signature_from_fun(lowered_hir, fun)),
            _ => None,
        })
        .or_else(|| {
            lowered_hir
                .member_funs
                .iter()
                .find(|fun| fun.fqn == fqn)
                .map(|fun| callable_signature_from_fun(lowered_hir, fun))
        })
        .unwrap_or_else(|| CallableSignature {
            receiver_ty: None,
            parameter_tys: Vec::new(),
            return_ty: lowered_hir.builtins.unit,
            effects: EffectRow::pure(),
        })
}

fn callable_type_by_fqn(lowered_hir: &LoweredHir, fqn: &str) -> TypeId {
    lowered_hir
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == fqn => Some(fun.ty),
            _ => None,
        })
        .or_else(|| {
            lowered_hir
                .member_funs
                .iter()
                .find(|fun| fun.fqn == fqn)
                .map(|fun| fun.ty)
        })
        .unwrap_or(lowered_hir.builtins.unit)
}

fn callable_type_by_dispatch_shape(
    lowered_hir: &LoweredHir,
    fqn: &str,
    params_len: u32,
    has_receiver: bool,
) -> TypeId {
    callable_funs_by_fqn(lowered_hir, fqn)
        .find(|fun| {
            fun.params.len() == params_len as usize
                && function_type_has_receiver(&lowered_hir.types, fun.ty) == has_receiver
        })
        .map(|fun| fun.ty)
        .unwrap_or_else(|| callable_type_by_fqn(lowered_hir, fqn))
}

fn callable_funs_by_fqn<'a>(
    lowered_hir: &'a LoweredHir,
    fqn: &'a str,
) -> impl Iterator<Item = &'a FunDecl> {
    lowered_hir
        .file
        .items
        .iter()
        .filter_map(move |item| match item {
            Item::Fun(fun) if fun.fqn == fqn => Some(fun),
            _ => None,
        })
        .chain(
            lowered_hir
                .member_funs
                .iter()
                .filter(move |fun| fun.fqn == fqn),
        )
}

fn function_type_has_receiver(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Function(fun)) if fun.receiver.is_some())
}

fn callable_signature_from_fun(lowered_hir: &LoweredHir, fun: &FunDecl) -> CallableSignature {
    if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = lowered_hir.types.kind(fun.ty) {
        return CallableSignature {
            receiver_ty: fun_ty.receiver,
            parameter_tys: fun_ty.params.clone(),
            return_ty: fun_ty.return_ty,
            effects: fun_ty.effects.clone(),
        };
    }
    CallableSignature {
        receiver_ty: None,
        parameter_tys: fun.params.iter().map(|param| param.ty).collect(),
        return_ty: fun.return_ty,
        effects: EffectRow::pure(),
    }
}

fn field_declaration_fact(
    lowered_hir: &LoweredHir,
    kind: &str,
    fqn: &str,
    owner: &str,
    owner_kind: FieldOwnerKind,
    name: &str,
    ty: TypeId,
) -> FieldDeclarationFact {
    FieldDeclarationFact {
        identity: FactIdentity::new(
            CanonicalTextKey::new(format!("{kind}:{owner}:{fqn}")),
            fqn,
            lowered_hir.stable_cone_key.clone(),
            None,
        ),
        owner: CanonicalTextKey::new(owner),
        owner_kind,
        name: name.to_string(),
        ty,
        source: None,
    }
}

fn initializer_fact_from_object(
    lowered_hir: &LoweredHir,
    object: &crate::hir::ObjectInit,
) -> InitializerFact {
    let mut fields = object
        .properties
        .values()
        .map(|property| InitializerFieldFact {
            name: property.name.clone(),
            ty: property.ty,
            source: None,
        })
        .collect::<Vec<_>>();
    fields.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
    InitializerFact {
        identity: fact_identity("object_initializer", &object.fqn, lowered_hir),
        initialized_root: CanonicalTextKey::new(&object.fqn),
        fields,
    }
}

fn initializer_fact_from_class(
    lowered_hir: &LoweredHir,
    class: &crate::hir::GenericClassDecl,
) -> InitializerFact {
    let mut fields = class
        .fields
        .iter()
        .map(|field| InitializerFieldFact {
            name: field.name.clone(),
            ty: field.ty,
            source: None,
        })
        .collect::<Vec<_>>();
    fields.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
    InitializerFact {
        identity: fact_identity("class_initializer", &class.fqn, lowered_hir),
        initialized_root: CanonicalTextKey::new(&class.fqn),
        fields,
    }
}

fn initializer_fact_from_mono_class(
    lowered_hir: &LoweredHir,
    class: &crate::hir::MonoClassInit,
) -> InitializerFact {
    let mut fields = class
        .fields
        .iter()
        .map(|field| InitializerFieldFact {
            name: field.name.clone(),
            ty: field.ty.inner(),
            source: None,
        })
        .collect::<Vec<_>>();
    fields.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
    InitializerFact {
        identity: fact_identity("class_initializer", &class.fqn, lowered_hir),
        initialized_root: CanonicalTextKey::new(&class.fqn),
        fields,
    }
}

fn type_parameter_facts(params: &[crate::hir::DeclTypeParam]) -> Vec<TypeParameterFact> {
    params
        .iter()
        .map(|param| TypeParameterFact {
            key: CanonicalTextKey::new(format!(
                "type_param:{}:{}..{}",
                param.name, param.span.start, param.span.end
            )),
            name: param.name.clone(),
            variance: variance_from_ast(param.variance),
            source: None,
        })
        .collect()
}

fn direct_supertypes_from_decl(
    supertypes: &[crate::hir::SupertypeDecl],
    interfaces: &[String],
) -> Vec<String> {
    let mut out = supertypes
        .iter()
        .filter_map(|supertype| supertype.fqn.clone())
        .chain(interfaces.iter().cloned())
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

fn nominal_kind_from_ast(kind: ast::TypeKind) -> HirFactNominalKind {
    match kind {
        ast::TypeKind::Class => HirFactNominalKind::Class,
        ast::TypeKind::Interface => HirFactNominalKind::Interface,
        ast::TypeKind::Struct => HirFactNominalKind::Struct,
        ast::TypeKind::Enum => HirFactNominalKind::Enum,
        ast::TypeKind::Effect => HirFactNominalKind::Effect,
    }
}

fn field_owner_kind_for_nominal(kind: ast::TypeKind) -> Option<FieldOwnerKind> {
    match kind {
        ast::TypeKind::Class => Some(FieldOwnerKind::Class),
        ast::TypeKind::Struct => Some(FieldOwnerKind::Struct),
        ast::TypeKind::Enum => None,
        ast::TypeKind::Interface | ast::TypeKind::Effect => None,
    }
}

fn field_fact_kind(owner_kind: FieldOwnerKind) -> &'static str {
    match owner_kind {
        FieldOwnerKind::Struct => "struct_field",
        FieldOwnerKind::Class => "class_field",
        FieldOwnerKind::Object => "object_property",
        FieldOwnerKind::EnumVariant => "enum_variant_field",
    }
}

fn variance_from_ast(variance: Option<ast::TypeParamVariance>) -> HirFactVariance {
    match variance {
        Some(ast::TypeParamVariance::In) => HirFactVariance::Contravariant,
        Some(ast::TypeParamVariance::Out) => HirFactVariance::Covariant,
        None => HirFactVariance::Invariant,
    }
}

fn global_storage_policy(storage: crate::hir::TopLevelVarStorage) -> GlobalStoragePolicy {
    match storage {
        crate::hir::TopLevelVarStorage::ThreadLocal => GlobalStoragePolicy::ThreadLocal,
        crate::hir::TopLevelVarStorage::Global => GlobalStoragePolicy::Global,
    }
}

fn fact_identity(kind: &str, fqn: &str, lowered_hir: &LoweredHir) -> FactIdentity {
    FactIdentity::new(
        CanonicalTextKey::new(format!("{kind}:{fqn}")),
        fqn,
        lowered_hir.stable_cone_key.clone(),
        None,
    )
}

struct ContractCollector<'a> {
    lowered_hir: &'a LoweredHir,
    runtime_error_effect_ty: Option<TypeId>,
    function_effects: Vec<FunctionEffectContract>,
    continuation_resume_sites: HashMap<CallSite, ContinuationResumeSiteContract>,
    perform_sites: HashMap<CallSite, PerformSiteContract>,
    handle_sites: HashMap<CallSite, HandleSiteContract>,
    call_site_kinds: HashMap<CallSite, TypedCallSiteKind>,
    call_site_contracts: HashMap<CallSite, TypedCallSiteContract>,
    with_update_contracts: HashMap<CallSite, ast::WithUpdateContract>,
    assign_place_contracts: HashMap<CallSite, AssignPlaceContract>,
    top_level_init_roots: Vec<TopLevelInitRootContract>,
    extern_global_contracts: Vec<ExternGlobalContract>,
}

impl<'a> ContractCollector<'a> {
    fn new(lowered_hir: &'a LoweredHir) -> Self {
        Self {
            lowered_hir,
            runtime_error_effect_ty: find_raise_runtime_error_effect(&lowered_hir.types),
            function_effects: Vec::new(),
            continuation_resume_sites: HashMap::new(),
            perform_sites: HashMap::new(),
            handle_sites: HashMap::new(),
            call_site_kinds: HashMap::new(),
            call_site_contracts: HashMap::new(),
            with_update_contracts: lowered_hir.with_update_contracts.clone(),
            assign_place_contracts: lowered_hir.assign_place_contracts.clone(),
            top_level_init_roots: collect_top_level_init_roots(lowered_hir),
            extern_global_contracts: collect_extern_global_contracts(lowered_hir),
        }
    }

    fn collect(mut self, source_path: &Path) -> Result<CollectedHirContracts, HirStageError> {
        for item in &self.lowered_hir.file.items {
            let item_path = self
                .item_source_path(source_path, item)
                .unwrap_or_else(|| source_path.to_path_buf());
            self.collect_item(&item_path, item)?;
        }

        self.collect_object_inits(None)?;
        self.collect_class_inits(None)?;

        for member_fun in &self.lowered_hir.member_funs {
            self.record_function_effect_contract(member_fun);
            self.collect_fun(member_fun)?;
        }

        self.function_effects
            .sort_by(compare_function_effect_contracts);
        Ok(CollectedHirContracts {
            function_effects: self.function_effects,
            continuation_resume_sites: self.continuation_resume_sites,
            perform_sites: self.perform_sites,
            handle_sites: self.handle_sites,
            call_site_kinds: self.call_site_kinds,
            call_site_contracts: self.call_site_contracts,
            with_update_contracts: self.with_update_contracts,
            assign_place_contracts: self.assign_place_contracts,
            top_level_init_roots: self.top_level_init_roots,
            extern_global_contracts: self.extern_global_contracts,
        })
    }

    fn collect_source_path(
        mut self,
        source_path: &Path,
    ) -> Result<CollectedHirContracts, HirStageError> {
        for item in &self.lowered_hir.file.items {
            let item_path = self.item_source_path(source_path, item);
            if item_path.as_deref() == Some(source_path) {
                self.collect_item(source_path, item)?;
            }
        }

        self.collect_object_inits(Some(source_path))?;
        self.collect_class_inits(Some(source_path))?;

        for member_fun in &self.lowered_hir.member_funs {
            if member_fun.source_path != source_path {
                continue;
            }
            self.record_function_effect_contract(member_fun);
            self.collect_fun(member_fun)?;
        }

        self.function_effects
            .sort_by(compare_function_effect_contracts);
        Ok(CollectedHirContracts {
            function_effects: self.function_effects,
            continuation_resume_sites: self.continuation_resume_sites,
            perform_sites: self.perform_sites,
            handle_sites: self.handle_sites,
            call_site_kinds: self.call_site_kinds,
            call_site_contracts: self.call_site_contracts,
            with_update_contracts: self.with_update_contracts,
            assign_place_contracts: self.assign_place_contracts,
            top_level_init_roots: self.top_level_init_roots,
            extern_global_contracts: self.extern_global_contracts,
        })
    }

    fn item_source_path(&self, default_source_path: &Path, item: &Item) -> Option<PathBuf> {
        match item {
            Item::Fun(fun) => Some(fun.source_path.clone()),
            Item::Val(val) => self
                .top_level_val_source_path(val)
                .or_else(|| Some(default_source_path.to_path_buf())),
            Item::Todo { .. } => None,
        }
    }

    fn collect_item(&mut self, source_path: &Path, item: &Item) -> Result<(), HirStageError> {
        match item {
            Item::Fun(fun) => {
                self.record_function_effect_contract(fun);
                self.collect_fun(fun)?;
            }
            Item::Val(val) => {
                let source_path = self
                    .top_level_val_source_path(val)
                    .unwrap_or_else(|| source_path.to_path_buf());
                if let Some(init) = &val.init {
                    self.collect_expr(&source_path, init)?;
                }
            }
            Item::Todo { .. } => {}
        }
        Ok(())
    }

    fn collect_object_inits(&mut self, only_path: Option<&Path>) -> Result<(), HirStageError> {
        let mut object_inits = self
            .lowered_hir
            .object_inits
            .values()
            .cloned()
            .collect::<Vec<_>>();
        object_inits.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for object in &object_inits {
            if only_path.is_some_and(|path| object.source_path != path) {
                continue;
            }
            self.collect_object_init(object)?;
        }
        Ok(())
    }

    fn collect_object_init(&mut self, object: &ObjectInit) -> Result<(), HirStageError> {
        for step in &object.steps {
            match step {
                ObjectInitStep::PropertyInit { init, .. } => {
                    self.collect_expr(&object.source_path, init)?;
                }
                ObjectInitStep::InitBlock { block } => {
                    self.collect_block(&object.source_path, block)?;
                }
            }
        }
        Ok(())
    }

    fn collect_class_inits(&mut self, only_path: Option<&Path>) -> Result<(), HirStageError> {
        let mut class_inits = self
            .lowered_hir
            .class_inits
            .values()
            .cloned()
            .collect::<Vec<_>>();
        class_inits.sort_by(|lhs, rhs| lhs.fqn.cmp(&rhs.fqn));
        for class in &class_inits {
            if only_path.is_some_and(|path| class.source_path != path) {
                continue;
            }
            self.collect_class_init(class)?;
        }
        Ok(())
    }

    fn collect_class_init(&mut self, class: &MonoClassInit) -> Result<(), HirStageError> {
        self.collect_class_init_parts(
            &class.source_path,
            &class.super_ctor_args,
            &class.steps,
            &class.ctors,
        )
    }

    fn collect_class_init_parts<T>(
        &mut self,
        source_path: &Path,
        super_ctor_args: &[CallArg],
        steps: &[ClassInitStep],
        ctors: &[ClassCtor<T>],
    ) -> Result<(), HirStageError> {
        for arg in super_ctor_args {
            self.collect_call_arg_expr(source_path, arg)?;
        }
        for step in steps {
            match step {
                ClassInitStep::PropertyInit { init, .. } => {
                    self.collect_expr(source_path, init)?;
                }
                ClassInitStep::InitBlock { block } => {
                    self.collect_block(source_path, block)?;
                }
            }
        }
        for ctor in ctors {
            self.collect_class_ctor(source_path, ctor)?;
        }
        Ok(())
    }

    fn collect_class_ctor<T>(
        &mut self,
        source_path: &Path,
        ctor: &ClassCtor<T>,
    ) -> Result<(), HirStageError> {
        for param in &ctor.params {
            if let Some(default_value) = &param.default_value {
                self.collect_expr(source_path, default_value)?;
            }
        }
        if let Some(delegation) = &ctor.delegation {
            for arg in &delegation.args {
                self.collect_call_arg_expr(source_path, arg)?;
            }
        }
        if let Some(body) = &ctor.body {
            self.collect_block(source_path, body)?;
        }
        Ok(())
    }

    fn record_function_effect_contract(&mut self, fun: &FunDecl) {
        let Some((allowed_effects, effects_closed)) =
            function_effect_contract(&self.lowered_hir.types, fun.ty)
        else {
            return;
        };

        self.function_effects.push(FunctionEffectContract::new(
            fun.source_path.clone(),
            fun.span,
            fun.fqn.clone(),
            fun.return_ty,
            allowed_effects,
            effects_closed,
        ));
    }

    fn hir_fun_decl(&self, fqn: &str) -> Option<&FunDecl> {
        self.lowered_hir
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == fqn => Some(fun),
                _ => None,
            })
            .or_else(|| {
                self.lowered_hir
                    .member_funs
                    .iter()
                    .find(|fun| fun.fqn == fqn)
            })
    }

    fn hir_fun_decl_at(&self, fqn: &str, decl_file: &Path, decl_span: Span) -> Option<&FunDecl> {
        fn span_contains(outer: Span, inner: Span) -> bool {
            outer.start <= inner.start && inner.end <= outer.end
        }

        self.lowered_hir
            .file
            .items
            .iter()
            .find_map(|item| {
                let Item::Fun(fun) = item else {
                    return None;
                };
                (fun.fqn == fqn
                    && fun.source_path.as_path() == decl_file
                    && (fun.span == decl_span || span_contains(fun.span, decl_span)))
                .then_some(fun)
            })
            .or_else(|| {
                self.lowered_hir.member_funs.iter().find(|fun| {
                    fun.fqn == fqn
                        && fun.source_path.as_path() == decl_file
                        && (fun.span == decl_span || span_contains(fun.span, decl_span))
                })
            })
    }

    fn unique_hir_fun_decl(&self, fqn: &str) -> Option<&FunDecl> {
        let mut found = None;
        for item in &self.lowered_hir.file.items {
            let Item::Fun(fun) = item else {
                continue;
            };
            if fun.fqn != fqn {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(fun);
        }
        for fun in &self.lowered_hir.member_funs {
            if fun.fqn != fqn {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(fun);
        }
        found
    }

    fn unique_hir_fun_fqn_by_local_name(&self, name: &str) -> Option<String> {
        let mut found = None::<String>;
        for item in &self.lowered_hir.file.items {
            let Item::Fun(fun) = item else {
                continue;
            };
            if fun.fqn.rsplit('.').next() != Some(name) {
                continue;
            }
            if found.as_deref().is_some_and(|fqn| fqn != fun.fqn) {
                return None;
            }
            found = Some(fun.fqn.clone());
        }
        for body in &self.lowered_hir.callable_body_inventory {
            if body.fqn.rsplit('.').next() != Some(name) {
                continue;
            }
            if found.as_deref().is_some_and(|fqn| fqn != body.fqn) {
                return None;
            }
            found = Some(body.fqn.clone());
        }
        found
    }

    fn has_multiple_hir_fun_decls(&self, fqn: &str) -> bool {
        let mut found_one = false;
        for item in &self.lowered_hir.file.items {
            let Item::Fun(fun) = item else {
                continue;
            };
            if fun.fqn != fqn {
                continue;
            }
            if found_one {
                return true;
            }
            found_one = true;
        }
        for fun in &self.lowered_hir.member_funs {
            if fun.fqn != fqn {
                continue;
            }
            if found_one {
                return true;
            }
            found_one = true;
        }
        false
    }

    fn stable_template_key_for_binding(
        &self,
        binding: &ast::TopLevelFunCallBinding,
    ) -> Option<StableTemplateKey> {
        fn span_contains(outer: Span, inner: Span) -> bool {
            outer.start <= inner.start && inner.end <= outer.end
        }

        self.lowered_hir
            .generic_stable_template_keys
            .iter()
            .filter(|(template, _)| {
                template.fqn == binding.fqn
                    && template.source_path == binding.decl_file
                    && (template.decl_span == binding.decl_span
                        || span_contains(template.decl_span, binding.decl_span))
            })
            .min_by(|(lhs, _), (rhs, _)| {
                lhs.decl_span
                    .start
                    .cmp(&rhs.decl_span.start)
                    .then(lhs.decl_span.end.cmp(&rhs.decl_span.end))
            })
            .map(|(_, stable)| stable.clone())
            .or_else(|| {
                self.lowered_hir
                    .generic_template_inventory
                    .iter()
                    .filter(|info| {
                        info.template.fqn == binding.fqn
                            && info.template.source_path == binding.decl_file
                            && (info.template.decl_span == binding.decl_span
                                || span_contains(info.template.decl_span, binding.decl_span)
                                || info.request_lookup_key.2 == binding.decl_span)
                    })
                    .min_by(|lhs, rhs| {
                        lhs.template
                            .decl_span
                            .start
                            .cmp(&rhs.template.decl_span.start)
                            .then(lhs.template.decl_span.end.cmp(&rhs.template.decl_span.end))
                    })
                    .map(|info| info.stable_template_key.clone())
            })
    }

    fn synthetic_stable_member_target(
        &self,
        fqn: &str,
        args: &[CallArg],
        return_ty: TypeId,
        abi_identity: CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    ) -> Option<FunctionTargetContract> {
        let template_fqn = generic_template_base_fqn(fqn);
        let receiver_ty = args.first().map(call_arg_value_ty)?;
        let (owner_fqn, _) = self.member_binding_for_fqn(template_fqn)?;
        let (receiver_fqn, owner_type_args, owner_eff_arg) =
            match self.lowered_hir.types.kind(receiver_ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => (
                    nominal.fqn.as_str(),
                    nominal.args.clone(),
                    nominal.eff.clone(),
                ),
                _ => return None,
            };
        if receiver_fqn != owner_fqn {
            return None;
        }

        let template = self
            .lowered_hir
            .generic_template_inventory
            .iter()
            .find(|template| {
                template.template.fqn == template_fqn
                    && template.owner_type_param_names.len() == owner_type_args.len()
                    && template.function_type_param_names.is_empty()
            })?;
        let eff_args = match (&template.owner_eff_param_name, owner_eff_arg) {
            (Some(_), Some(row)) => vec![row],
            (Some(_), None) => return None,
            (None, _) => Vec::new(),
        };

        Some(FunctionTargetContract::synthetic_with_stable_args(
            &self.lowered_hir.types,
            SyntheticStableFunctionTarget {
                fqn: template_fqn.to_string(),
                decl_file: template.template.source_path.clone(),
                decl_span: template.template.decl_span,
                abi_identity,
                stable_template_key: template.stable_template_key.clone(),
                type_args: owner_type_args,
                eff_args,
                param_tys: vec![receiver_ty],
                return_ty: Some(return_ty),
                arg_binding,
            },
        ))
    }

    fn synthetic_stable_top_level_target(
        &self,
        fqn: &str,
        args: &[CallArg],
        return_ty: TypeId,
        abi_identity: CallableAbiIdentity,
        arg_binding: Option<CallArgBindingContract>,
    ) -> Option<FunctionTargetContract> {
        let template_fqn = generic_template_base_fqn(fqn);
        let template = self
            .lowered_hir
            .generic_template_inventory
            .iter()
            .find(|template| {
                template.template.fqn == template_fqn
                    && template.owner_type_param_names.is_empty()
                    && template.owner_eff_param_name.is_none()
            })?;
        if template.function_eff_param_name.is_some() {
            return None;
        }
        let fun = self.hir_fun_decl(template_fqn)?;
        let mut bindings = HashMap::new();
        for (param, arg) in fun.params.iter().zip(args.iter()) {
            collect_type_param_bindings_in_type(
                &self.lowered_hir.types,
                param.ty,
                call_arg_value_ty(arg),
                &mut bindings,
            );
        }
        collect_type_param_bindings_in_type(
            &self.lowered_hir.types,
            fun.return_ty,
            return_ty,
            &mut bindings,
        );
        let type_args = template
            .function_type_param_names
            .iter()
            .map(|name| bindings.get(name).copied())
            .collect::<Option<Vec<_>>>()?;
        if type_args.is_empty() {
            return None;
        }

        Some(FunctionTargetContract::synthetic_with_stable_args(
            &self.lowered_hir.types,
            SyntheticStableFunctionTarget {
                fqn: template_fqn.to_string(),
                decl_file: template.template.source_path.clone(),
                decl_span: template.template.decl_span,
                abi_identity,
                stable_template_key: template.stable_template_key.clone(),
                type_args,
                eff_args: Vec::new(),
                param_tys: fun.params.iter().map(|param| param.ty).collect(),
                return_ty: Some(return_ty),
                arg_binding,
            },
        ))
    }

    fn callable_abi_identity_for_fqn(&self, fqn: &str) -> CallableAbiIdentity {
        if let Some(extern_fun) = self.lowered_hir.extern_funs.get(fqn) {
            return extern_fun.callable_abi_identity();
        }

        let call_may_suspend = self
            .hir_fun_decl(fqn)
            .is_some_and(|fun| callable_declared_effectful(&self.lowered_hir.types, fun.ty));
        CallableAbiIdentity::managed_callable(call_may_suspend)
    }

    fn callable_abi_identity_for_binding(
        &self,
        binding: &ast::TopLevelFunCallBinding,
        source_path: &Path,
        span: Span,
    ) -> Result<CallableAbiIdentity, HirStageError> {
        if let Some(extern_fun) = self.lowered_hir.extern_funs.get(&binding.fqn) {
            return Ok(extern_fun.callable_abi_identity());
        }

        let selected_fun = self
            .hir_fun_decl_at(&binding.fqn, &binding.decl_file, binding.decl_span)
            .or_else(|| self.unique_hir_fun_decl(&binding.fqn));
        if let Some(selected_fun) = selected_fun {
            return Ok(CallableAbiIdentity::managed_callable(
                callable_declared_effectful(&self.lowered_hir.types, selected_fun.ty),
            ));
        }

        if self.has_multiple_hir_fun_decls(&binding.fqn) {
            return Err(HirStageError::new(
                source_path.to_path_buf(),
                span,
                format!(
                    "call binding for `{}` is missing its selected declaration identity",
                    binding.fqn
                ),
                "typed HIR call contract",
            ));
        }

        Ok(self.callable_abi_identity_for_fqn(&binding.fqn))
    }

    fn managed_callable_abi_identity_for_ty(&self, ty: TypeId) -> CallableAbiIdentity {
        CallableAbiIdentity::managed_callable(callable_declared_effectful(
            &self.lowered_hir.types,
            ty,
        ))
    }

    fn funptr_callable_abi_identity_for_ty(&self, _ty: TypeId) -> CallableAbiIdentity {
        CallableAbiIdentity::funptr()
    }

    fn top_level_val_source_path(&self, val: &ValDecl) -> Option<PathBuf> {
        self.lowered_hir
            .top_level_vars
            .values()
            .find(|global| global.span == val.span)
            .map(|global| global.source_path.clone())
            .or_else(|| {
                self.lowered_hir
                    .top_level_immutable_values
                    .values()
                    .find(|value| value.span == val.span)
                    .map(|value| value.source_path.clone())
            })
    }

    fn collect_fun(&mut self, fun: &FunDecl) -> Result<(), HirStageError> {
        if let Some(body) = &fun.body {
            self.collect_block(&fun.source_path, body)?;
        }
        Ok(())
    }

    fn collect_block(
        &mut self,
        source_path: &Path,
        block: &crate::hir::Block,
    ) -> Result<(), HirStageError> {
        for stmt in &block.stmts {
            self.collect_stmt(source_path, stmt)?;
        }
        Ok(())
    }

    fn collect_stmt(&mut self, source_path: &Path, stmt: &Stmt) -> Result<(), HirStageError> {
        match &stmt.kind {
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
            StmtKind::Expr(expr) => self.collect_expr(source_path, expr)?,
            StmtKind::Val(val) => {
                if let Some(init) = &val.init {
                    self.collect_expr(source_path, init)?;
                }
            }
            StmtKind::Assign { lhs, rhs, .. } => {
                self.collect_expr(source_path, lhs)?;
                self.collect_expr(source_path, rhs)?;
            }
            StmtKind::While { cond, body } => {
                self.collect_expr(source_path, cond)?;
                self.collect_block(source_path, body)?;
            }
            StmtKind::Return { value } => {
                if let Some(value) = value {
                    self.collect_expr(source_path, value)?;
                }
            }
        }
        Ok(())
    }

    fn collect_expr(&mut self, source_path: &Path, expr: &Expr) -> Result<(), HirStageError> {
        match &expr.kind {
            ExprKind::Missing
            | ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::ClassLiteral(_)
            | ExprKind::Todo(_) => {}
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.collect_expr(source_path, &field.value)?;
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.collect_expr(source_path, element)?;
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        self.collect_expr(source_path, expr)?;
                    }
                }
            }
            ExprKind::Unary { expr: operand, .. } => {
                self.record_semantic_call_contract(source_path, expr.span)?;
                self.collect_expr(source_path, operand)?;
            }
            ExprKind::TypeCheck { expr: operand, .. } | ExprKind::Cast { expr: operand, .. } => {
                self.collect_expr(source_path, operand)?;
            }
            ExprKind::MemberAccess { receiver, .. } => {
                self.record_semantic_call_contract(source_path, expr.span)?;
                self.collect_expr(source_path, receiver)?;
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.record_semantic_call_contract(source_path, expr.span)?;
                self.collect_expr(source_path, lhs)?;
                self.collect_expr(source_path, rhs)?;
            }
            ExprKind::Block(block) => self.collect_block(source_path, block)?,
            ExprKind::Closure(closure) => self.collect_expr(source_path, &closure.body)?,
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_expr(source_path, cond)?;
                self.collect_expr(source_path, then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.collect_expr(source_path, else_branch)?;
                }
            }
            ExprKind::When { subject, arms } => {
                self.collect_expr(source_path, subject)?;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(source_path, guard)?;
                    }
                    self.collect_expr(source_path, &arm.body)?;
                }
            }
            ExprKind::Call { callee, args } => {
                self.record_call_contract(source_path, expr, callee, args)?;
                self.collect_expr(source_path, callee)?;
                for arg in args {
                    self.collect_call_arg_expr(source_path, arg)?;
                }
            }
            ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => {
                self.record_perform_contract(source_path, expr, *effect_ty, op, args);
                for arg in args {
                    self.collect_call_arg_expr(source_path, arg)?;
                }
            }
            ExprKind::Handle(handle) => {
                self.record_handle_contract(source_path, expr, handle);
                self.collect_block(source_path, &handle.body)?;
                for arm in &handle.arms {
                    self.collect_expr(source_path, &arm.body)?;
                }
                if let Some(finally) = &handle.finally {
                    self.collect_block(source_path, finally)?;
                }
            }
        }
        Ok(())
    }

    fn collect_call_arg_expr(
        &mut self,
        source_path: &Path,
        arg: &CallArg,
    ) -> Result<(), HirStageError> {
        match arg {
            CallArg::Positional(expr) => self.collect_expr(source_path, expr)?,
            CallArg::Named { value, .. } => self.collect_expr(source_path, value)?,
        }
        Ok(())
    }

    fn record_semantic_call_contract(
        &mut self,
        source_path: &Path,
        span: Span,
    ) -> Result<(), HirStageError> {
        let call_site = self.call_site(source_path, span);
        if self.call_site_contracts.contains_key(&call_site) {
            return Ok(());
        }
        let Some(binding) = self.lowered_hir.top_level_fun_call_sites.get(&call_site) else {
            return Ok(());
        };

        let arg_binding = self.call_arg_binding_contract(source_path, call_site.span);
        let abi_identity = self.callable_abi_identity_for_binding(binding, source_path, span)?;
        let stable_template_key = self.stable_template_key_for_binding(binding);
        let function = FunctionTargetContract::from_binding(
            &self.lowered_hir.types,
            binding,
            abi_identity,
            stable_template_key,
            arg_binding,
        );
        let contract = if binding.is_intrinsic {
            TypedCallSiteContract::Intrinsic {
                kind: TypedIntrinsicKind::from_call_binding(binding),
                function,
            }
        } else if let Some((dispatch_kind, receiver_ty)) =
            self.dispatch_kind_and_receiver_ty(source_path, span)
        {
            let (owner_fqn, member_name) =
                self.member_binding_for_fqn(&binding.fqn).ok_or_else(|| {
                    HirStageError::new(
                        source_path.to_path_buf(),
                        span,
                        format!(
                            "semantic dispatch call contract missing owner/member binding for `{}`",
                            binding.fqn
                        ),
                        "typed HIR call contract",
                    )
                })?;
            let member = MemberCallTargetContract::new(
                owner_fqn,
                member_name,
                binding.fqn.clone(),
                receiver_ty,
                function,
            );
            match dispatch_kind {
                DispatchCallKind::Virtual => TypedCallSiteContract::Virtual(member),
                DispatchCallKind::Interface => TypedCallSiteContract::Interface(member),
            }
        } else if let Some((owner_fqn, member_name)) = self.member_binding_for_fqn(&binding.fqn) {
            let receiver_ty = function.param_tys().first().copied().ok_or_else(|| {
                HirStageError::new(
                    source_path.to_path_buf(),
                    span,
                    format!(
                        "semantic member call contract for `{}` is missing receiver type",
                        binding.fqn
                    ),
                    "typed HIR call contract",
                )
            })?;
            TypedCallSiteContract::MemberDirect(MemberCallTargetContract::new(
                owner_fqn,
                member_name,
                binding.fqn.clone(),
                receiver_ty,
                function,
            ))
        } else {
            TypedCallSiteContract::DirectTopLevel(function)
        };

        self.call_site_kinds
            .insert(call_site.clone(), contract.kind());
        self.call_site_contracts.insert(call_site, contract);
        Ok(())
    }

    fn record_call_contract(
        &mut self,
        source_path: &Path,
        expr: &Expr,
        callee: &Expr,
        args: &[CallArg],
    ) -> Result<(), HirStageError> {
        let call_site = self.call_site(source_path, expr.span);
        if let Some(contract) = self.continuation_resume_contract(expr, callee, args) {
            self.continuation_resume_sites
                .insert(call_site.clone(), contract.clone());
            self.call_site_kinds
                .insert(call_site.clone(), TypedCallSiteKind::ContinuationResume);
            self.call_site_contracts.insert(
                call_site,
                TypedCallSiteContract::ContinuationResume(contract),
            );
            return Ok(());
        }

        if let Some(info) = self.lowered_hir.ctor_call_sites.get(&call_site) {
            let contract = TypedCallSiteContract::Constructor(ConstructorCallTargetContract::new(
                info.class_fqn.clone(),
                info.ctor_span,
                expr.ty,
                info.arg_mapping.clone(),
            ));
            self.call_site_kinds
                .insert(call_site.clone(), contract.kind());
            self.call_site_contracts.insert(call_site, contract);
            return Ok(());
        }

        if let Some(binding) = self.lowered_hir.top_level_fun_call_sites.get(&call_site) {
            let arg_binding = self.call_arg_binding_contract(source_path, call_site.span);
            let abi_identity =
                self.callable_abi_identity_for_binding(binding, source_path, expr.span)?;
            let stable_template_key = self.stable_template_key_for_binding(binding);
            let function = FunctionTargetContract::from_binding(
                &self.lowered_hir.types,
                binding,
                abi_identity,
                stable_template_key,
                arg_binding.clone(),
            );

            let contract = if binding.is_intrinsic {
                TypedCallSiteContract::Intrinsic {
                    kind: TypedIntrinsicKind::from_call_binding(binding),
                    function,
                }
            } else if let Some((dispatch_kind, receiver_ty)) =
                self.dispatch_kind_and_receiver_ty(source_path, expr.span)
            {
                let (owner_fqn, member_name) =
                    self.member_binding_for_fqn(&binding.fqn).ok_or_else(|| {
                        HirStageError::new(
                            source_path.to_path_buf(),
                            expr.span,
                            format!(
                                "dispatch call contract missing owner/member binding for `{}`",
                                binding.fqn
                            ),
                            "typed HIR call contract",
                        )
                    })?;
                let member = MemberCallTargetContract::new(
                    owner_fqn,
                    member_name,
                    binding.fqn.clone(),
                    receiver_ty,
                    function,
                );
                match dispatch_kind {
                    DispatchCallKind::Virtual => TypedCallSiteContract::Virtual(member),
                    DispatchCallKind::Interface => TypedCallSiteContract::Interface(member),
                }
            } else if let Some(receiver_ty) =
                receiver_ty_from_call_contract_source(callee, arg_binding.as_ref(), args)
            {
                if let Some((owner_fqn, member_name)) = self.member_binding_for_fqn(&binding.fqn) {
                    TypedCallSiteContract::MemberDirect(MemberCallTargetContract::new(
                        owner_fqn,
                        member_name,
                        binding.fqn.clone(),
                        receiver_ty,
                        function,
                    ))
                } else {
                    TypedCallSiteContract::Extension {
                        receiver_ty,
                        function,
                    }
                }
            } else {
                TypedCallSiteContract::DirectTopLevel(function)
            };

            self.call_site_kinds
                .insert(call_site.clone(), contract.kind());
            self.call_site_contracts.insert(call_site, contract);
            return Ok(());
        }

        let arg_binding = self.call_arg_binding_contract(source_path, call_site.span);
        let contract = if let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind {
            let abi_identity = self.callable_abi_identity_for_fqn(fqn);
            let function = self
                .synthetic_stable_member_target(
                    fqn,
                    args,
                    expr.ty,
                    abi_identity,
                    arg_binding.clone(),
                )
                .or_else(|| {
                    self.synthetic_stable_top_level_target(
                        fqn,
                        args,
                        expr.ty,
                        abi_identity,
                        arg_binding.clone(),
                    )
                })
                .unwrap_or_else(|| {
                    FunctionTargetContract::synthetic_with_arg_binding(
                        fqn.clone(),
                        abi_identity,
                        arg_binding.clone(),
                    )
                });
            if let Some((dispatch_kind, receiver_ty)) =
                self.dispatch_kind_and_receiver_ty(source_path, expr.span)
            {
                let (owner_fqn, member_name) = self.member_binding_for_fqn(fqn).ok_or_else(|| {
                    HirStageError::new(
                        source_path.to_path_buf(),
                        expr.span,
                        format!(
                            "synthetic dispatch call contract missing owner/member binding for `{fqn}`"
                        ),
                        "typed HIR call contract",
                    )
                })?;
                let member = MemberCallTargetContract::new(
                    owner_fqn,
                    member_name,
                    fqn.clone(),
                    receiver_ty,
                    function,
                );
                match dispatch_kind {
                    DispatchCallKind::Virtual => Some(TypedCallSiteContract::Virtual(member)),
                    DispatchCallKind::Interface => Some(TypedCallSiteContract::Interface(member)),
                }
            } else if fqn.starts_with("scoop.core.__") || fqn.starts_with("scoop.core.GC.") {
                Some(TypedCallSiteContract::Intrinsic {
                    kind: TypedIntrinsicKind::from_fqn(fqn),
                    function,
                })
            } else {
                Some(TypedCallSiteContract::DirectTopLevel(function))
            }
        } else if matches!(callee.kind, ExprKind::Closure(_)) {
            Some(TypedCallSiteContract::Closure {
                callee_ty: callee.ty,
                return_ty: expr.ty,
                abi_identity: self.managed_callable_abi_identity_for_ty(callee.ty),
                arg_binding: arg_binding.clone(),
            })
        } else if let ExprKind::UnresolvedIdent { name } = &callee.kind
            && name.starts_with("__scoop_")
        {
            let fqn = format!("scoop.core.{name}");
            Some(TypedCallSiteContract::Intrinsic {
                kind: TypedIntrinsicKind::from_fqn(&fqn),
                function: FunctionTargetContract::synthetic_with_arg_binding(
                    fqn,
                    CallableAbiIdentity::ManagedOrdinary,
                    arg_binding,
                ),
            })
        } else if let ExprKind::UnresolvedIdent { name } = &callee.kind
            && let Some(fqn) = self.unique_hir_fun_fqn_by_local_name(name)
        {
            let abi_identity = self.callable_abi_identity_for_fqn(&fqn);
            let function = self
                .synthetic_stable_top_level_target(
                    &fqn,
                    args,
                    expr.ty,
                    abi_identity,
                    arg_binding.clone(),
                )
                .unwrap_or_else(|| {
                    FunctionTargetContract::synthetic_with_arg_binding(
                        fqn.clone(),
                        abi_identity,
                        arg_binding.clone(),
                    )
                });
            Some(TypedCallSiteContract::DirectTopLevel(function))
        } else if is_funptr_ty(&self.lowered_hir.types, callee.ty) {
            Some(TypedCallSiteContract::FunPtr {
                callee_ty: callee.ty,
                return_ty: expr.ty,
                abi_identity: self.funptr_callable_abi_identity_for_ty(callee.ty),
                arg_binding: arg_binding.clone(),
            })
        } else if let Some(fqn) = gc_member_intrinsic_fqn(callee) {
            Some(TypedCallSiteContract::Intrinsic {
                kind: TypedIntrinsicKind::from_fqn(&fqn),
                function: FunctionTargetContract::synthetic_with_arg_binding(
                    fqn,
                    CallableAbiIdentity::ManagedOrdinary,
                    arg_binding,
                ),
            })
        } else if let Some((owner_fqn, member_name, fqn, receiver_ty)) =
            resolved_member_call_binding(callee)
        {
            // 合成的 member-access call（例如 generic delegated property 或 f-string ToString）
            // 在 lowering 时已写回 `MemberRef::Fun { fqn }`；这里据此发布 typed contract，
            // 让 MIR / effect-facts / late-lowering 能像普通 source-level member 调用一样消费。
            let abi_identity = self.callable_abi_identity_for_fqn(&fqn);
            let function = FunctionTargetContract::synthetic_with_arg_binding(
                fqn.clone(),
                abi_identity,
                arg_binding,
            );
            let member =
                MemberCallTargetContract::new(owner_fqn, member_name, fqn, receiver_ty, function);
            match self.dispatch_kind_and_receiver_ty(source_path, expr.span) {
                Some((DispatchCallKind::Virtual, _)) => {
                    Some(TypedCallSiteContract::Virtual(member))
                }
                Some((DispatchCallKind::Interface, _)) => {
                    Some(TypedCallSiteContract::Interface(member))
                }
                None => Some(TypedCallSiteContract::MemberDirect(member)),
            }
        } else if is_function_ty(&self.lowered_hir.types, callee.ty)
            || matches!(callee.kind, ExprKind::VarRef(ValueRef::Local { .. }))
        {
            Some(TypedCallSiteContract::FunValue {
                callee_ty: callee.ty,
                return_ty: expr.ty,
                abi_identity: self.managed_callable_abi_identity_for_ty(callee.ty),
                arg_binding,
            })
        } else {
            None
        };

        if let Some(contract) = contract {
            self.call_site_kinds
                .insert(call_site.clone(), contract.kind());
            self.call_site_contracts.insert(call_site, contract);
            return Ok(());
        }

        if self.call_may_lower_without_typed_contract(expr, callee) {
            return Ok(());
        }

        Err(HirStageError::new(
            source_path.to_path_buf(),
            expr.span,
            "call expression missing typed call-site contract",
            "typed HIR call contract",
        ))
    }

    fn call_may_lower_without_typed_contract(&self, expr: &Expr, callee: &Expr) -> bool {
        let ExprKind::UnresolvedIdent { name } = &callee.kind else {
            return false;
        };

        if self.unresolved_callee_is_enum_variant(name) {
            return true;
        }

        if self.unresolved_callee_is_nominal_type_ctor(name) {
            return true;
        }

        match self.lowered_hir.types.kind(expr.ty) {
            // Some class-init internal helpers still arrive as unresolved/Any; they
            // cannot publish stable targets, and generic requests still fail later.
            TypeKind::Ref(RefTypeKind::Any) => true,
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                self.unresolved_callee_is_nominal_ctor(name, nominal)
            }
            TypeKind::Value(ValueTypeKind::Option(_)) => true,
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                self.unresolved_callee_is_nominal_ctor(name, nominal)
                    || self
                        .lowered_hir
                        .nominal_kinds
                        .get(&nominal.fqn)
                        .is_some_and(|kind| *kind == ast::TypeKind::Enum)
            }
            _ => false,
        }
    }

    fn unresolved_callee_is_nominal_ctor(&self, name: &str, nominal: &NominalType) -> bool {
        let Some(local_name) = nominal.fqn.rsplit('.').next() else {
            return false;
        };
        local_name == name
    }

    fn unresolved_callee_is_nominal_type_ctor(&self, name: &str) -> bool {
        self.lowered_hir.nominal_kinds.iter().any(|(fqn, kind)| {
            matches!(kind, ast::TypeKind::Class | ast::TypeKind::Struct)
                && fqn.rsplit('.').next() == Some(name)
        })
    }

    fn unresolved_callee_is_enum_variant(&self, name: &str) -> bool {
        self.lowered_hir
            .enum_layouts
            .values()
            .any(|layout| layout.variants.iter().any(|variant| variant.name == name))
    }

    fn call_arg_binding_contract(
        &self,
        source_path: &Path,
        span: Span,
    ) -> Option<CallArgBindingContract> {
        self.lowered_hir
            .call_arg_bindings
            .get(&self.call_site(source_path, span))
            .map(|binding| CallArgBindingContract {
                params: binding
                    .params
                    .iter()
                    .map(|param| match param {
                        ast::CallArgParamBinding::Receiver => CallArgParamContract::Receiver,
                        ast::CallArgParamBinding::Explicit(element) => {
                            CallArgParamContract::Explicit(CallArgElementContract {
                                arg_index: element.arg_index,
                                spread: element.spread,
                            })
                        }
                        ast::CallArgParamBinding::Default => CallArgParamContract::Default,
                        ast::CallArgParamBinding::Vararg(elements) => CallArgParamContract::Vararg(
                            elements
                                .iter()
                                .map(|element| CallArgElementContract {
                                    arg_index: element.arg_index,
                                    spread: element.spread,
                                })
                                .collect(),
                        ),
                    })
                    .collect(),
            })
    }

    fn dispatch_kind_and_receiver_ty(
        &self,
        source_path: &Path,
        span: Span,
    ) -> Option<(DispatchCallKind, TypeId)> {
        self.lowered_hir
            .dispatch_call_sites
            .iter()
            .find(|(site, _)| site.source_path == source_path && site.span == span)
            .map(|(site, kind)| (*kind, site.receiver_ty))
    }

    fn member_binding_for_fqn(&self, fqn: &str) -> Option<(String, String)> {
        let mut owners = self
            .lowered_hir
            .nominal_kinds
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        collect_decl_owner_fqns(&self.lowered_hir.file.decls, &mut owners);
        owners.sort_by_key(|owner| std::cmp::Reverse(owner.len()));
        owners.into_iter().find_map(|owner| {
            let suffix = fqn.strip_prefix(&owner)?.strip_prefix('.')?;
            (!suffix.is_empty()).then(|| (owner, suffix.to_string()))
        })
    }

    fn record_perform_contract(
        &mut self,
        source_path: &Path,
        expr: &Expr,
        effect_ty: TypeId,
        op: &crate::hir::EffectOpRef,
        args: &[CallArg],
    ) {
        let call_site = self.call_site(source_path, expr.span);
        let info = self.lowered_hir.effect_op_call_sites.get(&call_site);
        let arg_mapping = info
            .map(|binding| binding.arg_mapping.clone())
            .unwrap_or_else(|| (0..args.len()).collect());
        let payload_components = args.iter().map(call_arg_value_ty).collect::<Vec<_>>();
        let payload_ty = match payload_components.as_slice() {
            [] => Some(self.lowered_hir.builtins.unit),
            [single] => Some(*single),
            _ => info.and_then(|binding| binding.payload_tuple_ty),
        };

        let contract = PerformSiteContract::new(
            effect_ty,
            op.fqn.clone(),
            expr.ty,
            PayloadTypeContract::new(payload_ty, payload_components),
            arg_mapping,
        );
        self.perform_sites
            .insert(call_site.clone(), contract.clone());
        self.call_site_kinds
            .insert(call_site.clone(), TypedCallSiteKind::EffectOp);
        self.call_site_contracts
            .insert(call_site, TypedCallSiteContract::EffectOp(contract));
    }

    fn record_handle_contract(
        &mut self,
        source_path: &Path,
        expr: &Expr,
        handle: &crate::hir::HandleExpr,
    ) {
        let arm_contracts = handle
            .arms
            .iter()
            .map(|arm| {
                let payload_components = arm
                    .op
                    .binders
                    .iter()
                    .map(|binder| binder.ty)
                    .collect::<Vec<_>>();
                let payload_ty = match payload_components.as_slice() {
                    [] => Some(self.lowered_hir.builtins.unit),
                    [single] => Some(*single),
                    _ => self
                        .lowered_hir
                        .handle_payload_tuple_tys
                        .get(&self.call_site(source_path, arm.op.span))
                        .copied(),
                };
                let kind = match arm.kind {
                    HandleArmKind::NonResuming => HandleArmContractKind::NonResuming,
                    HandleArmKind::EscapeContinuation { .. } => {
                        HandleArmContractKind::EscapeContinuation
                    }
                };

                HandleArmSiteContract::new(
                    arm.op.effect_ty,
                    arm.op.op.fqn.clone(),
                    PayloadTypeContract::new(payload_ty, payload_components),
                    arm.body.ty,
                    kind,
                )
            })
            .collect::<Vec<_>>();

        self.handle_sites.insert(
            self.call_site(source_path, expr.span),
            HandleSiteContract::new(
                expr.ty,
                handle.body.ty,
                arm_contracts,
                handle.finally.as_ref().map(|finally| finally.ty),
            ),
        );
    }

    fn continuation_resume_contract(
        &self,
        expr: &Expr,
        callee: &Expr,
        args: &[CallArg],
    ) -> Option<ContinuationResumeSiteContract> {
        let (receiver_route, receiver_ty, payload_arg_indices) = match &callee.kind {
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. })
                if fqn == "scoop.core.Continuation.resume" =>
            {
                let receiver = args.first()?;
                (
                    ContinuationResumeReceiverRoute::CallArg { index: 0 },
                    call_arg_value_ty(receiver),
                    (1..args.len()).collect(),
                )
            }
            ExprKind::MemberAccess { receiver, member }
                if member.name == "resume"
                    && matches!(
                        member.resolved.as_ref(),
                        Some(crate::hir::MemberRef::Fun { fqn, .. })
                            if fqn == "scoop.core.Continuation.resume"
                    ) =>
            {
                (
                    ContinuationResumeReceiverRoute::MemberReceiver,
                    receiver.ty,
                    (0..args.len()).collect(),
                )
            }
            _ => return None,
        };

        let (resume_ty, answer_ty, out_effects) =
            continuation_receiver_contract(&self.lowered_hir.types, receiver_ty)?;

        Some(ContinuationResumeSiteContract {
            receiver_route,
            payload_arg_indices,
            receiver_ty,
            resume_ty,
            answer_ty,
            return_ty: expr.ty,
            out_effects,
            runtime_error_effect_ty: self.runtime_error_effect_ty,
        })
    }

    fn call_site(&self, source_path: &Path, span: Span) -> CallSite {
        CallSite::new(source_path.to_path_buf(), span)
    }
}

fn call_arg_value_ty(arg: &CallArg) -> TypeId {
    match arg {
        CallArg::Positional(expr) => expr.ty,
        CallArg::Named { value, .. } => value.ty,
    }
}

fn generic_template_base_fqn(fqn: &str) -> &str {
    let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn receiver_ty_from_call_contract_source(
    callee: &Expr,
    binding: Option<&CallArgBindingContract>,
    args: &[CallArg],
) -> Option<TypeId> {
    let binding = binding?;
    for param in binding.params() {
        if matches!(param, CallArgParamContract::Receiver) {
            return match &callee.kind {
                ExprKind::MemberAccess { receiver, .. } => Some(receiver.ty),
                _ => args.first().map(call_arg_value_ty),
            };
        }
    }
    None
}

fn gc_member_intrinsic_fqn(callee: &Expr) -> Option<String> {
    let ExprKind::MemberAccess { member, .. } = &callee.kind else {
        return None;
    };
    let crate::hir::MemberRef::Fun { fqn, .. } = member.resolved.as_ref()? else {
        return None;
    };
    fqn.starts_with("scoop.core.GC.").then(|| fqn.clone())
}

/// 当 HIR lowering 把合成 member call callee 已经 resolve 成具体 `MemberRef::Fun { fqn }` 时，
/// 把 owner FQN / member 名 / 完整 FQN / receiver type 拆出来，让上游可以直接发布 typed
/// call-site contract。
fn resolved_member_call_binding(callee: &Expr) -> Option<(String, String, String, TypeId)> {
    let ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        return None;
    };
    let crate::hir::MemberRef::Fun { fqn, .. } = member.resolved.as_ref()? else {
        return None;
    };
    if fqn.starts_with("scoop.core.GC.") {
        return None;
    }
    let (owner_fqn, member_name) = match fqn.rsplit_once('.') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() => {
            (owner.to_string(), name.to_string())
        }
        _ => return None,
    };
    Some((owner_fqn, member_name, fqn.clone(), receiver.ty))
}

fn collect_decl_owner_fqns(decls: &[Decl], owners: &mut Vec<String>) {
    for decl in decls {
        match decl {
            Decl::TypeAlias(_) | Decl::ExtensionProperty(_) => {}
            Decl::Nominal(nominal) => {
                owners.push(nominal.fqn.clone());
                collect_decl_member_owner_fqns(&nominal.members, owners);
            }
            Decl::Object(object) => {
                owners.push(object.fqn.clone());
                collect_decl_member_owner_fqns(&object.members, owners);
            }
        }
    }
}

fn collect_decl_member_owner_fqns(members: &[crate::hir::DeclMember], owners: &mut Vec<String>) {
    for member in members {
        if let crate::hir::DeclMember::Nested(nested) = member {
            collect_decl_owner_fqns(std::slice::from_ref(nested), owners);
        }
    }
}

fn is_function_ty(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

fn is_funptr_ty(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if nominal.fqn == "scoop.unsafe.FunPtr" && nominal.args.len() == 1
    )
}

fn type_id_in_store(types: &TypeStore, ty: TypeId) -> bool {
    (ty.as_u32() as usize) < types.len()
}

fn instance_args_are_concrete(
    types: &TypeStore,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> bool {
    type_args.iter().all(|&ty| !type_contains_param(types, ty))
        && eff_args
            .iter()
            .all(|row| row.terms.iter().all(|&ty| !type_contains_param(types, ty)))
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

fn collect_type_param_bindings_in_type(
    types: &TypeStore,
    pattern_ty: TypeId,
    concrete_ty: TypeId,
    bindings: &mut HashMap<String, TypeId>,
) {
    match (types.kind(pattern_ty), types.kind(concrete_ty)) {
        (TypeKind::Param(param), _) if !type_contains_param(types, concrete_ty) => {
            bindings.entry(param.name.clone()).or_insert(concrete_ty);
        }
        (
            TypeKind::Ref(RefTypeKind::Nominal(pattern))
            | TypeKind::Value(ValueTypeKind::Nominal(pattern)),
            TypeKind::Ref(RefTypeKind::Nominal(concrete))
            | TypeKind::Value(ValueTypeKind::Nominal(concrete)),
        ) if pattern.fqn == concrete.fqn => {
            for (&pattern_arg, &concrete_arg) in pattern.args.iter().zip(concrete.args.iter()) {
                collect_type_param_bindings_in_type(types, pattern_arg, concrete_arg, bindings);
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Option(pattern)),
            TypeKind::Value(ValueTypeKind::Option(concrete)),
        ) => {
            collect_type_param_bindings_in_type(types, *pattern, *concrete, bindings);
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(pattern)),
            TypeKind::Value(ValueTypeKind::Tuple(concrete)),
        ) => {
            for (&pattern_ty, &concrete_ty) in pattern.iter().zip(concrete.iter()) {
                collect_type_param_bindings_in_type(types, pattern_ty, concrete_ty, bindings);
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(pattern)),
            TypeKind::Ref(RefTypeKind::Function(concrete)),
        ) => {
            if let (Some(pattern_receiver), Some(concrete_receiver)) =
                (pattern.receiver, concrete.receiver)
            {
                collect_type_param_bindings_in_type(
                    types,
                    pattern_receiver,
                    concrete_receiver,
                    bindings,
                );
            }
            for (&pattern_param, &concrete_param) in
                pattern.params.iter().zip(concrete.params.iter())
            {
                collect_type_param_bindings_in_type(types, pattern_param, concrete_param, bindings);
            }
            collect_type_param_bindings_in_type(
                types,
                pattern.return_ty,
                concrete.return_ty,
                bindings,
            );
        }
        _ => {}
    }
}

fn function_effect_contract(types: &TypeStore, fun_ty: TypeId) -> Option<(EffectRow, bool)> {
    let TypeKind::Ref(RefTypeKind::Function(function)) = types.kind(fun_ty) else {
        return None;
    };

    Some((function.effects.clone(), function.effects_closed))
}

fn callable_declared_effectful(types: &TypeStore, callable_ty: TypeId) -> bool {
    function_effect_contract(types, callable_ty)
        .is_some_and(|(effects, _effects_closed)| !effects.is_pure())
}

fn find_raise_runtime_error_effect(types: &TypeStore) -> Option<TypeId> {
    let runtime_error_ty = find_nominal_type_by_fqn(types, "scoop.core.RuntimeError")?;

    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Raise"
                    && nominal.args.as_slice() == [runtime_error_ty]
        )
    })
}

fn ensure_raise_runtime_error_effect(types: &mut TypeStore) -> TypeId {
    if let Some(effect_ty) = find_raise_runtime_error_effect(types) {
        return effect_ty;
    }
    let runtime_error_ty = find_nominal_type_by_fqn(types, "scoop.core.RuntimeError")
        .unwrap_or_else(|| {
            types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: "scoop.core.RuntimeError".to_string(),
                args: Vec::new(),
                eff: None,
            })))
        });
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: "scoop.core.Raise".to_string(),
        args: vec![runtime_error_ty],
        eff: None,
    })))
}

fn find_nominal_type_by_fqn(types: &TypeStore, fqn: &str) -> Option<TypeId> {
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == fqn
        ) || matches!(
            types.kind(id),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) if nominal.fqn == fqn
        )
    })
}

fn continuation_receiver_contract(
    types: &TypeStore,
    receiver_ty: TypeId,
) -> Option<(TypeId, TypeId, EffectRow)> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
        return None;
    };
    if nominal.fqn != "scoop.core.Continuation" || nominal.args.len() < 2 {
        return None;
    }

    Some((
        nominal.args[0],
        nominal.args[1],
        nominal.eff.clone().unwrap_or_else(EffectRow::pure),
    ))
}

fn collect_top_level_init_roots(lowered_hir: &LoweredHir) -> Vec<TopLevelInitRootContract> {
    let dependency_kinds = top_level_dependency_kinds(lowered_hir);
    let dependency_ctx = TopLevelDependencyContext::new(lowered_hir, &dependency_kinds);
    let lowered_object_fqns = lowered_object_decl_fqns(lowered_hir);
    let mut roots = Vec::new();

    for value in lowered_hir.top_level_immutable_values.values() {
        roots.push(TopLevelInitRootContract {
            fqn: value.fqn.clone(),
            source_path: value.source_path.clone(),
            span: value.span,
            kind: TopLevelInitRootKind::RuntimeImmutableVal,
            ty: Some(value.ty),
            initializer_ty: value.init.as_ref().map(|init| init.ty),
            has_initializer: value.init.is_some(),
            dependencies: dependencies_for_expr(
                value.fqn.as_str(),
                value.source_path.as_path(),
                value.init.as_ref(),
                &dependency_ctx,
            ),
        });
    }

    for var in lowered_hir.top_level_vars.values() {
        roots.push(TopLevelInitRootContract {
            fqn: var.fqn.clone(),
            source_path: var.source_path.clone(),
            span: var.span,
            kind: TopLevelInitRootKind::RuntimeMutableVar {
                storage: var.storage,
            },
            ty: Some(var.ty),
            initializer_ty: var.init.as_ref().map(|init| init.ty),
            has_initializer: var.init.is_some(),
            dependencies: dependencies_for_expr(
                var.fqn.as_str(),
                var.source_path.as_path(),
                var.init.as_ref(),
                &dependency_ctx,
            ),
        });
    }

    for object in lowered_hir
        .object_inits
        .values()
        .filter(|object| lowered_object_fqns.contains(&object.fqn))
    {
        roots.push(TopLevelInitRootContract {
            fqn: object.fqn.clone(),
            source_path: object.source_path.clone(),
            span: object.span,
            kind: TopLevelInitRootKind::ObjectSingleton,
            ty: None,
            initializer_ty: None,
            has_initializer: !object.steps.is_empty(),
            dependencies: dependencies_for_object(
                object.fqn.as_str(),
                object.source_path.as_path(),
                object,
                &dependency_ctx,
            ),
        });
    }

    roots.sort_by(|lhs, rhs| {
        lhs.fqn
            .cmp(&rhs.fqn)
            .then(lhs.span.start.cmp(&rhs.span.start))
    });
    roots
}

fn lowered_object_decl_fqns(lowered_hir: &LoweredHir) -> HashSet<String> {
    let mut out = HashSet::new();
    for decl in &lowered_hir.file.decls {
        collect_object_decl_fqns(decl, &mut out);
    }
    out
}

fn collect_object_decl_fqns(decl: &Decl, out: &mut HashSet<String>) {
    match decl {
        Decl::Object(object) => {
            out.insert(object.fqn.clone());
            for member in &object.members {
                if let crate::hir::DeclMember::Nested(nested) = member {
                    collect_object_decl_fqns(nested, out);
                }
            }
        }
        Decl::Nominal(nominal) => {
            for member in &nominal.members {
                if let crate::hir::DeclMember::Nested(nested) = member {
                    collect_object_decl_fqns(nested, out);
                }
            }
        }
        Decl::TypeAlias(_) | Decl::ExtensionProperty(_) => {}
    }
}

fn collect_extern_global_contracts(lowered_hir: &LoweredHir) -> Vec<ExternGlobalContract> {
    let mut contracts = lowered_hir
        .extern_globals
        .values()
        .map(ExternGlobalContract::from_hir)
        .collect::<Vec<_>>();
    contracts.sort_by(|lhs, rhs| {
        lhs.fqn
            .cmp(&rhs.fqn)
            .then(lhs.span.start.cmp(&rhs.span.start))
    });
    contracts
}

fn top_level_dependency_kinds(
    lowered_hir: &LoweredHir,
) -> HashMap<String, TopLevelInitDependencyKind> {
    let mut out = HashMap::new();
    for fqn in lowered_hir.top_level_immutable_values.keys() {
        out.insert(fqn.clone(), TopLevelInitDependencyKind::TopLevelValue);
    }
    for fqn in lowered_hir.top_level_vars.keys() {
        out.insert(fqn.clone(), TopLevelInitDependencyKind::TopLevelValue);
    }
    for fqn in lowered_hir.object_inits.keys() {
        out.insert(fqn.clone(), TopLevelInitDependencyKind::ObjectSingleton);
    }
    out
}

struct TopLevelDependencyContext<'a> {
    dependency_kinds: &'a HashMap<String, TopLevelInitDependencyKind>,
    function_bodies: HashMap<&'a str, (&'a Path, &'a Block)>,
    call_sites: &'a crate::hir::TopLevelFunCallSiteIndex,
}

impl<'a> TopLevelDependencyContext<'a> {
    fn new(
        lowered_hir: &'a LoweredHir,
        dependency_kinds: &'a HashMap<String, TopLevelInitDependencyKind>,
    ) -> Self {
        let mut function_bodies = HashMap::new();
        for item in &lowered_hir.file.items {
            if let Item::Fun(fun) = item
                && let Some(body) = &fun.body
            {
                function_bodies.insert(fun.fqn.as_str(), (fun.source_path.as_path(), body));
            }
        }
        for fun in &lowered_hir.member_funs {
            if let Some(body) = &fun.body {
                function_bodies.insert(fun.fqn.as_str(), (fun.source_path.as_path(), body));
            }
        }

        Self {
            dependency_kinds,
            function_bodies,
            call_sites: &lowered_hir.top_level_fun_call_sites,
        }
    }
}

fn dependencies_for_expr(
    owner_fqn: &str,
    source_path: &Path,
    expr: Option<&Expr>,
    ctx: &TopLevelDependencyContext<'_>,
) -> Vec<TopLevelInitDependency> {
    let mut out = Vec::new();
    if let Some(expr) = expr {
        collect_expr_dependencies(
            expr,
            owner_fqn,
            source_path,
            ctx,
            &mut HashSet::new(),
            &mut out,
        );
    }
    stable_dependencies(out)
}

fn dependencies_for_object(
    owner_fqn: &str,
    source_path: &Path,
    object: &crate::hir::ObjectInit,
    ctx: &TopLevelDependencyContext<'_>,
) -> Vec<TopLevelInitDependency> {
    let mut out = Vec::new();
    let mut visiting = HashSet::new();
    for step in &object.steps {
        match step {
            crate::hir::ObjectInitStep::PropertyInit { init, .. } => {
                collect_expr_dependencies(
                    init,
                    owner_fqn,
                    source_path,
                    ctx,
                    &mut visiting,
                    &mut out,
                );
            }
            crate::hir::ObjectInitStep::InitBlock { block } => {
                collect_block_dependencies(
                    block,
                    owner_fqn,
                    source_path,
                    ctx,
                    &mut visiting,
                    &mut out,
                );
            }
        }
    }
    stable_dependencies(out)
}

fn stable_dependencies(
    mut dependencies: Vec<TopLevelInitDependency>,
) -> Vec<TopLevelInitDependency> {
    dependencies.sort_by(|lhs, rhs| {
        lhs.fqn
            .cmp(&rhs.fqn)
            .then((lhs.kind as u8).cmp(&(rhs.kind as u8)))
    });
    dependencies.dedup_by(|lhs, rhs| lhs.fqn == rhs.fqn && lhs.kind == rhs.kind);
    dependencies
}

fn push_top_level_dependency(
    fqn: &str,
    owner_fqn: &str,
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    if fqn == owner_fqn {
        return;
    }
    if let Some(kind) = dependency_kinds.get(fqn).copied() {
        out.push(TopLevelInitDependency::new(fqn.to_string(), kind));
    }
}

fn collect_block_dependencies(
    block: &Block,
    owner_fqn: &str,
    source_path: &Path,
    ctx: &TopLevelDependencyContext<'_>,
    visiting: &mut HashSet<String>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    for stmt in &block.stmts {
        collect_stmt_dependencies(stmt, owner_fqn, source_path, ctx, visiting, out);
    }
}

fn collect_stmt_dependencies(
    stmt: &Stmt,
    owner_fqn: &str,
    source_path: &Path,
    ctx: &TopLevelDependencyContext<'_>,
    visiting: &mut HashSet<String>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => {
            collect_expr_dependencies(expr, owner_fqn, source_path, ctx, visiting, out)
        }
        StmtKind::Val(val) => {
            if let Some(init) = &val.init {
                collect_expr_dependencies(init, owner_fqn, source_path, ctx, visiting, out);
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_expr_dependencies(lhs, owner_fqn, source_path, ctx, visiting, out);
            collect_expr_dependencies(rhs, owner_fqn, source_path, ctx, visiting, out);
        }
        StmtKind::While { cond, body } => {
            collect_expr_dependencies(cond, owner_fqn, source_path, ctx, visiting, out);
            collect_block_dependencies(body, owner_fqn, source_path, ctx, visiting, out);
        }
        StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_expr_dependencies(value, owner_fqn, source_path, ctx, visiting, out);
            }
        }
    }
}

fn collect_expr_dependencies(
    expr: &Expr,
    owner_fqn: &str,
    source_path: &Path,
    ctx: &TopLevelDependencyContext<'_>,
    visiting: &mut HashSet<String>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Todo(_) => {}
        ExprKind::VarRef(ValueRef::Local { .. }) => {}
        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
            push_top_level_dependency(fqn, owner_fqn, ctx.dependency_kinds, out);
        }
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_expr_dependencies(&field.value, owner_fqn, source_path, ctx, visiting, out);
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_expr_dependencies(element, owner_fqn, source_path, ctx, visiting, out);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_expr_dependencies(expr, owner_fqn, source_path, ctx, visiting, out);
                }
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => {
            collect_expr_dependencies(expr, owner_fqn, source_path, ctx, visiting, out);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_dependencies(lhs, owner_fqn, source_path, ctx, visiting, out);
            collect_expr_dependencies(rhs, owner_fqn, source_path, ctx, visiting, out);
        }
        ExprKind::Block(block) => {
            collect_block_dependencies(block, owner_fqn, source_path, ctx, visiting, out)
        }
        ExprKind::Closure(_) => {}
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_dependencies(cond, owner_fqn, source_path, ctx, visiting, out);
            collect_expr_dependencies(then_branch, owner_fqn, source_path, ctx, visiting, out);
            if let Some(else_branch) = else_branch {
                collect_expr_dependencies(else_branch, owner_fqn, source_path, ctx, visiting, out);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_expr_dependencies(subject, owner_fqn, source_path, ctx, visiting, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_dependencies(guard, owner_fqn, source_path, ctx, visiting, out);
                }
                collect_expr_dependencies(&arm.body, owner_fqn, source_path, ctx, visiting, out);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            collect_expr_dependencies(receiver, owner_fqn, source_path, ctx, visiting, out);
        }
        ExprKind::Call { callee, args } => {
            collect_expr_dependencies(callee, owner_fqn, source_path, ctx, visiting, out);
            collect_call_arg_dependencies(args, owner_fqn, source_path, ctx, visiting, out);
            collect_direct_call_dependencies(expr, owner_fqn, source_path, ctx, visiting, out);
        }
        ExprKind::Perform { args, .. } => {
            collect_call_arg_dependencies(args, owner_fqn, source_path, ctx, visiting, out);
        }
        ExprKind::Handle(handle) => {
            collect_block_dependencies(&handle.body, owner_fqn, source_path, ctx, visiting, out);
            for arm in &handle.arms {
                collect_expr_dependencies(&arm.body, owner_fqn, source_path, ctx, visiting, out);
            }
            if let Some(finally) = &handle.finally {
                collect_block_dependencies(finally, owner_fqn, source_path, ctx, visiting, out);
            }
        }
    }
}

fn collect_call_arg_dependencies(
    args: &[CallArg],
    owner_fqn: &str,
    source_path: &Path,
    ctx: &TopLevelDependencyContext<'_>,
    visiting: &mut HashSet<String>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    for arg in args {
        match arg {
            CallArg::Positional(expr) => {
                collect_expr_dependencies(expr, owner_fqn, source_path, ctx, visiting, out);
            }
            CallArg::Named { value, .. } => {
                collect_expr_dependencies(value, owner_fqn, source_path, ctx, visiting, out);
            }
        }
    }
}

fn collect_direct_call_dependencies(
    expr: &Expr,
    owner_fqn: &str,
    source_path: &Path,
    ctx: &TopLevelDependencyContext<'_>,
    visiting: &mut HashSet<String>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    let site = CallSite::new(source_path.to_path_buf(), expr.span);
    let Some(binding) = ctx.call_sites.get(&site) else {
        return;
    };
    let target_fqn = binding.fqn.as_str();
    if !visiting.insert(target_fqn.to_string()) {
        return;
    }
    if let Some((target_source_path, body)) = ctx.function_bodies.get(target_fqn) {
        collect_block_dependencies(body, owner_fqn, target_source_path, ctx, visiting, out);
    }
    visiting.remove(target_fqn);
}

fn collect_when_pat_binding_names(
    lowered_hir: &LoweredHir,
) -> HashMap<crate::hir::WhenPatBindingSite, String> {
    let mut names = HashMap::new();
    for item in &lowered_hir.file.items {
        match item {
            Item::Fun(fun) => collect_when_pat_binding_names_from_fun(fun, &mut names),
            Item::Val(val) => {
                if let Some(init) = &val.init {
                    let source_path = top_level_val_source_path(lowered_hir, val)
                        .unwrap_or_else(|| PathBuf::from("<unknown>"));
                    collect_when_pat_binding_names_from_expr(&source_path, init, &mut names);
                }
            }
            Item::Todo { .. } => {}
        }
    }
    for fun in &lowered_hir.member_funs {
        collect_when_pat_binding_names_from_fun(fun, &mut names);
    }
    names
}

fn top_level_val_source_path(lowered_hir: &LoweredHir, val: &ValDecl) -> Option<PathBuf> {
    lowered_hir
        .top_level_vars
        .values()
        .find(|global| global.span == val.span)
        .map(|global| global.source_path.clone())
        .or_else(|| {
            lowered_hir
                .top_level_immutable_values
                .values()
                .find(|value| value.span == val.span)
                .map(|value| value.source_path.clone())
        })
}

fn collect_when_pat_binding_names_from_fun(
    fun: &FunDecl,
    names: &mut HashMap<crate::hir::WhenPatBindingSite, String>,
) {
    if let Some(body) = &fun.body {
        collect_when_pat_binding_names_from_block(&fun.source_path, body, names);
    }
}

fn collect_when_pat_binding_names_from_block(
    source_path: &Path,
    block: &Block,
    names: &mut HashMap<crate::hir::WhenPatBindingSite, String>,
) {
    for stmt in &block.stmts {
        collect_when_pat_binding_names_from_stmt(source_path, stmt, names);
    }
}

fn collect_when_pat_binding_names_from_stmt(
    source_path: &Path,
    stmt: &Stmt,
    names: &mut HashMap<crate::hir::WhenPatBindingSite, String>,
) {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => collect_when_pat_binding_names_from_expr(source_path, expr, names),
        StmtKind::Val(val) => {
            if let Some(init) = &val.init {
                collect_when_pat_binding_names_from_expr(source_path, init, names);
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_when_pat_binding_names_from_expr(source_path, lhs, names);
            collect_when_pat_binding_names_from_expr(source_path, rhs, names);
        }
        StmtKind::While { cond, body } => {
            collect_when_pat_binding_names_from_expr(source_path, cond, names);
            collect_when_pat_binding_names_from_block(source_path, body, names);
        }
        StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_when_pat_binding_names_from_expr(source_path, value, names);
            }
        }
    }
}

fn collect_when_pat_binding_names_from_expr(
    source_path: &Path,
    expr: &Expr,
    names: &mut HashMap<crate::hir::WhenPatBindingSite, String>,
) {
    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Todo(_) => {}
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_when_pat_binding_names_from_expr(source_path, &field.value, names);
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_when_pat_binding_names_from_expr(source_path, element, names);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_when_pat_binding_names_from_expr(source_path, expr, names);
                }
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => {
            collect_when_pat_binding_names_from_expr(source_path, expr, names);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_when_pat_binding_names_from_expr(source_path, lhs, names);
            collect_when_pat_binding_names_from_expr(source_path, rhs, names);
        }
        ExprKind::Block(block) => {
            collect_when_pat_binding_names_from_block(source_path, block, names)
        }
        ExprKind::Closure(closure) => {
            collect_when_pat_binding_names_from_expr(source_path, &closure.body, names);
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_when_pat_binding_names_from_expr(source_path, cond, names);
            collect_when_pat_binding_names_from_expr(source_path, then_branch, names);
            if let Some(else_branch) = else_branch {
                collect_when_pat_binding_names_from_expr(source_path, else_branch, names);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_when_pat_binding_names_from_expr(source_path, subject, names);
            for arm in arms {
                collect_when_pat_binding_names_from_pat(source_path, &arm.pat, names);
                if let Some(guard) = &arm.guard {
                    collect_when_pat_binding_names_from_expr(source_path, guard, names);
                }
                collect_when_pat_binding_names_from_expr(source_path, &arm.body, names);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            collect_when_pat_binding_names_from_expr(source_path, receiver, names);
        }
        ExprKind::Call { callee, args } => {
            collect_when_pat_binding_names_from_expr(source_path, callee, names);
            collect_when_pat_binding_names_from_args(source_path, args, names);
        }
        ExprKind::Perform { args, .. } => {
            collect_when_pat_binding_names_from_args(source_path, args, names);
        }
        ExprKind::Handle(handle) => {
            collect_when_pat_binding_names_from_block(source_path, &handle.body, names);
            for arm in &handle.arms {
                collect_when_pat_binding_names_from_expr(source_path, &arm.body, names);
            }
            if let Some(finally) = &handle.finally {
                collect_when_pat_binding_names_from_block(source_path, finally, names);
            }
        }
    }
}

fn collect_when_pat_binding_names_from_args(
    source_path: &Path,
    args: &[CallArg],
    names: &mut HashMap<crate::hir::WhenPatBindingSite, String>,
) {
    for arg in args {
        match arg {
            CallArg::Positional(expr) => {
                collect_when_pat_binding_names_from_expr(source_path, expr, names);
            }
            CallArg::Named { value, .. } => {
                collect_when_pat_binding_names_from_expr(source_path, value, names);
            }
        }
    }
}

fn collect_when_pat_binding_names_from_pat(
    source_path: &Path,
    pat: &crate::hir::WhenPat,
    names: &mut HashMap<crate::hir::WhenPatBindingSite, String>,
) {
    match pat {
        crate::hir::WhenPat::Bind { span, name, .. } => {
            names.insert(
                crate::hir::WhenPatBindingSite {
                    source_path: source_path.to_path_buf(),
                    decl_span: *span,
                },
                name.clone(),
            );
        }
        crate::hir::WhenPat::Or { pats, .. } => {
            for pat in pats {
                collect_when_pat_binding_names_from_pat(source_path, pat, names);
            }
        }
        crate::hir::WhenPat::Tuple { elements, .. } => {
            for pat in elements {
                collect_when_pat_binding_names_from_pat(source_path, pat, names);
            }
        }
        crate::hir::WhenPat::Variant { args, .. } => {
            for pat in args {
                collect_when_pat_binding_names_from_pat(source_path, pat, names);
            }
        }
        crate::hir::WhenPat::Else { .. }
        | crate::hir::WhenPat::Wildcard { .. }
        | crate::hir::WhenPat::Rest { .. }
        | crate::hir::WhenPat::Is { .. }
        | crate::hir::WhenPat::IntLit { .. }
        | crate::hir::WhenPat::CharLit { .. }
        | crate::hir::WhenPat::StringLit { .. }
        | crate::hir::WhenPat::BoolLit { .. } => {}
    }
}

fn compare_call_sites(lhs: &CallSite, rhs: &CallSite) -> Ordering {
    lhs.source_path
        .cmp(&rhs.source_path)
        .then(lhs.span.start.cmp(&rhs.span.start))
        .then(lhs.span.end.cmp(&rhs.span.end))
}

fn compare_function_effect_contracts(
    lhs: &FunctionEffectContract,
    rhs: &FunctionEffectContract,
) -> Ordering {
    lhs.fqn()
        .cmp(rhs.fqn())
        .then(lhs.source_path().cmp(rhs.source_path()))
        .then(lhs.span().start.cmp(&rhs.span().start))
        .then(lhs.span().end.cmp(&rhs.span().end))
}

fn format_effect_row(types: &TypeStore, row: &EffectRow) -> String {
    if row.is_pure() {
        return "Pure".to_string();
    }

    row.terms
        .iter()
        .map(|ty| types.display(*ty).to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}

fn format_required_effects(
    types: &TypeStore,
    out_effects: &EffectRow,
    runtime_error_effect_ty: Option<TypeId>,
) -> String {
    let mut terms = out_effects.terms.clone();
    if let Some(runtime_error_effect_ty) = runtime_error_effect_ty
        && !terms.contains(&runtime_error_effect_ty)
    {
        terms.push(runtime_error_effect_ty);
    }
    format_effect_row(types, &EffectRow::new(terms))
}

fn format_assign_place_contract(
    out: &mut String,
    types: &TypeStore,
    call_site: &CallSite,
    contract: &AssignPlaceContract,
) {
    let _ = writeln!(out, "        AssignPlaceContract {{");
    let _ = writeln!(out, "            span: {:?},", call_site.span);
    match &contract.kind {
        AssignPlaceKind::Local {
            name, decl_span, ..
        } => {
            let _ = writeln!(out, "            kind: Local,");
            let label = crate::dump_support::LocalEntityKey::new(
                "typed_hir_assign_place",
                &call_site.source_path,
                *decl_span,
                "assign_place_local",
                name,
                0,
            )
            .label("sym");
            let _ = writeln!(out, "            label: {},", label);
            let _ = writeln!(out, "            name: {:?},", name);
            let _ = writeln!(out, "            decl_span: {:?},", decl_span);
        }
        AssignPlaceKind::TopLevel { fqn, .. } => {
            let _ = writeln!(out, "            kind: TopLevel,");
            let _ = writeln!(out, "            fqn: {:?},", fqn);
        }
        AssignPlaceKind::Member {
            receiver_ty,
            owner_fqn,
            member_fqn,
            member_name,
            member_span,
            resolved,
        } => {
            let _ = writeln!(out, "            kind: Member,");
            let _ = writeln!(
                out,
                "            receiver_ty: {},",
                types.display(*receiver_ty)
            );
            let _ = writeln!(out, "            owner_fqn: {:?},", owner_fqn);
            let _ = writeln!(out, "            member_fqn: {:?},", member_fqn);
            let _ = writeln!(out, "            member_name: {:?},", member_name);
            let _ = writeln!(out, "            member_span: {:?},", member_span);
            let _ = writeln!(out, "            resolved: {:?},", resolved);
        }
    }
    let _ = writeln!(
        out,
        "            place_ty: {},",
        format_type_id_lossy(types, contract.place_ty)
    );
    let _ = writeln!(
        out,
        "            value_ty: {},",
        format_type_id_lossy(types, contract.value_ty)
    );
    let _ = writeln!(out, "            mutable: {},", contract.mutable);
    let _ = writeln!(
        out,
        "            write_barrier: {},",
        format_assign_write_barrier(types, &contract.write_barrier)
    );
    let _ = writeln!(
        out,
        "            unsafe_required: {},",
        contract.unsafe_required
    );
    let _ = writeln!(out, "        }},");
}

fn format_assign_write_barrier(
    types: &TypeStore,
    requirement: &ast::AssignWriteBarrierRequirement,
) -> String {
    match requirement {
        ast::AssignWriteBarrierRequirement::NotRequired => "NotRequired".to_string(),
        ast::AssignWriteBarrierRequirement::StorageSlot { slot_ty } => {
            format!("StorageSlot({})", format_type_id_lossy(types, *slot_ty))
        }
    }
}

fn format_top_level_init_root(
    out: &mut String,
    types: &TypeStore,
    root: &TopLevelInitRootContract,
) {
    let _ = writeln!(out, "        TopLevelInitRootContract {{");
    let _ = writeln!(out, "            span: {:?},", root.span());
    let _ = writeln!(out, "            fqn: {:?},", root.fqn());
    let _ = writeln!(out, "            kind: {:?},", root.kind());
    match root.ty() {
        Some(ty) => {
            let _ = writeln!(out, "            ty: {},", format_type_id_lossy(types, ty));
        }
        None => {
            let _ = writeln!(out, "            ty: None,");
        }
    }
    match root.initializer_ty() {
        Some(ty) => {
            let _ = writeln!(
                out,
                "            initializer_ty: {},",
                format_type_id_lossy(types, ty)
            );
        }
        None => {
            let _ = writeln!(out, "            initializer_ty: None,");
        }
    }
    let _ = writeln!(
        out,
        "            has_initializer: {},",
        root.has_initializer()
    );
    let _ = writeln!(out, "            dependencies: [");
    for dependency in root.dependencies() {
        let _ = writeln!(out, "                TopLevelInitDependency {{");
        let _ = writeln!(out, "                    fqn: {:?},", dependency.fqn());
        let _ = writeln!(out, "                    kind: {:?},", dependency.kind());
        let _ = writeln!(out, "                }},");
    }
    let _ = writeln!(out, "            ],");
    let _ = writeln!(out, "        }},");
}

fn format_extern_global_contract(
    out: &mut String,
    types: &TypeStore,
    contract: &ExternGlobalContract,
) {
    let _ = writeln!(out, "        ExternGlobalContract {{");
    let _ = writeln!(out, "            span: {:?},", contract.span());
    let _ = writeln!(out, "            fqn: {:?},", contract.fqn());
    let _ = writeln!(out, "            symbol: {:?},", contract.symbol());
    let _ = writeln!(out, "            linkage: {:?},", contract.linkage());
    let _ = writeln!(out, "            storage: {:?},", contract.storage());
    let _ = writeln!(
        out,
        "            ty: {},",
        format_type_id_lossy(types, contract.ty())
    );
    let _ = writeln!(out, "            mutable: {},", contract.mutable());
    let _ = writeln!(
        out,
        "            initializer_absent: {},",
        contract.initializer_absent()
    );
    let _ = writeln!(
        out,
        "            unsafe_required: {},",
        contract.unsafe_required()
    );
    let _ = writeln!(out, "        }},");
}

fn format_with_update_contract(
    out: &mut String,
    types: &TypeStore,
    call_site: &CallSite,
    contract: &ast::WithUpdateContract,
) {
    let _ = writeln!(out, "        WithUpdateContract {{");
    let _ = writeln!(out, "            span: {:?},", call_site.span);
    let _ = writeln!(
        out,
        "            base_ty: {},",
        format_type_id_lossy(types, contract.base_ty)
    );
    let _ = writeln!(
        out,
        "            result_ty: {},",
        format_type_id_lossy(types, contract.result_ty)
    );
    let _ = writeln!(out, "            aggregates: [");
    for aggregate in &contract.aggregates {
        let _ = writeln!(out, "                WithUpdateAggregateContract {{");
        let _ = writeln!(out, "                    prefix: {:?},", aggregate.prefix);
        let _ = writeln!(
            out,
            "                    ty: {},",
            format_type_id_lossy(types, aggregate.ty)
        );
        match &aggregate.kind {
            ast::WithUpdateAggregateContractKind::Struct { fqn, fields } => {
                let _ = writeln!(out, "                    kind: Struct,");
                let _ = writeln!(out, "                    fqn: {:?},", fqn);
                let _ = writeln!(
                    out,
                    "                    fields: [{}],",
                    fields
                        .iter()
                        .map(|field| format!(
                            "{}: {}",
                            field.name,
                            format_type_id_lossy(types, field.ty)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            ast::WithUpdateAggregateContractKind::Tuple { elements } => {
                let _ = writeln!(out, "                    kind: Tuple,");
                let _ = writeln!(
                    out,
                    "                    elements: [{}],",
                    elements
                        .iter()
                        .map(|ty| format_type_id_lossy(types, *ty))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            ast::WithUpdateAggregateContractKind::Enum { info } => {
                let _ = writeln!(out, "                    kind: Enum,");
                let _ = writeln!(out, "                    enum_fqn: {:?},", info.enum_fqn);
                let _ = writeln!(
                    out,
                    "                    variants: [{}],",
                    format_with_update_variants(types, &info.variants)
                );
            }
        }
        let _ = writeln!(out, "                }},");
    }
    let _ = writeln!(out, "            ],");
    let _ = writeln!(out, "            updates: [");
    for update in &contract.updates {
        let _ = writeln!(out, "                WithUpdateUpdateContract {{");
        let _ = writeln!(out, "                    path: {:?},", update.path);
        let _ = writeln!(
            out,
            "                    target_ty: {},",
            format_type_id_lossy(types, update.target_ty)
        );
        let _ = writeln!(
            out,
            "                    value_ty: {},",
            format_type_id_lossy(types, update.value_ty)
        );
        let _ = writeln!(out, "                    segments: [");
        for segment in &update.segments {
            let _ = writeln!(
                out,
                "                        WithUpdatePathSegmentContract {{"
            );
            let _ = writeln!(
                out,
                "                            aggregate_prefix: {:?},",
                segment.aggregate_prefix
            );
            let _ = writeln!(
                out,
                "                            aggregate_ty: {},",
                format_type_id_lossy(types, segment.aggregate_ty)
            );
            let _ = writeln!(
                out,
                "                            field_ty: {},",
                format_type_id_lossy(types, segment.field_ty)
            );
            let _ = writeln!(out, "                            kind: {:?},", segment.kind);
            let _ = writeln!(out, "                        }},");
        }
        let _ = writeln!(out, "                    ],");
        let _ = writeln!(out, "                }},");
    }
    let _ = writeln!(out, "            ],");
    let _ = writeln!(out, "        }},");
}

fn format_with_update_variants(
    types: &TypeStore,
    variants: &[ast::WithUpdateResolvedEnumVariant],
) -> String {
    variants
        .iter()
        .map(|variant| {
            let fields = variant
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, format_type_id_lossy(types, field.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", variant.name, fields)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_call_site_contract(
    out: &mut String,
    types: &TypeStore,
    call_site: &CallSite,
    contract: &TypedCallSiteContract,
) {
    let _ = writeln!(out, "        CallSiteContract {{");
    let _ = writeln!(out, "            span: {:?},", call_site.span);
    let _ = writeln!(out, "            kind: {:?},", contract.kind());
    match contract {
        TypedCallSiteContract::DirectTopLevel(function) => {
            format_function_target(out, types, function, "target");
        }
        TypedCallSiteContract::MemberDirect(member) => {
            format_member_target(out, types, member, "member");
        }
        TypedCallSiteContract::Extension {
            receiver_ty,
            function,
        } => {
            let _ = writeln!(
                out,
                "            receiver_ty: {},",
                types.display(*receiver_ty)
            );
            format_function_target(out, types, function, "target");
        }
        TypedCallSiteContract::Constructor(ctor) => {
            let _ = writeln!(out, "            owner_fqn: {:?},", ctor.owner_fqn());
            let _ = writeln!(out, "            ctor_span: {:?},", ctor.ctor_span());
            let _ = writeln!(
                out,
                "            result_ty: {},",
                types.display(ctor.result_ty())
            );
            let _ = writeln!(out, "            arg_mapping: {:?},", ctor.arg_mapping());
        }
        TypedCallSiteContract::Closure {
            callee_ty,
            return_ty,
            abi_identity,
            arg_binding,
        }
        | TypedCallSiteContract::FunValue {
            callee_ty,
            return_ty,
            abi_identity,
            arg_binding,
        }
        | TypedCallSiteContract::FunPtr {
            callee_ty,
            return_ty,
            abi_identity,
            arg_binding,
        } => {
            let _ = writeln!(out, "            callee_ty: {},", types.display(*callee_ty));
            let _ = writeln!(out, "            return_ty: {},", types.display(*return_ty));
            let _ = writeln!(out, "            abi_identity: {:?},", abi_identity);
            if let Some(binding) = arg_binding {
                let _ = writeln!(out, "            arg_binding: {:?},", binding.params());
            }
        }
        TypedCallSiteContract::Virtual(member) | TypedCallSiteContract::Interface(member) => {
            format_member_target(out, types, member, "dispatch");
        }
        TypedCallSiteContract::Intrinsic { kind, function } => {
            let _ = writeln!(out, "            intrinsic_kind: {:?},", kind);
            let _ = writeln!(
                out,
                "            intrinsic_allowed_context: {:?},",
                kind.allowed_context()
            );
            let _ = writeln!(
                out,
                "            intrinsic_runtime_fallback: {:?},",
                kind.runtime_fallback()
            );
            format_function_target(out, types, function, "target");
        }
        TypedCallSiteContract::EffectOp(perform) => {
            let _ = writeln!(
                out,
                "            effect_ty: {},",
                types.display(perform.effect_ty())
            );
            let _ = writeln!(out, "            op_fqn: {:?},", perform.op_fqn());
            let _ = writeln!(
                out,
                "            result_ty: {},",
                types.display(perform.result_ty())
            );
            let _ = writeln!(
                out,
                "            payload_ty: {},",
                perform.payload().display(types)
            );
            let _ = writeln!(out, "            arg_mapping: {:?},", perform.arg_mapping());
        }
        TypedCallSiteContract::ContinuationResume(resume) => {
            let _ = writeln!(
                out,
                "            receiver_route: {:?},",
                resume.receiver_route()
            );
            let _ = writeln!(
                out,
                "            payload_arg_indices: {:?},",
                resume.payload_arg_indices()
            );
            let _ = writeln!(
                out,
                "            receiver_ty: {},",
                types.display(resume.receiver_ty())
            );
            let _ = writeln!(
                out,
                "            resume_ty: {},",
                types.display(resume.resume_ty())
            );
            let _ = writeln!(
                out,
                "            answer_ty: {},",
                types.display(resume.answer_ty())
            );
            let _ = writeln!(
                out,
                "            out_effects: {},",
                format_effect_row(types, resume.out_effects())
            );
        }
    }
    let _ = writeln!(out, "        }},");
}

fn format_member_target(
    out: &mut String,
    types: &TypeStore,
    member: &MemberCallTargetContract,
    label: &str,
) {
    let _ = writeln!(
        out,
        "            {label}_owner_fqn: {:?},",
        member.owner_fqn()
    );
    let _ = writeln!(
        out,
        "            {label}_member_name: {:?},",
        member.member_name()
    );
    let _ = writeln!(
        out,
        "            {label}_member_fqn: {:?},",
        member.member_fqn()
    );
    let _ = writeln!(
        out,
        "            receiver_ty: {},",
        types.display(member.receiver_ty())
    );
    format_function_target(out, types, member.function(), "target");
}

fn format_function_target(
    out: &mut String,
    types: &TypeStore,
    function: &FunctionTargetContract,
    label: &str,
) {
    let _ = writeln!(out, "            {label}_fqn: {:?},", function.fqn());
    let _ = writeln!(
        out,
        "            {label}_decl_span: {:?},",
        function.decl_span()
    );
    let _ = writeln!(
        out,
        "            {label}_type_args: [{}],",
        format_type_args(types, function.type_args())
    );
    let _ = writeln!(
        out,
        "            {label}_eff_args: [{}],",
        format_eff_args(types, function.eff_args())
    );
    if let Some(binding) = function.arg_binding() {
        let _ = writeln!(
            out,
            "            {label}_arg_binding: {:?},",
            binding.params()
        );
    }
}

fn format_type_args(types: &TypeStore, args: &[TypeId]) -> String {
    args.iter()
        .map(|ty| format_type_id_lossy(types, *ty))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_eff_args(types: &TypeStore, args: &[EffectRow]) -> String {
    args.iter()
        .map(|row| {
            if row.is_pure() {
                "Pure".to_string()
            } else {
                row.terms
                    .iter()
                    .map(|ty| format_type_id_lossy(types, *ty))
                    .collect::<Vec<_>>()
                    .join(" + ")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_type_id_lossy(types: &TypeStore, ty: TypeId) -> String {
    if type_id_in_store(types, ty) {
        types.display(ty).to_string()
    } else {
        format!("TypeId({})", ty.as_u32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_stage(source_text: &str) -> HirStageOutput {
        let session = Session::new().expect("create test session");
        let source = SourceFile::new_virtual("<t2_01r_source_effect_facts>", source_text);
        let lowered = crate::hir::lower_typed_for_dump(&session, &source)
            .expect("test source should lower to typed HIR");
        HirStageOutput::new(lowered, source.path()).expect("HIR facts should verify")
    }

    #[test]
    fn source_effect_facts_include_operator_semantic_bindings() {
        let output = lower_stage(
            r#"
package fixtures.t2_01r

effect Log {
    fun write(message: String): Unit
}

struct Box {
    val value: Int

    operator fun plus(other: Box): Box / Log {
        return other
    }

    operator fun inv(): Box / Log {
        return this
    }
}

private fun viaOperators(a: Box, b: Box): Box {
    val c: Box = a + b
    val d: Box = ~a
    return c
}
"#,
        );
        let facts = &output.hir_facts().source_sites;
        let callable = facts
            .callable_source_effects
            .iter()
            .find(|fact| fact.fqn == "fixtures.t2_01r.viaOperators")
            .expect("viaOperators callable source effect fact should be published");

        let inferred = callable.inferred_surface_row_template.canonical_text();
        assert!(
            inferred.contains("fixtures.t2_01r.Log"),
            "operator call effects should flow into inferred surface row, got {inferred}"
        );
        let published = callable.published_surface_row_template.canonical_text();
        assert!(
            published.contains("fixtures.t2_01r.Log"),
            "private omitted-row function should publish inferred operator effects, got {published}"
        );

        let semantic_targets = facts
            .semantic_operations
            .iter()
            .filter_map(|fact| match &fact.kind {
                hir_site_facts::CanonicalSemanticOperationKind::CoreCall { target, .. } => {
                    target.as_ref().map(|target| target.as_str().to_string())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            semantic_targets
                .iter()
                .any(|target| target.contains("fixtures.t2_01r.Box.plus")),
            "binary operator should publish a canonical semantic core call, got {semantic_targets:?}"
        );
        assert!(
            semantic_targets
                .iter()
                .any(|target| target.contains("fixtures.t2_01r.Box.inv")),
            "unary operator should publish a canonical semantic core call, got {semantic_targets:?}"
        );
    }
}
