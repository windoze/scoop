use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::ast;
use crate::hir::{
    AssignPlaceContract, AssignPlaceKind, Block, CallArg, CallSite, Decl, DispatchCallKind, Expr,
    ExprKind, FunDecl, HandleArmKind, HirLowerError, HirStageError, Item, LoweredHir, Stmt,
    StmtKind, ValDecl, ValueRef,
};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

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
    span: Span,
    fqn: String,
    return_ty: TypeId,
    allowed_effects: EffectRow,
    effects_closed: bool,
}

impl FunctionEffectContract {
    fn new(
        span: Span,
        fqn: String,
        return_ty: TypeId,
        allowed_effects: EffectRow,
        effects_closed: bool,
    ) -> Self {
        Self {
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
    fqn: String,
    source_path: PathBuf,
    span: Span,
    kind: TopLevelInitRootKind,
    ty: Option<TypeId>,
    initializer_ty: Option<TypeId>,
    has_initializer: bool,
    dependencies: Vec<TopLevelInitDependency>,
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
    ConstVal,
    RuntimeImmutableVal,
    RuntimeMutableVar {
        storage: crate::hir::TopLevelVarStorage,
    },
    ObjectSingleton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelInitDependency {
    fqn: String,
    kind: TopLevelInitDependencyKind,
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
    fqn: String,
    source_path: PathBuf,
    span: Span,
    ty: TypeId,
    mutable: bool,
    symbol: String,
    linkage: crate::hir::ExternGlobalLinkage,
    storage: crate::hir::TopLevelVarStorage,
    initializer_absent: bool,
    unsafe_required: bool,
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

/// 单个 `handle { ... } with { ... }` 站点的 typed contract。
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
    DirectCall,
    ContinuationResume,
    Perform,
}

/// 编译器/运行时 intrinsic 在 typed HIR call contract 中的稳定分类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedIntrinsicKind {
    Reflection { name: String },
    Platform { name: String },
    Gc { name: String },
    Runtime { name: String },
    Compiler { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicAllowedContext {
    ComptimeAndRuntime,
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
    fn from_fqn(fqn: &str) -> Self {
        let name = fqn.rsplit('.').next().unwrap_or(fqn).to_string();
        match fqn {
            "scoop.core.nameOf"
            | "scoop.core.sizeOf"
            | "scoop.core.alignOf"
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
        match self {
            Self::Reflection { .. } | Self::Platform { .. } => {
                IntrinsicAllowedContext::ComptimeAndRuntime
            }
            Self::Gc { .. } | Self::Runtime { .. } | Self::Compiler { .. } => {
                IntrinsicAllowedContext::RuntimeOnly
            }
        }
    }

    pub fn runtime_fallback(&self) -> IntrinsicRuntimeFallback {
        match self {
            Self::Reflection { .. } => IntrinsicRuntimeFallback::NormalRuntimeCall,
            Self::Platform { .. } => IntrinsicRuntimeFallback::PlatformQuery,
            Self::Gc { .. } | Self::Runtime { .. } => IntrinsicRuntimeFallback::RuntimeIntrinsic,
            Self::Compiler { .. } => IntrinsicRuntimeFallback::CompilerLowered,
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
    fqn: String,
    decl_file: Option<PathBuf>,
    decl_span: Option<Span>,
    type_args: Vec<TypeId>,
    eff_args: Vec<EffectRow>,
    arg_binding: Option<CallArgBindingContract>,
}

impl FunctionTargetContract {
    fn from_binding(
        types: &TypeStore,
        binding: &ast::TopLevelFunCallBinding,
        arg_binding: Option<CallArgBindingContract>,
    ) -> Self {
        Self {
            fqn: binding.fqn.clone(),
            decl_file: Some(binding.decl_file.clone()),
            decl_span: Some(binding.decl_span),
            type_args: binding
                .type_args
                .iter()
                .copied()
                .filter(|ty| type_id_in_store(types, *ty))
                .collect(),
            eff_args: binding
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
                .collect(),
            arg_binding,
        }
    }

    fn synthetic_with_arg_binding(
        fqn: String,
        arg_binding: Option<CallArgBindingContract>,
    ) -> Self {
        Self {
            fqn,
            decl_file: None,
            decl_span: None,
            type_args: Vec::new(),
            eff_args: Vec::new(),
            arg_binding,
        }
    }

    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn decl_file(&self) -> Option<&Path> {
        self.decl_file.as_deref()
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

    pub fn arg_binding(&self) -> Option<&CallArgBindingContract> {
        self.arg_binding.as_ref()
    }
}

/// 成员调用的结构化 owner/member 绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCallTargetContract {
    owner_fqn: String,
    member_name: String,
    member_fqn: String,
    receiver_ty: TypeId,
    function: FunctionTargetContract,
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
    owner_fqn: String,
    ctor_span: Option<Span>,
    result_ty: TypeId,
    arg_mapping: Vec<Option<usize>>,
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
        arg_binding: Option<CallArgBindingContract>,
    },
    FunValue {
        callee_ty: TypeId,
        return_ty: TypeId,
        arg_binding: Option<CallArgBindingContract>,
    },
    FunPtr {
        callee_ty: TypeId,
        return_ty: TypeId,
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

/// refactor typed HIR stage 显式输出的 effect / continuation contract side tables。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypedHirEffectContracts {
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

impl TypedHirEffectContracts {
    pub(crate) fn from_lowered_hir(
        lowered_hir: &LoweredHir,
        source_path: &Path,
    ) -> Result<Self, HirStageError> {
        ContractCollector::new(lowered_hir).collect(source_path)
    }

    pub const fn is_placeholder(&self) -> bool {
        false
    }

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

    pub fn function_effects(&self) -> &[FunctionEffectContract] {
        &self.function_effects
    }

    pub fn continuation_resume_sites(&self) -> &HashMap<CallSite, ContinuationResumeSiteContract> {
        &self.continuation_resume_sites
    }

    pub fn continuation_resume_site(
        &self,
        call_site: &CallSite,
    ) -> Option<&ContinuationResumeSiteContract> {
        self.continuation_resume_sites.get(call_site)
    }

    pub fn perform_sites(&self) -> &HashMap<CallSite, PerformSiteContract> {
        &self.perform_sites
    }

    pub fn perform_site(&self, call_site: &CallSite) -> Option<&PerformSiteContract> {
        self.perform_sites.get(call_site)
    }

    pub fn handle_sites(&self) -> &HashMap<CallSite, HandleSiteContract> {
        &self.handle_sites
    }

    pub fn handle_site(&self, call_site: &CallSite) -> Option<&HandleSiteContract> {
        self.handle_sites.get(call_site)
    }

    pub fn call_site_kinds(&self) -> &HashMap<CallSite, TypedCallSiteKind> {
        &self.call_site_kinds
    }

    pub fn call_site_kind(&self, call_site: &CallSite) -> Option<TypedCallSiteKind> {
        self.call_site_kinds.get(call_site).copied()
    }

    pub fn call_site_contracts(&self) -> &HashMap<CallSite, TypedCallSiteContract> {
        &self.call_site_contracts
    }

    pub fn call_site_contract(&self, call_site: &CallSite) -> Option<&TypedCallSiteContract> {
        self.call_site_contracts.get(call_site)
    }

    pub fn with_update_contracts(&self) -> &HashMap<CallSite, ast::WithUpdateContract> {
        &self.with_update_contracts
    }

    pub fn assign_place_contracts(&self) -> &HashMap<CallSite, AssignPlaceContract> {
        &self.assign_place_contracts
    }

    pub fn top_level_init_roots(&self) -> &[TopLevelInitRootContract] {
        &self.top_level_init_roots
    }

    pub fn extern_global_contracts(&self) -> &[ExternGlobalContract] {
        &self.extern_global_contracts
    }

    /// 以稳定顺序渲染 typed HIR side tables，供 `dump-hir` 与 snapshot tests 使用。
    pub fn stable_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "TypedHirEffectContracts {{");

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

/// refactor typed HIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P2/P3 及后续阶段直接消费：
/// - 输出已经过 resolver + typecheck，可直接视为 typed HIR handoff；
/// - `Continuation` / `resume` / `perform` / `handle` 的 typed contract 应在此阶段显式化，
///   下游不应再回 AST 猜测 surface 语义；
/// - `dump-hir` 的 refactor 路径必须优先消费这一 stage 输出，而不是 legacy
///   `hir::lower_for_dump(...)`；
/// - `effect_contracts` 现在显式输出函数级 allowed-row contract，以及 `Continuation.resume(...)` /
///   `perform` / `handle` 的结构化 typed contract，固定 `ResumeTuple` / `Answer` / `Out`、
///   runtime error ordinary effect 贡献、performed effect/payload、以及 handler arm typed 关系，
///   供后续阶段直接消费。
#[derive(Debug)]
pub struct TypedHirStageOutput {
    lowered_hir: LoweredHir,
    effect_contracts: TypedHirEffectContracts,
    source_path: PathBuf,
}

impl TypedHirStageOutput {
    pub(crate) fn new(lowered_hir: LoweredHir, source_path: &Path) -> Result<Self, HirStageError> {
        HirCompletenessVerifier::new(&lowered_hir, source_path).verify()?;
        Self::new_checked(lowered_hir, source_path)
    }

    fn new_checked(mut lowered_hir: LoweredHir, source_path: &Path) -> Result<Self, HirStageError> {
        ensure_raise_runtime_error_effect(&mut lowered_hir.types);
        let effect_contracts =
            TypedHirEffectContracts::from_lowered_hir(&lowered_hir, source_path)?;
        Ok(Self {
            lowered_hir,
            effect_contracts,
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

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// 以稳定文本渲染 refactor typed HIR dump：先打印 HIR `File`，再追加 typed side tables。
    pub fn stable_dump(&self) -> String {
        let mut out =
            crate::hir::stable_dump_file(self.hir_file(), self.types(), self.source_path());
        out.push('\n');
        out.push('\n');
        out.push_str(&self.effect_contracts.stable_dump(self.types()));
        out.push('\n');
        out
    }

    pub fn into_lowered_hir(self) -> LoweredHir {
        self.lowered_hir
    }
}

pub(crate) fn run(
    session: &Session,
    source: &SourceFile,
) -> Result<TypedHirStageOutput, HirLowerError> {
    let lowered_hir = crate::hir::lower_typed_for_dump(session, source)?;
    TypedHirStageOutput::new(lowered_hir, source.path()).map_err(HirLowerError::from)
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

    fn collect(mut self, source_path: &Path) -> Result<TypedHirEffectContracts, HirStageError> {
        for item in &self.lowered_hir.file.items {
            self.collect_item(source_path, item)?;
        }

        for member_fun in &self.lowered_hir.member_funs {
            self.record_function_effect_contract(member_fun);
            self.collect_fun(member_fun)?;
        }

        self.function_effects
            .sort_by(compare_function_effect_contracts);
        Ok(TypedHirEffectContracts {
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

    fn record_function_effect_contract(&mut self, fun: &FunDecl) {
        let Some((allowed_effects, effects_closed)) =
            function_effect_contract(&self.lowered_hir.types, fun.ty)
        else {
            return;
        };

        self.function_effects.push(FunctionEffectContract::new(
            fun.span,
            fun.fqn.clone(),
            fun.return_ty,
            allowed_effects,
            effects_closed,
        ));
    }

    fn top_level_val_source_path(&self, val: &ValDecl) -> Option<PathBuf> {
        self.lowered_hir
            .top_level_vars
            .values()
            .find(|global| global.span == val.span)
            .map(|global| global.source_path.clone())
            .or_else(|| {
                self.lowered_hir
                    .top_level_consts
                    .values()
                    .find(|konst| konst.span == val.span)
                    .map(|konst| konst.source_path.clone())
            })
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
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. }
            | ExprKind::MemberAccess { receiver: expr, .. } => {
                self.collect_expr(source_path, expr)?;
            }
            ExprKind::Binary { lhs, rhs, .. } => {
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
            let function = FunctionTargetContract::from_binding(
                &self.lowered_hir.types,
                binding,
                arg_binding.clone(),
            );

            let contract = if binding.is_intrinsic {
                TypedCallSiteContract::Intrinsic {
                    kind: TypedIntrinsicKind::from_fqn(&binding.fqn),
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
                receiver_ty_from_arg_binding(arg_binding.as_ref(), args)
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
            let function = FunctionTargetContract::synthetic_with_arg_binding(
                fqn.clone(),
                arg_binding.clone(),
            );
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
        } else if let ExprKind::MemberAccess { member, .. } = &callee.kind {
            if let Some(crate::hir::MemberRef::Fun { fqn, .. }) = member.resolved.as_ref()
                && fqn.starts_with("scoop.core.GC.")
            {
                Some(TypedCallSiteContract::Intrinsic {
                    kind: TypedIntrinsicKind::from_fqn(fqn),
                    function: FunctionTargetContract::synthetic_with_arg_binding(
                        fqn.clone(),
                        arg_binding.clone(),
                    ),
                })
            } else {
                None
            }
        } else if matches!(callee.kind, ExprKind::Closure(_)) {
            Some(TypedCallSiteContract::Closure {
                callee_ty: callee.ty,
                return_ty: expr.ty,
                arg_binding: arg_binding.clone(),
            })
        } else if is_funptr_ty(&self.lowered_hir.types, callee.ty) {
            Some(TypedCallSiteContract::FunPtr {
                callee_ty: callee.ty,
                return_ty: expr.ty,
                arg_binding: arg_binding.clone(),
            })
        } else if is_function_ty(&self.lowered_hir.types, callee.ty)
            || matches!(callee.kind, ExprKind::VarRef(ValueRef::Local { .. }))
        {
            Some(TypedCallSiteContract::FunValue {
                callee_ty: callee.ty,
                return_ty: expr.ty,
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
        if !matches!(callee.kind, ExprKind::UnresolvedIdent { .. }) {
            return false;
        }

        match self.lowered_hir.types.kind(expr.ty) {
            TypeKind::Value(ValueTypeKind::Option(_)) => true,
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => self
                .lowered_hir
                .nominal_kinds
                .get(&nominal.fqn)
                .is_some_and(|kind| *kind == ast::TypeKind::Enum),
            _ => false,
        }
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

fn receiver_ty_from_arg_binding(
    binding: Option<&CallArgBindingContract>,
    args: &[CallArg],
) -> Option<TypeId> {
    let binding = binding?;
    for param in binding.params() {
        if matches!(param, CallArgParamContract::Receiver) {
            return args.first().map(call_arg_value_ty);
        }
    }
    None
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

fn function_effect_contract(types: &TypeStore, fun_ty: TypeId) -> Option<(EffectRow, bool)> {
    let TypeKind::Ref(RefTypeKind::Function(function)) = types.kind(fun_ty) else {
        return None;
    };

    Some((function.effects.clone(), function.effects_closed))
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
    let lowered_object_fqns = lowered_object_decl_fqns(lowered_hir);
    let mut roots = Vec::new();

    for konst in lowered_hir.top_level_consts.values() {
        roots.push(TopLevelInitRootContract {
            fqn: konst.fqn.clone(),
            source_path: konst.source_path.clone(),
            span: konst.span,
            kind: TopLevelInitRootKind::ConstVal,
            ty: Some(konst.ty),
            initializer_ty: konst.init.as_ref().map(|init| init.ty),
            has_initializer: konst.init.is_some(),
            dependencies: dependencies_for_expr(
                konst.fqn.as_str(),
                konst.init.as_ref(),
                &dependency_kinds,
            ),
        });
    }

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
                value.init.as_ref(),
                &dependency_kinds,
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
                var.init.as_ref(),
                &dependency_kinds,
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
            dependencies: dependencies_for_object(object.fqn.as_str(), object, &dependency_kinds),
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
    for fqn in lowered_hir.top_level_consts.keys() {
        out.insert(fqn.clone(), TopLevelInitDependencyKind::TopLevelValue);
    }
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

fn dependencies_for_expr(
    owner_fqn: &str,
    expr: Option<&Expr>,
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
) -> Vec<TopLevelInitDependency> {
    let mut out = Vec::new();
    if let Some(expr) = expr {
        collect_expr_dependencies(expr, owner_fqn, dependency_kinds, &mut out);
    }
    stable_dependencies(out)
}

fn dependencies_for_object(
    owner_fqn: &str,
    object: &crate::hir::ObjectInit,
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
) -> Vec<TopLevelInitDependency> {
    let mut out = Vec::new();
    for step in &object.steps {
        match step {
            crate::hir::ObjectInitStep::PropertyInit { init, .. } => {
                collect_expr_dependencies(init, owner_fqn, dependency_kinds, &mut out);
            }
            crate::hir::ObjectInitStep::InitBlock { block } => {
                collect_block_dependencies(block, owner_fqn, dependency_kinds, &mut out);
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
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    for stmt in &block.stmts {
        collect_stmt_dependencies(stmt, owner_fqn, dependency_kinds, out);
    }
}

fn collect_stmt_dependencies(
    stmt: &Stmt,
    owner_fqn: &str,
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => collect_expr_dependencies(expr, owner_fqn, dependency_kinds, out),
        StmtKind::Val(val) => {
            if let Some(init) = &val.init {
                collect_expr_dependencies(init, owner_fqn, dependency_kinds, out);
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_expr_dependencies(lhs, owner_fqn, dependency_kinds, out);
            collect_expr_dependencies(rhs, owner_fqn, dependency_kinds, out);
        }
        StmtKind::While { cond, body } => {
            collect_expr_dependencies(cond, owner_fqn, dependency_kinds, out);
            collect_block_dependencies(body, owner_fqn, dependency_kinds, out);
        }
        StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_expr_dependencies(value, owner_fqn, dependency_kinds, out);
            }
        }
    }
}

fn collect_expr_dependencies(
    expr: &Expr,
    owner_fqn: &str,
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
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
            push_top_level_dependency(fqn, owner_fqn, dependency_kinds, out);
        }
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_expr_dependencies(&field.value, owner_fqn, dependency_kinds, out);
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_expr_dependencies(element, owner_fqn, dependency_kinds, out);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_expr_dependencies(expr, owner_fqn, dependency_kinds, out);
                }
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => {
            collect_expr_dependencies(expr, owner_fqn, dependency_kinds, out);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_dependencies(lhs, owner_fqn, dependency_kinds, out);
            collect_expr_dependencies(rhs, owner_fqn, dependency_kinds, out);
        }
        ExprKind::Block(block) => {
            collect_block_dependencies(block, owner_fqn, dependency_kinds, out)
        }
        ExprKind::Closure(_) => {}
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_dependencies(cond, owner_fqn, dependency_kinds, out);
            collect_expr_dependencies(then_branch, owner_fqn, dependency_kinds, out);
            if let Some(else_branch) = else_branch {
                collect_expr_dependencies(else_branch, owner_fqn, dependency_kinds, out);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_expr_dependencies(subject, owner_fqn, dependency_kinds, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_dependencies(guard, owner_fqn, dependency_kinds, out);
                }
                collect_expr_dependencies(&arm.body, owner_fqn, dependency_kinds, out);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            collect_expr_dependencies(receiver, owner_fqn, dependency_kinds, out);
        }
        ExprKind::Call { callee, args } => {
            collect_expr_dependencies(callee, owner_fqn, dependency_kinds, out);
            collect_call_arg_dependencies(args, owner_fqn, dependency_kinds, out);
        }
        ExprKind::Perform { args, .. } => {
            collect_call_arg_dependencies(args, owner_fqn, dependency_kinds, out);
        }
        ExprKind::Handle(handle) => {
            collect_block_dependencies(&handle.body, owner_fqn, dependency_kinds, out);
            for arm in &handle.arms {
                collect_expr_dependencies(&arm.body, owner_fqn, dependency_kinds, out);
            }
            if let Some(finally) = &handle.finally {
                collect_block_dependencies(finally, owner_fqn, dependency_kinds, out);
            }
        }
    }
}

fn collect_call_arg_dependencies(
    args: &[CallArg],
    owner_fqn: &str,
    dependency_kinds: &HashMap<String, TopLevelInitDependencyKind>,
    out: &mut Vec<TopLevelInitDependency>,
) {
    for arg in args {
        match arg {
            CallArg::Positional(expr) => {
                collect_expr_dependencies(expr, owner_fqn, dependency_kinds, out);
            }
            CallArg::Named { value, .. } => {
                collect_expr_dependencies(value, owner_fqn, dependency_kinds, out);
            }
        }
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
            arg_binding,
        }
        | TypedCallSiteContract::FunValue {
            callee_ty,
            return_ty,
            arg_binding,
        }
        | TypedCallSiteContract::FunPtr {
            callee_ty,
            return_ty,
            arg_binding,
        } => {
            let _ = writeln!(out, "            callee_ty: {},", types.display(*callee_ty));
            let _ = writeln!(out, "            return_ty: {},", types.display(*return_ty));
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
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::session::SessionOptions;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn load_hir_fixture(name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir")
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn clean_lowered_hir() -> (LoweredHir, PathBuf) {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_no_todo_clean.scoop",
            "package sample\nfun main() {}\n",
        );
        let source_path = source.path().to_path_buf();
        let lowered = crate::hir::lower_typed_for_dump(&session, &source).unwrap();
        (lowered, source_path)
    }

    fn stage_error_for(lowered: LoweredHir, source_path: &std::path::Path) -> HirStageError {
        TypedHirStageOutput::new(lowered, source_path)
            .expect_err("refactor HIR completeness verifier 应拒绝 placeholder")
    }

    fn test_span() -> Span {
        Span::new(21, 22)
    }

    fn expr_with_kind(lowered: &LoweredHir, kind: ExprKind) -> Expr {
        Expr {
            span: test_span(),
            ty: lowered.builtins.unit,
            kind,
        }
    }

    fn stmt_with_kind(lowered: &LoweredHir, kind: StmtKind) -> Stmt {
        Stmt {
            span: test_span(),
            ty: lowered.builtins.unit,
            kind,
        }
    }

    fn replace_main_body_with_stmt(lowered: &mut LoweredHir, stmt: Stmt) {
        let fun = lowered
            .file
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "sample.main" => Some(fun),
                _ => None,
            })
            .expect("clean fixture 应包含 sample.main");
        fun.body = Some(crate::hir::Block {
            span: test_span(),
            ty: lowered.builtins.unit,
            stmts: vec![stmt],
        });
    }

    fn main_fun_clone(lowered: &LoweredHir) -> FunDecl {
        lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "sample.main" => Some(fun.clone()),
                _ => None,
            })
            .expect("clean fixture 应包含 sample.main")
    }

    #[test]
    fn refactor_hir_no_todo_rejects_current_placeholder_reasons() {
        const EXPR_REASONS: &[&str] = &[
            "array_lit",
            "spread_arg",
            "named_arg",
            "structured_concurrency_spawn_deferred",
            "structured_concurrency_join_deferred",
            "splice_field",
            "assign",
            "with_update",
        ];
        for reason in EXPR_REASONS {
            let (mut lowered, source_path) = clean_lowered_hir();
            let stmt = stmt_with_kind(
                &lowered,
                StmtKind::Expr(expr_with_kind(&lowered, ExprKind::Todo(reason))),
            );
            replace_main_body_with_stmt(&mut lowered, stmt);

            let err = stage_error_for(lowered, &source_path);
            let expected = format!("ExprKind::Todo({reason})");
            assert_eq!(err.reason(), expected);
            assert_eq!(err.owner(), "fun sample.main");
            assert_eq!(err.span(), test_span());
            assert_eq!(err.source_path(), source_path.as_path());
        }

        const STMT_REASONS: &[&str] = &[
            "missing_stmt",
            "comptime_block",
            "comptime_if",
            "comptime_for",
            "for_custom_iterator",
        ];
        for reason in STMT_REASONS {
            let (mut lowered, source_path) = clean_lowered_hir();
            let stmt = stmt_with_kind(&lowered, StmtKind::Todo(reason));
            replace_main_body_with_stmt(&mut lowered, stmt);

            let err = stage_error_for(lowered, &source_path);
            let expected = format!("StmtKind::Todo({reason})");
            assert_eq!(err.reason(), expected);
            assert_eq!(err.owner(), "fun sample.main");
            assert_eq!(err.span(), test_span());
            assert_eq!(err.source_path(), source_path.as_path());
        }

        const ITEM_REASONS: &[&str] = &["comptime_if_item"];
        for reason in ITEM_REASONS {
            let (mut lowered, source_path) = clean_lowered_hir();
            lowered.file.items = vec![Item::Todo {
                span: test_span(),
                kind: reason,
            }];

            let err = stage_error_for(lowered, &source_path);
            let expected = format!("Item::Todo({reason})");
            assert_eq!(err.reason(), expected);
            assert_eq!(err.owner(), "top-level item");
            assert_eq!(err.span(), test_span());
            assert_eq!(err.source_path(), source_path.as_path());
        }

        let (mut lowered, source_path) = clean_lowered_hir();
        let stmt = stmt_with_kind(
            &lowered,
            StmtKind::Expr(expr_with_kind(&lowered, ExprKind::Missing)),
        );
        replace_main_body_with_stmt(&mut lowered, stmt);

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Missing");
        assert_eq!(err.owner(), "fun sample.main");
        assert_eq!(err.span(), test_span());
        assert_eq!(err.source_path(), source_path.as_path());
    }

    #[test]
    fn refactor_hir_no_todo_scans_member_fun_and_init_roots() {
        let (mut lowered, source_path) = clean_lowered_hir();
        let mut member_fun = main_fun_clone(&lowered);
        member_fun.fqn = "sample.Box.member".to_string();
        member_fun.name = "member".to_string();
        let stmt = stmt_with_kind(
            &lowered,
            StmtKind::Expr(expr_with_kind(&lowered, ExprKind::Todo("array_lit"))),
        );
        member_fun.body = Some(crate::hir::Block {
            span: test_span(),
            ty: lowered.builtins.unit,
            stmts: vec![stmt],
        });
        lowered.member_funs.push(member_fun);

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "member fun sample.Box.member");

        let (mut lowered, source_path) = clean_lowered_hir();
        lowered.top_level_vars.insert(
            "sample.global".to_string(),
            crate::hir::TopLevelVar {
                fqn: "sample.global".to_string(),
                source_path: source_path.clone(),
                span: test_span(),
                storage: crate::hir::TopLevelVarStorage::Global,
                ty: lowered.builtins.unit,
                init: Some(expr_with_kind(&lowered, ExprKind::Todo("array_lit"))),
            },
        );

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "top-level var sample.global");

        let (mut lowered, source_path) = clean_lowered_hir();
        lowered.object_inits.insert(
            "sample.Singleton".to_string(),
            crate::hir::ObjectInit {
                fqn: "sample.Singleton".to_string(),
                span: test_span(),
                source_path: source_path.clone(),
                properties: HashMap::new(),
                steps: vec![crate::hir::ObjectInitStep::PropertyInit {
                    name: "x".to_string(),
                    init: expr_with_kind(&lowered, ExprKind::Todo("array_lit")),
                }],
            },
        );

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "object sample.Singleton");

        let (mut lowered, source_path) = clean_lowered_hir();
        lowered.class_inits.insert(
            "sample.Box".to_string(),
            crate::hir::ClassInit {
                fqn: "sample.Box".to_string(),
                source_path: source_path.clone(),
                super_class_fqn: None,
                super_ctor_args_span: None,
                super_ctor_call: None,
                super_ctor_args: Vec::new(),
                this_id: crate::hir::SymbolId::from_raw(1),
                fields: Vec::new(),
                field_indices: HashMap::new(),
                steps: vec![crate::hir::ClassInitStep::PropertyInit {
                    field_fqn: "sample.Box.x".to_string(),
                    init: expr_with_kind(&lowered, ExprKind::Todo("array_lit")),
                }],
                ctors: Vec::new(),
            },
        );

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "class sample.Box");
    }

    #[test]
    fn refactor_hir_decls_lowers_typealias_nominal_object_and_extension_property_graph() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_decls.scoop",
            r#"package sample
typealias Alias = Int
interface Named {
    fun name(): String
}
class Person(val id: Int) : Named {
    val title: String = "Dr"
    init {}
    constructor(id: Int, name: String) : this(id) {}
    fun name(): String { return "person" }
}
struct Point(val x: Int, val y: Int)
enum Choice {
    Some(val value: Int)
    None
}
object Registry {
    val count: Int = 0
    fun name(): String { return "registry" }
}
val String.last: Int
    get() = 0
fun main() {}
"#,
        );

        let output = run(&session, &source).expect("declaration graph 不应生成 Item::Todo");
        let decls = &output.hir_file().decls;
        assert!(
            decls
                .iter()
                .any(|decl| matches!(decl, crate::hir::Decl::TypeAlias(alias) if alias.fqn == "sample.Alias")),
            "typealias declaration should be present: {decls:#?}"
        );
        assert!(
            decls.iter().any(|decl| {
                matches!(decl, crate::hir::Decl::Nominal(nominal)
                if nominal.fqn == "sample.Person"
                    && nominal.kind == crate::ast::TypeKind::Class
                    && nominal.interfaces.iter().any(|iface| iface == "sample.Named")
                    && nominal.constructors.len() == 2
                    && nominal.members.iter().any(|member| matches!(
                        member,
                        crate::hir::DeclMember::Field(field) if field.fqn == "sample.Person.id"
                    ))
                    && nominal.members.iter().any(|member| matches!(
                        member,
                        crate::hir::DeclMember::Fun(fun) if fun.fqn == "sample.Person.name"
                    )))
            }),
            "class declaration should expose identity, constructors, fields, funcs, and interfaces: {decls:#?}"
        );
        assert!(
            decls.iter().any(|decl| {
                matches!(decl, crate::hir::Decl::Nominal(nominal)
                if nominal.fqn == "sample.Point"
                    && nominal.kind == crate::ast::TypeKind::Struct
                    && nominal.members.iter().any(|member| matches!(
                        member,
                        crate::hir::DeclMember::Field(field) if field.fqn == "sample.Point.x"
                    )))
            }),
            "struct declaration should expose primary-constructor fields: {decls:#?}"
        );
        assert!(
            decls.iter().any(|decl| {
                matches!(decl, crate::hir::Decl::Nominal(nominal)
                    if nominal.fqn == "sample.Choice"
                        && nominal.kind == crate::ast::TypeKind::Enum
                        && nominal.members.iter().any(|member| matches!(
                            member,
                            crate::hir::DeclMember::EnumVariant(variant) if variant.fqn == "sample.Choice.Some"
                        )))
            }),
            "enum declaration should expose variants: {decls:#?}"
        );
        assert!(
            decls.iter().any(|decl| {
                matches!(decl, crate::hir::Decl::Object(object)
                if object.fqn == "sample.Registry"
                    && object.initializer_root == "sample.Registry"
                    && object.members.iter().any(|member| matches!(
                        member,
                        crate::hir::DeclMember::Field(field) if field.fqn == "sample.Registry.count"
                    )))
            }),
            "object declaration should expose singleton identity, members, and initializer root: {decls:#?}"
        );
        assert!(
            decls.iter().any(|decl| {
                matches!(decl, crate::hir::Decl::ExtensionProperty(prop)
                    if prop.fqn == "sample.last" && prop.getter.is_some() && prop.setter.is_none())
            }),
            "extension property declaration should expose getter/setter contract: {decls:#?}"
        );
        assert!(
            output.stable_dump().contains("ExtensionPropertyDecl"),
            "stable dump should render declaration graph: {}",
            output.stable_dump()
        );
    }

    #[test]
    fn refactor_hir_decls_reports_extension_property_missing_getter_before_hir() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/extension_property_missing_getter.scoop",
            "package sample\nval String.bad: Int\nfun main(): Int { return \"\".bad }\n",
        );

        let err = run(&session, &source).expect_err("missing getter should be diagnosed");
        let HirLowerError::PropertyDecl(err) = err else {
            panic!("expected property declaration diagnostic, got {err:?}");
        };
        assert!(matches!(
            *err,
            crate::typecheck::PropertyDeclError::ExtensionPropertyGetterRequired { .. }
        ));
    }

    #[test]
    fn refactor_hir_splice_field_lowers_static_contracts_to_member_access() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_splice_field_static.scoop",
            r#"package sample
import scoop.core.*
struct Point(val x: Int, val y: Int)
fun get_x(p: Point): Int { return p.["x"] }
fun get_y(p: Point): Int { return p.[FieldMeta { name: "y" }] }
"#,
        );

        let output = run(&session, &source).expect("static splice field should lower to HIR");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("splice_field"),
            "dump must not contain splice Todo: {dump}"
        );
        assert!(
            dump.contains("fqn: \"sample.Point.x\"") && dump.contains("fqn: \"sample.Point.y\""),
            "splice fields should become resolved member accesses: {dump}"
        );
    }

    #[test]
    fn refactor_hir_splice_field_lowers_reflection_loop_field_meta() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_splice_field_loop.scoop",
            r#"package sample
import scoop.core.*
struct Point(val x: Int, val y: Int)
fun visit(p: Point) {
    comptime for (field in fieldsOf<Point>()) {
        val value = p.[field]
    }
}
"#,
        );

        let output = run(&session, &source).expect("comptime FieldMeta binder should lower to HIR");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("splice_field"),
            "dump must not contain splice Todo: {dump}"
        );
        assert!(
            dump.contains("fqn: \"sample.Point.x\"") && dump.contains("fqn: \"sample.Point.y\""),
            "reflection loop should unroll splice fields to concrete accesses: {dump}"
        );
    }

    #[test]
    fn refactor_hir_splice_field_reports_non_static_field_name() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_splice_field_dynamic.scoop",
            r#"package sample
import scoop.core.*
struct Point(val x: Int)
fun bad(p: Point, name: String): Any { return p.[name] }
"#,
        );

        let err = run(&session, &source).expect_err("dynamic splice field should be rejected");
        let HirLowerError::ExprType(err) = err else {
            panic!("expected splice field typecheck diagnostic, got {err:?}");
        };
        assert!(matches!(
            *err,
            crate::typecheck::ExprTypeError::SpliceFieldNameNotStatic { .. }
        ));
    }

    #[test]
    fn refactor_hir_call_args_canonicalizes_arrays_named_defaults_and_spread() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_call_args.scoop",
            r#"package sample
import scoop.core.*

fun choose<T>(value: T, count: Int = 1): T { return value }

class Box(val base: Int) {
    fun add(x: Int = 1, y: Int): Int { return x + y }
}

class Pair(val x: Int = 1, val y: Int)

fun Int.bump(delta: Int = 1): Int { return this + delta }

fun sum(prefix: Int, vararg xs: Int): Int { return prefix }

fun main(): Int {
    val a: Int = choose<Int>(count = 2, value = 1)
    val b: Int = Box(10).add(y = 2)
    val p: Pair = Pair(y = 2)
    val c: Int = 1.bump()
    val xs: Array<Int> = [1, 2]
    val empty: Array<Int> = []
    val d: Int = sum(0, *xs)
    return a + b + c + d
}
"#,
        );

