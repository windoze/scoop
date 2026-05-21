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

use crate::ast;
use crate::expr_facts::HirFactResolver;
use crate::hir;
use crate::pipeline::{
    CallArgBindingContract, CallArgElementContract, CallArgParamContract,
    ConstructorCallTargetContract, ContinuationResumeReceiverRoute, ExternGlobalContract,
    FunctionTargetContract, MemberCallTargetContract, TopLevelInitDependency,
    TopLevelInitDependencyKind, TopLevelInitRootContract, TopLevelInitRootKind,
    TypedCallSiteContract, TypedIntrinsicKind,
};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
    is_builtin_scalar_nominal_value_type,
};
use scoopc_hir_facts::{HirFacts, source_sites as hir_site_facts};

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
pub(crate) struct MirLoweringFacts {
    dispatch_call_sites: HashMap<hir::DispatchCallSite, DispatchTargetKind>,
    call_arg_bindings: HashMap<hir::CallSite, CallArgBindingContract>,
    resume_sites: HashMap<hir::CallSite, ResumeCallInfo>,
    perform_sites: HashMap<hir::CallSite, PerformMetadata>,
    handle_sites: HashMap<hir::CallSite, HandleSiteInfo>,
    call_sites: HashMap<hir::CallSite, TypedCallSiteContract>,
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