        let output = run(&session, &source).expect("canonical call args should lower to HIR");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("array_lit"),
            "array literal Todo leaked: {dump}"
        );
        assert!(!dump.contains("named_arg"), "named arg Todo leaked: {dump}");
        assert!(
            !dump.contains("spread_arg"),
            "spread arg Todo leaked: {dump}"
        );
        assert!(
            !dump.contains("Named"),
            "HIR calls should use ordered positional args, not raw named args: {dump}"
        );
        assert!(
            dump.contains("__call_default") && dump.contains("__call_vararg"),
            "default and spread canonicalization should be visible in HIR dump: {dump}"
        );
    }

    #[test]
    fn refactor_hir_call_contracts_record_callable_provenance() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_call_contracts.scoop",
            r#"package sample
import scoop.core.*
import scoop.unsafe.*

effect Boom {
    fun boom(value: Int): Int
}

fun direct(x: Int): Int { return x }

fun Int.ext(delta: Int): Int { return this + delta }

open class Base() {
    open fun ping(): Int { return 1 }
}

interface IFace {
    fun foo(): Int
}

class Box(val value: Int)

object Singleton {
    fun get(): Int { return 3 }
}

@Extern("native_get_funptr")
fun getFunPtr(): FunPtr<() -> Int>

fun use(k: Continuation<Int, Unit, eff Pure>, b: Base, i: IFace): Int / Raise<RuntimeError> {
    val d: Int = direct(1)
    val e: Int = 1.ext(2)
    val m: Int = Singleton.get()
    val box: Box = Box(3)
    val v: Int = b.ping()
    val iface: Int = i.foo()
    val c: (Int) -> Int = { x -> x + 1 }
    val fv: Int = c(4)
    val cl: Int = ({ x: Int -> x + 2 })(5)
    val n: String = nameOf<Box>()
    val fp: FunPtr<() -> Int> = @Unsafe do { getFunPtr() }
    val p: Int = @Unsafe do { fp() }
    k.resume(1)
    val handled: Int = handle { Boom.boom(1) } with { Boom.boom(value: Int) -> value }
    return d + e + m + box.value + v + iface + fv + cl + p + handled
}
"#,
        );

        let output = run(&session, &source).expect("call contract fixture should lower");
        let contracts = output.effect_contracts().call_site_contracts();

        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::DirectTopLevel(target)
                if target.fqn() == "sample.direct"
        )));
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::Extension { function, .. }
                if function.fqn() == "sample.ext"
        )));
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::MemberDirect(member)
                if member.owner_fqn() == "sample.Singleton" && member.member_name() == "get"
        )));
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::Constructor(ctor) if ctor.owner_fqn() == "sample.Box"
        )));
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::Virtual(member)
                if member.owner_fqn() == "sample.Base" && member.member_name() == "ping"
        )));
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::Interface(member)
                if member.owner_fqn() == "sample.IFace" && member.member_name() == "foo"
        )));
        assert!(
            contracts
                .values()
                .any(|contract| matches!(contract, TypedCallSiteContract::FunValue { .. }))
        );
        assert!(
            contracts
                .values()
                .any(|contract| matches!(contract, TypedCallSiteContract::Closure { .. }))
        );
        assert!(
            contracts
                .values()
                .any(|contract| matches!(contract, TypedCallSiteContract::FunPtr { .. }))
        );
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::Intrinsic { function, .. }
                if function.fqn() == "scoop.core.nameOf"
        )));
        assert!(
            contracts
                .values()
                .any(|contract| matches!(contract, TypedCallSiteContract::ContinuationResume(_)))
        );
        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::EffectOp(perform)
                if perform.op_fqn() == "sample.Boom.boom"
        )));
    }

    #[test]
    fn refactor_hir_gc_intrinsic_member_calls_publish_intrinsic_contracts() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_gc_intrinsics.scoop",
            r#"package sample
import scoop.core.*

class Box(val value: Int)

fun main(): Unit {
    @Unsafe do {
        val box: Box = Box(value = 1)
        val pinned: Pinned = GC.pin(box)
        GC.unpin(pinned)
        val gcHandle: GcHandle = GC.handleNew(box)
        val value: Any = GC.handleGet(gcHandle)
        GC.handleDrop(gcHandle)
        val _ = value
    }
}
"#,
        );

        let output =
            run(&session, &source).expect("GC intrinsic member calls should lower through HIR");
        let contracts = output.effect_contracts().call_site_contracts();
        let mut saw_pin = false;
        let mut saw_unpin = false;
        let mut saw_handle_new = false;
        let mut saw_handle_get = false;
        let mut saw_handle_drop = false;

        for contract in contracts.values() {
            let TypedCallSiteContract::Intrinsic { kind, function } = contract else {
                continue;
            };
            assert_eq!(kind.allowed_context(), IntrinsicAllowedContext::RuntimeOnly);
            assert_eq!(
                kind.runtime_fallback(),
                IntrinsicRuntimeFallback::RuntimeIntrinsic
            );
            match function.fqn() {
                "scoop.core.GC.pin" => saw_pin = true,
                "scoop.core.GC.unpin" => saw_unpin = true,
                "scoop.core.GC.handleNew" => saw_handle_new = true,
                "scoop.core.GC.handleGet" => saw_handle_get = true,
                "scoop.core.GC.handleDrop" => saw_handle_drop = true,
                _ => {}
            }
        }

        assert!(saw_pin, "GC.pin intrinsic contract missing");
        assert!(saw_unpin, "GC.unpin intrinsic contract missing");
        assert!(saw_handle_new, "GC.handleNew intrinsic contract missing");
        assert!(saw_handle_get, "GC.handleGet intrinsic contract missing");
        assert!(saw_handle_drop, "GC.handleDrop intrinsic contract missing");
    }

    #[test]
    fn refactor_hir_ctor_contract_canonicalizes_default_args_to_ordered_slots() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_ctor_call_contracts.scoop",
            r#"package sample

class Pair(val first: Int = 7, val second: Int)

fun make(): Pair {
    return Pair(second = 6)
}

fun main(): Int {
    return make().first
}
"#,
        );

        let output = run(&session, &source).expect("constructor default args should lower");
        let contracts = output.effect_contracts().call_site_contracts();

        let ctor = contracts
            .values()
            .find_map(|contract| match contract {
                TypedCallSiteContract::Constructor(ctor) if ctor.owner_fqn() == "sample.Pair" => {
                    Some(ctor)
                }
                _ => None,
            })
            .expect("Pair ctor contract should be published");

        assert!(
            ctor.ctor_span().is_some(),
            "constructor contract must publish the selected ctor identity"
        );
        assert_eq!(
            ctor.arg_mapping(),
            [Some(0), Some(1)],
            "constructor contract must expose canonical ordered args after default-arg lowering"
        );
    }

    #[test]
    fn refactor_hir_call_contracts_publish_where_bound_member_dispatch() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_where_bound_call_contracts.scoop",
            r#"package sample
import scoop.core.*

fun <T> stringify(value: T): String where T: ToString {
    return value.toString()
}
"#,
        );

        let output = run(&session, &source).expect("where-bound member call should lower");
        let contracts = output.effect_contracts().call_site_contracts();

        assert!(contracts.values().any(|contract| matches!(
            contract,
            TypedCallSiteContract::Interface(member)
                if member.owner_fqn() == "scoop.core.ToString"
                    && member.member_name() == "toString"
        )));
    }

    #[test]
    fn refactor_hir_with_update_publishes_copy_update_contracts() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_with_update.scoop",
            r#"package sample
import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Result {
    Ok(val point: Point)
    Err(val code: Int)
}

fun use(pair: (Point, (Int, Int)), r: Result): Int {
    val point: Point = Point { x: 1, y: 2 } with { x: 10 }
    val tupled: (Point, (Int, Int)) = pair with { _0.x: 20, _1._0: 30 }
    val result: Result = r with { Ok.point.y: 40, Err.code: 50 }
    return point.x + tupled._0.y
}
"#,
        );

        let output = run(&session, &source).expect("with-update should lower through typed HIR");
        let contracts = output.effect_contracts().with_update_contracts();
        assert_eq!(contracts.len(), 3, "{contracts:#?}");
        assert!(contracts.values().any(|contract| {
            contract.aggregates.iter().any(|aggregate| {
                aggregate.prefix.is_empty()
                    && matches!(
                        &aggregate.kind,
                        ast::WithUpdateAggregateContractKind::Struct { .. }
                    )
            })
        }));
        assert!(contracts.values().any(|contract| {
            contract.aggregates.iter().any(|aggregate| {
                aggregate.prefix.is_empty()
                    && matches!(
                        &aggregate.kind,
                        ast::WithUpdateAggregateContractKind::Tuple { .. }
                    )
            }) && contract
                .aggregates
                .iter()
                .any(|aggregate| aggregate.prefix == "_0")
        }));
        assert!(contracts.values().any(|contract| {
            contract.aggregates.iter().any(|aggregate| {
                aggregate.prefix.is_empty()
                    && matches!(
                        &aggregate.kind,
                        ast::WithUpdateAggregateContractKind::Enum { .. }
                    )
            }) && contract
                .updates
                .iter()
                .any(|update| update.path == "Ok.point.y")
        }));

        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo(\"with_update\")"),
            "with-update Todo leaked: {dump}"
        );
        assert!(
            dump.contains("WithUpdateContract"),
            "contract missing: {dump}"
        );
        assert!(
            dump.contains("kind: Struct"),
            "struct contract missing: {dump}"
        );
        assert!(
            dump.contains("kind: Tuple"),
            "tuple contract missing: {dump}"
        );
        assert!(dump.contains("kind: Enum"), "enum contract missing: {dump}");
        assert!(
            dump.contains("path: \"Ok.point.y\""),
            "enum update path missing: {dump}"
        );
    }

    #[test]
    fn refactor_hir_places_publish_assignment_contracts() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_places.scoop",
            r#"package sample

import scoop.core.*

@ThreadLocal
var G: Int = 0

class Box(var value: Int)

fun entry(box: Box): Int {
    var x: Int = 1
    x = 2
    G = x
    box.value = G
    return box.value
}
"#,
        );

        let output = run(&session, &source).expect("assignment places should lower through HIR");
        let contracts = output.effect_contracts().assign_place_contracts();
        assert_eq!(contracts.len(), 3, "{contracts:#?}");
        assert!(contracts.values().any(|contract| matches!(
            &contract.kind,
            crate::hir::AssignPlaceKind::Local { name, .. } if name == "x"
        )));
        assert!(contracts.values().any(|contract| matches!(
            &contract.kind,
            crate::hir::AssignPlaceKind::TopLevel { fqn, .. } if fqn == "sample.G"
        )));
        assert!(contracts.values().any(|contract| matches!(
            &contract.kind,
            crate::hir::AssignPlaceKind::Member { member_fqn, .. } if member_fqn == "sample.Box.value"
        )));

        let dump = output.stable_dump();
        assert!(
            dump.contains("AssignPlaceContract")
                && dump.contains("kind: Local")
                && dump.contains("kind: TopLevel")
                && dump.contains("kind: Member"),
            "place contracts should be visible in typed HIR dump: {dump}"
        );

        let facts = crate::mir::MirLoweringFacts::from_refactor_typed_handoff(
            output.lowered_hir(),
            output.effect_contracts(),
        );
        let mut types = output.lowered_hir().types.clone();
        let mir = crate::mir::lower_hir_file_for_dump_with_facts(
            output.lowered_hir().builtins,
            &mut types,
            output.hir_file(),
            &output.lowered_hir().member_funs,
            &facts,
        );
        let mut saw_top_level_store = false;
        let mut saw_member_store = false;
        for item in &mir.items {
            let crate::mir::Item::Fun(fun) = item else {
                continue;
            };
            let Some(body) = &fun.body else {
                continue;
            };
            for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
                assert!(
                    !matches!(stmt.kind, crate::mir::StatementKind::Todo(_)),
                    "refactor MIR should consume assignment place contracts: {mir:#?}"
                );
                match &stmt.kind {
                    crate::mir::StatementKind::StoreTopLevelVar { fqn, .. }
                        if fqn == "sample.G" =>
                    {
                        saw_top_level_store = true;
                    }
                    crate::mir::StatementKind::StoreMember { member, .. }
                        if matches!(
                            member.resolved.as_ref(),
                            Some(crate::mir::MemberTarget::Value { fqn })
                                if fqn == "sample.Box.value"
                        ) =>
                    {
                        saw_member_store = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(
            saw_top_level_store,
            "top-level assignment store missing: {mir:#?}"
        );
        assert!(
            saw_member_store,
            "member assignment store missing: {mir:#?}"
        );
    }

    #[test]
    fn refactor_hir_for_loop_lowers_custom_iterator_contracts() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_for_loop_custom_iterator.scoop",
            r#"package sample

import scoop.core.*

interface MyIterator {
    fun next(): Option<Int>
}

interface MyIterable {
    fun iterator(): MyIterator
}

fun sum(xs: MyIterable): Int {
    var total: Int = 0
    for (x in xs) {
        total = total + x
    }
    return total
}
"#,
        );

        let output = run(&session, &source).expect("custom iterator for-loop should lower");
        let dump = output.stable_dump();
        assert!(!dump.contains("for_custom_iterator"), "Todo leaked: {dump}");
        assert!(!dump.contains("StmtKind::Todo"), "Todo leaked: {dump}");
        assert!(
            dump.contains("__for_iterable")
                && dump.contains("__for_iter")
                && dump.contains("__for_running")
                && dump.contains("When"),
            "custom iterator should lower to explicit loop/when form: {dump}"
        );

        let contracts = output.effect_contracts().call_site_contracts();
        let saw_iterator = contracts.values().any(|contract| {
            matches!(contract, TypedCallSiteContract::Interface(member) if member.member_fqn() == "sample.MyIterable.iterator")
        });
        let saw_next = contracts.values().any(|contract| {
            matches!(contract, TypedCallSiteContract::Interface(member) if member.member_fqn() == "sample.MyIterator.next")
        });
        assert!(
            saw_iterator && saw_next,
            "iterator/next dispatch contracts missing: {contracts:#?}"
        );
    }

    #[test]
    fn refactor_hir_for_loop_missing_contract_is_stage_error() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_for_loop_missing_contract.scoop",
            r#"package sample

import scoop.core.*

interface MyIterator {
    fun next(): Option<Int>
}

interface MyIterable {
    fun iterator(): MyIterator
}

fun use(xs: MyIterable) {
    for (x in xs) {}
}
"#,
        );

        let err = crate::hir::lower_for_dump(&session, &source)
            .expect_err("untyped HIR lowering lacks the for-loop contract");
        let HirLowerError::Stage(err) = err else {
            panic!("expected HIR stage error, got {err:?}");
        };
        assert_eq!(
            err.reason(),
            "for-loop missing typechecked iterator contract"
        );
        assert_eq!(err.owner(), "HIR for-loop lowering");
        assert_eq!(err.source_path(), source.path());
    }

    #[test]
    fn static_initializer_handle_is_stage_error() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/static_initializer_handle.scoop",
            r#"package sample

import scoop.core.*

effect Ask {
    fun ask(): Int
}

val Broken: Int = handle {
    Ask.ask()
} with {
    Ask.ask() -> 1
}

fun main(): Int {
    return Broken
}
"#,
        );

        let err = run(&session, &source)
            .expect_err("top-level static initializer should reject Handle/state-machine forms");
        let HirLowerError::Stage(err) = err else {
            panic!("expected HIR stage error, got {err:?}");
        };
        assert_eq!(err.reason(), "static initializer Handle");
        assert_eq!(err.owner(), "top-level val sample.Broken");
        assert_eq!(err.source_path(), source.path());
    }

    #[test]
    fn refactor_hir_top_level_init_publishes_storage_and_extern_roots() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_top_level_init.scoop",
            r#"package sample

import scoop.core.*

const val Base: Int = 1
val Runtime: Int = Base + 1

@Global
var Counter: Int = Runtime

@Extern(name = "native_counter")
var NativeCounter: Int

object Holder {
    val value: Int = Runtime
}

fun main() {}
"#,
        );

        let output = run(&session, &source).expect("top-level init roots should lower");
        let roots = output.effect_contracts().top_level_init_roots();
        assert!(roots.iter().any(|root| {
            root.fqn() == "sample.Base" && root.kind() == TopLevelInitRootKind::ConstVal
        }));
        assert!(roots.iter().any(|root| {
            root.fqn() == "sample.Runtime"
                && root.kind() == TopLevelInitRootKind::RuntimeImmutableVal
                && root
                    .dependencies()
                    .iter()
                    .any(|dep| dep.fqn() == "sample.Base")
        }));
        assert!(roots.iter().any(|root| {
            root.fqn() == "sample.Counter"
                && matches!(
                    root.kind(),
                    TopLevelInitRootKind::RuntimeMutableVar {
                        storage: crate::hir::TopLevelVarStorage::Global
                    }
                )
                && root
                    .dependencies()
                    .iter()
                    .any(|dep| dep.fqn() == "sample.Runtime")
        }));
        assert!(roots.iter().any(|root| {
            root.fqn() == "sample.Holder"
                && root.kind() == TopLevelInitRootKind::ObjectSingleton
                && root
                    .dependencies()
                    .iter()
                    .any(|dep| dep.fqn() == "sample.Runtime")
        }));

        let externs = output.effect_contracts().extern_global_contracts();
        assert_eq!(externs.len(), 1, "{externs:#?}");
        let native = &externs[0];
        assert_eq!(native.fqn(), "sample.NativeCounter");
        assert_eq!(native.symbol(), "native_counter");
        assert_eq!(native.linkage(), crate::hir::ExternGlobalLinkage::External);
        assert_eq!(native.storage(), crate::hir::TopLevelVarStorage::Global);
        assert!(native.initializer_absent());
        assert!(native.unsafe_required());
        assert!(native.mutable());

        let dump = output.stable_dump();
        assert!(
            dump.contains("top_level_init_roots") && dump.contains("extern_global_contracts"),
            "init/storage roots should be visible in typed HIR dump: {dump}"
        );
    }

    #[test]
    fn refactor_hir_class_literal_and_intrinsic_contracts() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_class_literal.scoop",
            r#"package sample
import scoop.core.*

struct Point(val x: Int)

val ClassName: String = Point::class

fun runtime(): String {
    val n: String = nameOf<Point>()
    val bytes: Int = sizeOf<Point>()
    val platform: Platform = getPlatform()
    return n
}
"#,
        );

        let output = run(&session, &source).expect("class literal fixture should lower to HIR");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("class_lit"),
            "class literal Todo leaked: {dump}"
        );
        assert!(
            dump.contains("ClassLiteral"),
            "class literal contract missing: {dump}"
        );
        assert!(
            dump.contains("source_fqn: Some") && dump.contains("\"sample.Point\""),
            "class literal source type FQN missing: {dump}"
        );
        assert!(
            dump.contains("metadata_kind: TypeNameString"),
            "class literal metadata kind missing: {dump}"
        );
        assert!(
            dump.contains("intrinsic_allowed_context: ComptimeAndRuntime"),
            "intrinsic allowed context missing: {dump}"
        );
        assert!(
            dump.contains("intrinsic_runtime_fallback: NormalRuntimeCall"),
            "reflection intrinsic runtime fallback missing: {dump}"
        );
        assert!(
            dump.contains("intrinsic_runtime_fallback: PlatformQuery"),
            "platform intrinsic runtime fallback missing: {dump}"
        );

        let contracts = output.effect_contracts().call_site_contracts();
        let mut saw_name_of = false;
        let mut saw_size_of = false;
        let mut saw_get_platform = false;
        for contract in contracts.values() {
            let TypedCallSiteContract::Intrinsic { kind, function } = contract else {
                continue;
            };
            match function.fqn() {
                "scoop.core.nameOf" => {
                    saw_name_of = true;
                    assert_eq!(
                        kind.allowed_context(),
                        IntrinsicAllowedContext::ComptimeAndRuntime
                    );
                    assert_eq!(
                        kind.runtime_fallback(),
                        IntrinsicRuntimeFallback::NormalRuntimeCall
                    );
                }
                "scoop.core.sizeOf" => {
                    saw_size_of = true;
                    assert_eq!(
                        kind.allowed_context(),
                        IntrinsicAllowedContext::ComptimeAndRuntime
                    );
                    assert_eq!(
                        kind.runtime_fallback(),
                        IntrinsicRuntimeFallback::NormalRuntimeCall
                    );
                }
                "scoop.core.getPlatform" => {
                    saw_get_platform = true;
                    assert_eq!(
                        kind.allowed_context(),
                        IntrinsicAllowedContext::ComptimeAndRuntime
                    );
                    assert_eq!(
                        kind.runtime_fallback(),
                        IntrinsicRuntimeFallback::PlatformQuery
                    );
                }
                _ => {}
            }
        }
        assert!(saw_name_of, "nameOf intrinsic contract missing: {dump}");
        assert!(saw_size_of, "sizeOf intrinsic contract missing: {dump}");
        assert!(
            saw_get_platform,
            "getPlatform intrinsic contract missing: {dump}"
        );
    }

    fn assert_fixture_effect_contract_dump(name: &str, expected: &str) {
        let session = refactor_session();
        let source = load_hir_fixture(name);
        let output = run(&session, &source).expect("fixture 应能通过 refactor typed HIR stage");

        assert_eq!(
            output.effect_contracts().stable_dump(output.types()),
            expected
        );
    }

    #[test]
    fn refactor_typed_hir_stage_output_is_constructible() {
        let session = refactor_session();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert_eq!(output.hir_file().items.len(), 1);
        assert!(!output.effect_contracts().is_placeholder());
        assert_eq!(output.effect_contracts().function_effects().len(), 1);
        assert!(
            output
                .effect_contracts()
                .continuation_resume_sites()
                .is_empty()
        );
        assert!(output.effect_contracts().perform_sites().is_empty());
        assert!(output.effect_contracts().handle_sites().is_empty());
        assert!(output.effect_contracts().call_site_kinds().is_empty());
    }

    #[test]
    fn refactor_typed_hir_stage_builds_explicit_contract_tables() {
        let session = refactor_session();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert!(!output.types().is_empty());
        assert!(!output.effect_contracts().is_placeholder());
        assert_eq!(
            output.effect_contracts().function_effects()[0].fqn(),
            "sample.main"
        );
        assert!(output.stable_dump().contains("TypedHirEffectContracts"));
    }

    #[test]
    fn refactor_typed_hir_records_resume_contracts_in_typed_hir_stage() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_continuation_contracts.scoop",
            r#"
package fixtures.hirstage

import scoop.core.*

fun resumeWithEffects(k: Continuation<Int, Int, eff Raise<Int>>): Int / (Raise<Int> + Raise<RuntimeError>) {
    return k.resume(1)
}
"#,
        );

        let output = run(&session, &source).unwrap();
        let contracts = output.effect_contracts();

        assert_eq!(contracts.continuation_resume_sites().len(), 1);
        let (call_site, contract) = contracts
            .continuation_resume_sites()
            .iter()
            .next()
            .expect("应收集到唯一的 continuation resume contract");

        assert_eq!(call_site.source_path, source.path());
        assert_eq!(
            contracts.call_site_kind(call_site),
            Some(TypedCallSiteKind::ContinuationResume)
        );
        assert_eq!(
            output.types().display(contract.receiver_ty()).to_string(),
            "scoop.core.Continuation<Int, Int, eff scoop.core.Raise<Int>>"
        );
        assert_eq!(
            output.types().display(contract.resume_ty()).to_string(),
            "Int"
        );
        assert_eq!(
            output.types().display(contract.answer_ty()).to_string(),
            "Int"
        );
        assert_eq!(
            output.types().display(contract.return_ty()).to_string(),
            "Int"
        );
        assert_eq!(contract.out_effects().terms.len(), 1);
        assert_eq!(
            output
                .types()
                .display(contract.out_effects().terms[0])
                .to_string(),
            "scoop.core.Raise<Int>"
        );
        assert_eq!(
            output
                .types()
                .display(contract.runtime_error_effect_ty().unwrap())
                .to_string(),
            "scoop.core.Raise<scoop.core.RuntimeError>"
        );
        assert!(contract.required_effects_include_runtime_error());
    }

    #[test]
    fn refactor_typed_hir_continuation_contract_dump_snapshot() {
        assert_fixture_effect_contract_dump(
            "continuation_resume_surface_named_tuple_and_unit_basic.scoop",
            r#"TypedHirEffectContracts {
    function_effects: [
        FunctionEffectContract {
            span: 233..351,
            fqn: "fixtures.hir.resumePair",
            return_ty: Unit,
            allowed_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            effects_closed: false,
        },
        FunctionEffectContract {
            span: 80..231,
            fqn: "fixtures.hir.resumeUnit",
            return_ty: Unit,
            allowed_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            effects_closed: false,
        },
        FunctionEffectContract {
            span: 43..78,
            fqn: "fixtures.hir.takesUnit",
            return_ty: Unit,
            allowed_effects: Pure,
            effects_closed: false,
        },
    ],
    call_site_kinds: [
        TypedCallSiteContract {
            span: 168..178,
            kind: ContinuationResume,
        },
        TypedCallSiteContract {
            span: 183..195,
            kind: ContinuationResume,
        },
        TypedCallSiteContract {
            span: 200..211,
            kind: DirectTopLevel,
        },
        TypedCallSiteContract {
            span: 216..229,
            kind: DirectTopLevel,
        },
        TypedCallSiteContract {
            span: 330..349,
            kind: ContinuationResume,
        },
    ],
    call_site_contracts: [
        CallSiteContract {
            span: 168..178,
            kind: ContinuationResume,
            receiver_route: CallArg { index: 0 },
            payload_arg_indices: [1],
            receiver_ty: scoop.core.Continuation<Unit, Unit, eff Pure>,
            resume_ty: Unit,
            answer_ty: Unit,
            out_effects: Pure,
        },
        CallSiteContract {
            span: 183..195,
            kind: ContinuationResume,
            receiver_route: CallArg { index: 0 },
            payload_arg_indices: [1],
            receiver_ty: scoop.core.Continuation<Unit, Unit, eff Pure>,
            resume_ty: Unit,
            answer_ty: Unit,
            out_effects: Pure,
        },
        CallSiteContract {
            span: 200..211,
            kind: DirectTopLevel,
            target_fqn: "fixtures.hir.takesUnit",
            target_decl_span: Some(47..56),
            target_type_args: [],
            target_eff_args: [],
            target_arg_binding: [Explicit(CallArgElementContract { arg_index: 0, spread: false })],
        },
        CallSiteContract {
            span: 216..229,
            kind: DirectTopLevel,
            target_fqn: "fixtures.hir.takesUnit",
            target_decl_span: Some(47..56),
            target_type_args: [],
            target_eff_args: [],
            target_arg_binding: [Explicit(CallArgElementContract { arg_index: 0, spread: false })],
        },
        CallSiteContract {
            span: 330..349,
            kind: ContinuationResume,
            receiver_route: CallArg { index: 0 },
            payload_arg_indices: [1],
            receiver_ty: scoop.core.Continuation<(Int, String), Unit, eff Pure>,
            resume_ty: (Int, String),
            answer_ty: Unit,
            out_effects: Pure,
        },
    ],
    with_update_contracts: [
    ],
    assign_place_contracts: [
    ],
    continuation_resume_sites: [
        ContinuationResumeSiteContract {
            span: 168..178,
            receiver_route: CallArg { index: 0 },
            payload_arg_indices: [1],
            receiver_ty: scoop.core.Continuation<Unit, Unit, eff Pure>,
            resume_ty: Unit,
            answer_ty: Unit,
            return_ty: Unit,
            out_effects: Pure,
            required_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            includes_runtime_error_effect: true,
        },
        ContinuationResumeSiteContract {
            span: 183..195,
            receiver_route: CallArg { index: 0 },
            payload_arg_indices: [1],
            receiver_ty: scoop.core.Continuation<Unit, Unit, eff Pure>,
            resume_ty: Unit,
            answer_ty: Unit,
            return_ty: Unit,
            out_effects: Pure,
            required_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            includes_runtime_error_effect: true,
        },
        ContinuationResumeSiteContract {
            span: 330..349,
            receiver_route: CallArg { index: 0 },
            payload_arg_indices: [1],
            receiver_ty: scoop.core.Continuation<(Int, String), Unit, eff Pure>,
            resume_ty: (Int, String),
            answer_ty: Unit,
            return_ty: Unit,
            out_effects: Pure,
            required_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            includes_runtime_error_effect: true,
        },
    ],
    perform_sites: [
    ],
    handle_sites: [
    ],
}"#,
        );
    }

    #[test]
    fn refactor_typed_hir_runtime_error_contract_dump_records_required_effect() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/runtime_error_contract.scoop",
            r#"
package fixtures.hir

import scoop.core.*

fun resumeWithEffects(k: Continuation<Int, Int, eff Raise<Int>>): Int / (Raise<Int> + Raise<RuntimeError>) {
    return k.resume(1)
}
"#,
        );

        let output = run(&session, &source).unwrap();
        let rendered = output.effect_contracts().stable_dump(output.types());

        assert!(rendered.contains("out_effects: scoop.core.Raise<Int>"));
        assert!(rendered.contains(
            "required_effects: scoop.core.Raise<Int> + scoop.core.Raise<scoop.core.RuntimeError>"
        ));
        assert!(rendered.contains("includes_runtime_error_effect: true"));
    }

    #[test]
    fn refactor_typed_hir_handle_contract_dump_snapshot() {
        assert_fixture_effect_contract_dump(
            "handle_perform.scoop",
            r#"TypedHirEffectContracts {
    function_effects: [
        FunctionEffectContract {
            span: 36..125,
            fqn: "a.main",
            return_ty: Int,
            allowed_effects: Pure,
            effects_closed: false,
        },
    ],
    call_site_kinds: [
        TypedCallSiteContract {
            span: 64..78,
            kind: EffectOp,
        },
    ],
    call_site_contracts: [
        CallSiteContract {
            span: 64..78,
            kind: EffectOp,
            effect_ty: scoop.core.Raise<Int>,
            op_fqn: "scoop.core.Raise.raise",
            result_ty: Nothing,
            payload_ty: Int,
            arg_mapping: [0],
        },
    ],
    with_update_contracts: [
    ],
    assign_place_contracts: [
    ],
    continuation_resume_sites: [
    ],
    perform_sites: [
        PerformSiteContract {
            span: 64..78,
            effect_ty: scoop.core.Raise<Int>,
            op_fqn: "scoop.core.Raise.raise",
            result_ty: Nothing,
            payload_ty: Int,
            payload_components: [
                Int,
            ],
            arg_mapping: [0],
        },
    ],
    handle_sites: [
        HandleSiteContract {
            span: 51..123,
            result_ty: Int,
            body_result_ty: Int,
            arm_contracts: [
                HandleArmSiteContract {
                    op_fqn: "scoop.core.Raise.raise",
                    handled_effect_ty: scoop.core.Raise<Int>,
                    payload_ty: Int,
                    payload_components: [
                        Int,
                    ],
                    body_ty: Int,
                    kind: NonResuming,
                },
            ],
            finally_result_ty: None,
        },
    ],
}"#,
        );
    }

    #[test]
    fn refactor_typed_hir_collects_perform_and_handle_contracts() {
        let session = refactor_session();
        let source = load_hir_fixture("handle_perform.scoop");

        let output = run(&session, &source).unwrap();
        let contracts = output.effect_contracts();

        assert_eq!(contracts.perform_sites().len(), 1);
        let (perform_site, perform_contract) = contracts
            .perform_sites()
            .iter()
            .next()
            .expect("应收集到 perform site");
        assert_eq!(
            contracts.call_site_kind(perform_site),
            Some(TypedCallSiteKind::EffectOp)
        );
        assert_eq!(perform_contract.op_fqn(), "scoop.core.Raise.raise");
        assert_eq!(perform_contract.payload().components().len(), 1);
        assert_eq!(
            output
                .types()
                .display(perform_contract.payload().components()[0])
                .to_string(),
            "Int"
        );

        assert_eq!(contracts.handle_sites().len(), 1);
        let handle_contract = contracts
            .handle_sites()
            .values()
            .next()
            .expect("应收集到 handle site");
        assert_eq!(
            output
                .types()
                .display(handle_contract.result_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(
            output
                .types()
                .display(handle_contract.body_result_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(handle_contract.arm_contracts().len(), 1);
        let arm = &handle_contract.arm_contracts()[0];
        assert_eq!(arm.op_fqn(), "scoop.core.Raise.raise");
        assert_eq!(arm.kind(), HandleArmContractKind::NonResuming);
        assert_eq!(
            output.types().display(arm.handled_effect_ty()).to_string(),
            "scoop.core.Raise<Int>"
        );
    }
}
