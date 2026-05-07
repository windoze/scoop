//! typed/lowered HIR → generic early MIR / ANF template lowering。
//!
//! 说明：
//! - 当前入口仍主要服务 `scoop dump-mir` 与 `tests/fixtures/mir/**` 的回归；
//! - lowering 会显式消费 typed/shared HIR side tables，把 dispatch / resume / perform / pattern
//!   等语言级事实收口到 MIR；
//! - 这里不负责 materialize monomorphic instance，也不编码 LLVM/backend-specific 细节；
//! - 未覆盖的表达式/语句继续以 `Todo(...)` 占位，优先保证边界清晰、输出稳定、不 panic。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::effect_refactor_pipeline::{
    CallArgBindingContract, CallArgParamContract, ContinuationResumeReceiverRoute,
    ExternGlobalContract, FunctionTargetContract, HandleArmContractKind, MemberCallTargetContract,
    TopLevelInitDependencyKind, TopLevelInitRootContract, TopLevelInitRootKind,
    TypedCallSiteContract, TypedHirEffectContracts, TypedIntrinsicKind,
};
use crate::hir;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};

use super::{
    AccessorMetadata, AggregateTransportField, AggregateTransportKind, AggregateTransportMetadata,
    ArrayElementTransportMetadata, ArrayTransportOperation, BasicBlock, BasicBlockId, Body,
    CallAbiHandoffMetadata, CallArg, CallKind, CallTransportMetadata, CaptureBoxTransportMetadata,
    ClassCtorCallMetadata, ClosureCaptureTransportMetadata, ClosureEnvTransportMetadata,
    ConstValue, CtorMetadata, CtorParamMetadata, DeclMemberMetadata, DeclTypeParamMetadata,
    DispatchMetadata, EnumVariantMetadata, ExtensionPropertyMetadata, ExternGlobalRoot,
    FieldMetadata, File, FunDecl, GcIntrinsicOperation, GcIntrinsicPairing,
    GcIntrinsicTransportMetadata, GcRootLifetime, HandleMetadata, HandlerArm, HandlerArmKind,
    InitializerDependency, InitializerDependencyKind, InitializerRoot, InitializerRootKind,
    InterpolatedStringPart, Item, LocalDecl, LocalId, LocalSourceKind, MemberAccessMetadata,
    MemberFunMetadata, MemberTarget, MetadataRoot, MirBoxingIntent, MirBoxingReason,
    MirTransportKind, MirTransportRequirements, MirValidationError, NominalMetadata,
    ObjectMetadata, Operand, Param, Pattern, PatternBindingStep, PerformArg, PerformMetadata,
    PropertyMetadata, ResumeMetadata, RuntimeCastFailure, RuntimeCastMetadata, RuntimeCastResult,
    RuntimePatternTypeTestKind, RuntimePatternTypeTestMetadata, RuntimeTypeDescriptorKey,
    RuntimeTypeDescriptorKind, RuntimeTypeParameterizedMatch, RuntimeTypeStaticFold,
    RuntimeTypeTestMetadata, Rvalue, SiteId, Statement, StatementKind,
    StoredContinuationRoutePublication, StoredContinuationValueRoute, SupertypeMetadata,
    Terminator, TerminatorKind, TopLevelRef, TypeAliasMetadata, TypeMetadataLiteral,
    TypeMetadataLiteralKind, UnwindAction, ValueTransportMetadata,
};

/// MIR lowering 需要消费的最小共享事实。
///
/// 目标：
/// - 把 HIR/typecheck 已确认的调用语义收口成 MIR lowering 可直接查询的 backend-agnostic 输入；
/// - 避免 MIR 阶段重新回到 LLVM vtable/itable 细节或 `Continuation.resume` 名字推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MirSiteContractSource {
    LegacyFallbacks,
    RefactorTyped,
}

#[derive(Debug, Clone)]
pub(crate) struct MirLoweringFacts {
    site_contract_source: MirSiteContractSource,
    dispatch_call_sites: HashMap<hir::DispatchCallSite, DispatchTargetKind>,
    call_arg_bindings: HashMap<hir::CallSite, CallArgBindingContract>,
    legacy_resume_site_spans: HashSet<Span>,
    legacy_outward_resume_site_spans: HashSet<Span>,
    legacy_perform_sites: HashMap<Span, PerformCallSiteInfo>,
    refactor_resume_sites: HashMap<hir::CallSite, RefactorResumeCallInfo>,
    refactor_perform_sites: HashMap<hir::CallSite, PerformMetadata>,
    refactor_handle_sites: HashMap<hir::CallSite, RefactorHandleSiteInfo>,
    refactor_dispatch_sites: HashMap<hir::CallSite, RefactorDispatchCallInfo>,
    refactor_call_sites: HashMap<hir::CallSite, TypedCallSiteContract>,
    refactor_assign_places: HashMap<hir::CallSite, hir::AssignPlaceContract>,
    class_ctor_call_sites: HashMap<hir::CallSite, hir::CtorCallInfo>,
    class_ctor_hidden_effects: HashMap<hir::CallSite, EffectRow>,
    object_member_hidden_effects: HashMap<String, EffectRow>,
    top_level_ref_hidden_effects: HashMap<String, EffectRow>,
    refactor_top_level_init_roots: Vec<TopLevelInitRootContract>,
    refactor_extern_global_contracts: Vec<ExternGlobalContract>,
    when_pat_binding_tys: HashMap<Span, TypeId>,
    nominal_kinds: HashMap<String, ast::TypeKind>,
    top_level_fun_call_sites: HashMap<hir::CallSite, ast::TopLevelFunCallBinding>,
    member_value_tys: HashMap<String, TypeId>,
    continuation_identity_return_funs: HashMap<String, usize>,
}

impl Default for MirLoweringFacts {
    fn default() -> Self {
        Self {
            site_contract_source: MirSiteContractSource::LegacyFallbacks,
            dispatch_call_sites: HashMap::new(),
            call_arg_bindings: HashMap::new(),
            legacy_resume_site_spans: HashSet::new(),
            legacy_outward_resume_site_spans: HashSet::new(),
            legacy_perform_sites: HashMap::new(),
            refactor_resume_sites: HashMap::new(),
            refactor_perform_sites: HashMap::new(),
            refactor_handle_sites: HashMap::new(),
            refactor_dispatch_sites: HashMap::new(),
            refactor_call_sites: HashMap::new(),
            refactor_assign_places: HashMap::new(),
            class_ctor_call_sites: HashMap::new(),
            class_ctor_hidden_effects: HashMap::new(),
            object_member_hidden_effects: HashMap::new(),
            top_level_ref_hidden_effects: HashMap::new(),
            refactor_top_level_init_roots: Vec::new(),
            refactor_extern_global_contracts: Vec::new(),
            when_pat_binding_tys: HashMap::new(),
            nominal_kinds: HashMap::new(),
            top_level_fun_call_sites: HashMap::new(),
            member_value_tys: HashMap::new(),
            continuation_identity_return_funs: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchTargetKind {
    Virtual,
    Interface,
}

#[derive(Debug, Clone)]
struct PerformCallSiteInfo {
    arg_mapping: Vec<usize>,
    payload_tuple_ty: Option<TypeId>,
}

#[derive(Debug, Clone)]
struct RefactorHandleSiteInfo {
    metadata: HandleMetadata,
    arms: Vec<HandlerArm>,
}

#[derive(Debug, Clone)]
struct RefactorResumeCallInfo {
    receiver_route: ContinuationResumeReceiverRoute,
    payload_arg_indices: Vec<usize>,
    metadata: ResumeMetadata,
}

#[derive(Debug, Clone)]
struct RefactorDispatchCallInfo {
    kind: DispatchTargetKind,
    owner_fqn: String,
    member_name: String,
    member_fqn: String,
    member_decl_span: Option<Span>,
    receiver_ty: TypeId,
}

fn refactor_dispatch_call_info(
    kind: DispatchTargetKind,
    member: &MemberCallTargetContract,
) -> RefactorDispatchCallInfo {
    RefactorDispatchCallInfo {
        kind,
        owner_fqn: member.owner_fqn().to_string(),
        member_name: member.member_name().to_string(),
        member_fqn: member.member_fqn().to_string(),
        member_decl_span: member.function().decl_span(),
        receiver_ty: member.receiver_ty(),
    }
}

fn call_arg_expr(arg: &hir::CallArg) -> &hir::Expr {
    match arg {
        hir::CallArg::Positional(expr) => expr,
        hir::CallArg::Named { value, .. } => value,
    }
}

fn lowered_call_arg_binding_contract(binding: &ast::CallArgBinding) -> CallArgBindingContract {
    CallArgBindingContract::new(
        binding
            .params
            .iter()
            .map(|param| match param {
                ast::CallArgParamBinding::Receiver => CallArgParamContract::Receiver,
                ast::CallArgParamBinding::Explicit(element) => CallArgParamContract::Explicit(
                    crate::effect_refactor_pipeline::CallArgElementContract::new(
                        element.arg_index,
                        element.spread,
                    ),
                ),
                ast::CallArgParamBinding::Default => CallArgParamContract::Default,
                ast::CallArgParamBinding::Vararg(elements) => CallArgParamContract::Vararg(
                    elements
                        .iter()
                        .map(|element| {
                            crate::effect_refactor_pipeline::CallArgElementContract::new(
                                element.arg_index,
                                element.spread,
                            )
                        })
                        .collect(),
                ),
            })
            .collect(),
    )
}

fn call_arg_binding_has_receiver(binding: &CallArgBindingContract) -> bool {
    binding
        .params()
        .iter()
        .any(|param| matches!(param, CallArgParamContract::Receiver))
}

impl MirLoweringFacts {
    pub(crate) fn from_lowered_hir(lowered: &hir::LoweredHir) -> Self {
        Self::from_hir_side_tables_and_resume_spans(
            &lowered.dispatch_call_sites,
            lowered
                .continuation_resume_call_sites
                .iter()
                .map(|site| site.span),
            lowered
                .non_pure_continuation_resume_call_sites
                .iter()
                .map(|site| site.span),
            &lowered.effect_op_call_sites,
            &lowered.when_pat_binding_tys,
            &lowered.top_level_fun_call_sites,
        )
        .with_call_arg_bindings(lowered)
        .with_member_value_types(lowered)
        .with_nominal_kinds(lowered)
        .with_class_ctor_call_sites(lowered)
        .with_continuation_identity_return_funs(lowered)
        .with_class_ctor_hidden_effects(lowered)
    }

    pub(crate) fn from_refactor_typed_handoff(
        lowered: &hir::LoweredHir,
        contracts: &TypedHirEffectContracts,
    ) -> Self {
        let mut facts = Self::default();

        for (site, kind) in &lowered.dispatch_call_sites {
            facts.dispatch_call_sites.insert(
                site.clone(),
                match kind {
                    hir::DispatchCallKind::Virtual => DispatchTargetKind::Virtual,
                    hir::DispatchCallKind::Interface => DispatchTargetKind::Interface,
                },
            );
        }

        for (site, ty) in &lowered.when_pat_binding_tys {
            facts.when_pat_binding_tys.insert(site.decl_span, *ty);
        }

        facts
            .top_level_fun_call_sites
            .extend(lowered.top_level_fun_call_sites.clone());
        facts = facts
            .with_call_arg_bindings(lowered)
            .with_member_value_types(lowered)
            .with_nominal_kinds(lowered)
            .with_class_ctor_call_sites(lowered)
            .with_continuation_identity_return_funs(lowered)
            .with_class_ctor_hidden_effects(lowered);

        facts.with_refactor_typed_contracts(contracts)
    }

    pub(crate) fn from_hir_side_tables_and_resume_spans(
        dispatch_call_sites: &hir::DispatchCallSiteIndex,
        legacy_resume_site_spans: impl IntoIterator<Item = Span>,
        legacy_outward_resume_site_spans: impl IntoIterator<Item = Span>,
        effect_op_call_sites: &hir::EffectOpCallSiteIndex,
        when_pat_binding_tys: &hir::WhenPatBindingTypeIndex,
        top_level_fun_call_sites: &hir::TopLevelFunCallSiteIndex,
    ) -> Self {
        let mut facts = Self::default();

        for (site, kind) in dispatch_call_sites {
            facts.dispatch_call_sites.insert(
                site.clone(),
                match kind {
                    hir::DispatchCallKind::Virtual => DispatchTargetKind::Virtual,
                    hir::DispatchCallKind::Interface => DispatchTargetKind::Interface,
                },
            );
        }

        facts.legacy_resume_site_spans = legacy_resume_site_spans.into_iter().collect();
        facts.legacy_outward_resume_site_spans =
            legacy_outward_resume_site_spans.into_iter().collect();
        facts.with_hir_side_tables(
            effect_op_call_sites,
            when_pat_binding_tys,
            top_level_fun_call_sites,
        )
    }

    pub(crate) fn with_hir_side_tables(
        mut self,
        effect_op_call_sites: &hir::EffectOpCallSiteIndex,
        when_pat_binding_tys: &hir::WhenPatBindingTypeIndex,
        top_level_fun_call_sites: &hir::TopLevelFunCallSiteIndex,
    ) -> Self {
        for (site, info) in effect_op_call_sites {
            self.legacy_perform_sites.insert(
                site.span,
                PerformCallSiteInfo {
                    arg_mapping: info.arg_mapping.clone(),
                    payload_tuple_ty: info.payload_tuple_ty,
                },
            );
        }

        for (site, ty) in when_pat_binding_tys {
            self.when_pat_binding_tys.insert(site.decl_span, *ty);
        }

        self.top_level_fun_call_sites
            .extend(top_level_fun_call_sites.clone());

        self
    }

    fn with_member_value_types(mut self, lowered: &hir::LoweredHir) -> Self {
        for class in lowered.class_inits.values() {
            for field in &class.fields {
                if Self::member_fqn_matches_owner(&field.fqn, &class.fqn) {
                    self.member_value_tys
                        .entry(field.fqn.clone())
                        .or_insert(field.ty);
                }
            }
        }

        for layout in lowered.struct_layouts.values() {
            for field in &layout.fields {
                if let Some(ty) = field.ty
                    && Self::member_fqn_matches_owner(&field.fqn, &layout.fqn)
                {
                    self.member_value_tys.entry(field.fqn.clone()).or_insert(ty);
                }
            }
        }

        for object in lowered.object_inits.values() {
            for property in object.properties.values() {
                self.member_value_tys
                    .insert(format!("{}.{}", object.fqn, property.name), property.ty);
            }
        }

        self
    }

    fn with_call_arg_bindings(mut self, lowered: &hir::LoweredHir) -> Self {
        self.call_arg_bindings.extend(
            lowered
                .call_arg_bindings
                .iter()
                .map(|(site, binding)| (site.clone(), lowered_call_arg_binding_contract(binding))),
        );
        self
    }

    fn member_fqn_matches_owner(member_fqn: &str, owner_fqn: &str) -> bool {
        member_fqn
            .strip_prefix(owner_fqn)
            .is_some_and(|suffix| suffix.starts_with('.'))
    }

    fn with_class_ctor_call_sites(mut self, lowered: &hir::LoweredHir) -> Self {
        self.class_ctor_call_sites
            .extend(lowered.ctor_call_sites.clone());
        self
    }

    fn with_nominal_kinds(mut self, lowered: &hir::LoweredHir) -> Self {
        self.nominal_kinds.extend(lowered.nominal_kinds.clone());
        self
    }

    fn with_continuation_identity_return_funs(mut self, lowered: &hir::LoweredHir) -> Self {
        for item in &lowered.file.items {
            if let hir::Item::Fun(fun) = item
                && let Some(param_index) = continuation_identity_return_param(&lowered.types, fun)
            {
                self.continuation_identity_return_funs
                    .insert(fun.fqn.clone(), param_index);
            }
        }
        for fun in &lowered.member_funs {
            if let Some(param_index) = continuation_identity_return_param(&lowered.types, fun) {
                self.continuation_identity_return_funs
                    .insert(fun.fqn.clone(), param_index);
            }
        }

        self
    }

    fn with_class_ctor_hidden_effects(mut self, lowered: &hir::LoweredHir) -> Self {
        let analyzer = HiddenInitEffectAnalyzer::new(lowered);
        for (site, info) in &lowered.ctor_call_sites {
            let effects = analyzer.class_ctor_effect_row(&info.class_fqn, info.ctor_span);
            if !effects.is_pure() {
                self.class_ctor_hidden_effects.insert(site.clone(), effects);
            }
        }
        for object in lowered.object_inits.values() {
            let effects = analyzer.object_init_effect_row(&object.fqn);
            if effects.is_pure() {
                continue;
            }
            self.top_level_ref_hidden_effects
                .insert(object.fqn.clone(), effects.clone());
            for property_name in object.properties.keys() {
                self.object_member_hidden_effects
                    .insert(format!("{}.{}", object.fqn, property_name), effects.clone());
            }
        }
        for value in lowered.top_level_immutable_values.values() {
            let effects = analyzer.top_level_immutable_value_effect_row(&value.fqn);
            if !effects.is_pure() {
                self.top_level_ref_hidden_effects
                    .insert(value.fqn.clone(), effects);
            }
        }
        self
    }

    pub(crate) fn with_refactor_typed_contracts(
        mut self,
        contracts: &TypedHirEffectContracts,
    ) -> Self {
        self.site_contract_source = MirSiteContractSource::RefactorTyped;
        self.legacy_resume_site_spans.clear();
        self.legacy_outward_resume_site_spans.clear();
        self.legacy_perform_sites.clear();
        self.refactor_resume_sites.clear();
        self.refactor_perform_sites.clear();
        self.refactor_handle_sites.clear();
        self.refactor_dispatch_sites.clear();
        self.refactor_call_sites.clear();
        self.refactor_assign_places.clear();
        self.refactor_top_level_init_roots = contracts.top_level_init_roots().to_vec();
        self.refactor_extern_global_contracts = contracts.extern_global_contracts().to_vec();

        for (call_site, contract) in contracts.continuation_resume_sites() {
            self.refactor_resume_sites.insert(
                call_site.clone(),
                RefactorResumeCallInfo {
                    receiver_route: contract.receiver_route(),
                    payload_arg_indices: contract.payload_arg_indices().to_vec(),
                    metadata: ResumeMetadata {
                        continuation_ty: contract.receiver_ty(),
                        resume_ty: contract.resume_ty(),
                        answer_ty: contract.answer_ty(),
                        return_ty: contract.return_ty(),
                        out_effects: contract.out_effects().clone(),
                        runtime_error_effect_ty: contract.runtime_error_effect_ty(),
                        suspends_outward: !contract.out_effects().is_pure(),
                    },
                },
            );
        }

        for (call_site, contract) in contracts.perform_sites() {
            self.refactor_perform_sites.insert(
                call_site.clone(),
                PerformMetadata {
                    effect_ty: contract.effect_ty(),
                    result_ty: contract.result_ty(),
                    payload_tuple_ty: contract.payload().ty(),
                    payload_component_tys: contract.payload().components().to_vec(),
                    payload_transport: Vec::new(),
                    arg_mapping: contract.arg_mapping().to_vec(),
                },
            );
        }

        for (call_site, contract) in contracts.handle_sites() {
            let arms = contract
                .arm_contracts()
                .iter()
                .map(|arm| HandlerArm {
                    op_fqn: arm.op_fqn().to_string(),
                    binder_count: arm.payload().components().len(),
                    binder_locals: Vec::new(),
                    continuation_local: None,
                    handled_effect_ty: arm.handled_effect_ty(),
                    payload_tuple_ty: arm.payload().ty(),
                    payload_component_tys: arm.payload().components().to_vec(),
                    body_ty: arm.body_ty(),
                    kind: match arm.kind() {
                        HandleArmContractKind::NonResuming => HandlerArmKind::NonResuming,
                        HandleArmContractKind::EscapeContinuation => {
                            HandlerArmKind::EscapeContinuation
                        }
                    },
                })
                .collect();
            self.refactor_handle_sites.insert(
                call_site.clone(),
                RefactorHandleSiteInfo {
                    metadata: HandleMetadata {
                        result_ty: contract.result_ty(),
                        body_result_ty: contract.body_result_ty(),
                        finally_result_ty: contract.finally_result_ty(),
                    },
                    arms,
                },
            );
        }

        for (call_site, contract) in contracts.call_site_contracts() {
            self.refactor_call_sites
                .insert(call_site.clone(), contract.clone());
            let (kind, member) = match contract {
                TypedCallSiteContract::Virtual(member) => (DispatchTargetKind::Virtual, member),
                TypedCallSiteContract::Interface(member) => (DispatchTargetKind::Interface, member),
                _ => continue,
            };
            self.refactor_dispatch_sites
                .insert(call_site.clone(), refactor_dispatch_call_info(kind, member));
        }

        self.refactor_assign_places
            .extend(contracts.assign_place_contracts().clone());

        self
    }

    fn uses_refactor_typed_contracts(&self) -> bool {
        self.site_contract_source == MirSiteContractSource::RefactorTyped
    }

    fn nominal_kind(&self, fqn: &str) -> Option<ast::TypeKind> {
        self.nominal_kinds.get(fqn).copied()
    }

    fn dispatch_target_kind(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
        receiver_ty: TypeId,
    ) -> Option<DispatchTargetKind> {
        self.dispatch_call_sites
            .get(&hir::DispatchCallSite::new(
                source_path.to_path_buf(),
                call_span,
                receiver_ty,
            ))
            .copied()
    }

    fn refactor_dispatch_contract(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&RefactorDispatchCallInfo> {
        self.refactor_dispatch_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    fn refactor_assign_place_contract(
        &self,
        source_path: &std::path::Path,
        assign_span: Span,
    ) -> Option<&hir::AssignPlaceContract> {
        self.refactor_assign_places
            .get(&hir::CallSite::new(source_path.to_path_buf(), assign_span))
    }

    fn refactor_call_site_contract(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&TypedCallSiteContract> {
        self.refactor_call_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    fn class_ctor_call_info(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&hir::CtorCallInfo> {
        self.class_ctor_call_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    fn top_level_fun_call_binding(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&ast::TopLevelFunCallBinding> {
        self.top_level_fun_call_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    fn call_arg_binding(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&CallArgBindingContract> {
        self.call_arg_bindings
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    fn continuation_identity_return_param(&self, fqn: &str) -> Option<usize> {
        self.continuation_identity_return_funs.get(fqn).copied()
    }

    fn legacy_resume_site_matches(&self, span: Span) -> bool {
        self.legacy_resume_site_spans.contains(&span)
    }

    fn legacy_resume_site_suspends_outward(&self, span: Span) -> bool {
        self.legacy_outward_resume_site_spans.contains(&span)
    }

    fn legacy_perform_site_info(&self, span: Span) -> Option<&PerformCallSiteInfo> {
        self.legacy_perform_sites.get(&span)
    }

    fn refactor_resume_call_info(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&RefactorResumeCallInfo> {
        self.refactor_resume_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    fn refactor_perform_metadata(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&PerformMetadata> {
        self.refactor_perform_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    fn refactor_handle_site_info(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&RefactorHandleSiteInfo> {
        self.refactor_handle_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    fn class_ctor_hidden_effects(&self, source_path: &std::path::Path, span: Span) -> EffectRow {
        self.class_ctor_hidden_effects
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
            .cloned()
            .unwrap_or_else(EffectRow::pure)
    }

    fn object_member_hidden_effects(&self, fqn: &str) -> EffectRow {
        self.object_member_hidden_effects
            .get(fqn)
            .cloned()
            .unwrap_or_else(EffectRow::pure)
    }

    fn top_level_ref_hidden_effects(&self, fqn: &str) -> EffectRow {
        self.top_level_ref_hidden_effects
            .get(fqn)
            .cloned()
            .unwrap_or_else(EffectRow::pure)
    }

    fn refactor_top_level_init_roots(&self) -> &[TopLevelInitRootContract] {
        &self.refactor_top_level_init_roots
    }

    fn refactor_extern_global_contracts(&self) -> &[ExternGlobalContract] {
        &self.refactor_extern_global_contracts
    }

    fn when_pat_binding_ty(&self, span: Span) -> Option<TypeId> {
        self.when_pat_binding_tys.get(&span).copied()
    }
}

struct HiddenInitEffectAnalyzer<'a> {
    lowered: &'a hir::LoweredHir,
}

impl<'a> HiddenInitEffectAnalyzer<'a> {
    fn new(lowered: &'a hir::LoweredHir) -> Self {
        Self { lowered }
    }

    fn class_ctor_effect_row(&self, class_fqn: &str, ctor_span: Option<Span>) -> EffectRow {
        let mut visiting = HashSet::new();
        EffectRow::new(self.class_ctor_effect_terms(class_fqn, ctor_span, &mut visiting))
    }

    fn object_init_effect_row(&self, object_fqn: &str) -> EffectRow {
        let mut visiting = HashSet::new();
        EffectRow::new(self.object_init_effect_terms(object_fqn, &mut visiting))
    }

    fn top_level_immutable_value_effect_row(&self, value_fqn: &str) -> EffectRow {
        let mut visiting = HashSet::new();
        EffectRow::new(self.top_level_immutable_value_effect_terms(value_fqn, &mut visiting))
    }

    fn class_ctor_effect_terms(
        &self,
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
        terms.extend(self.scan_call_args(
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
                    terms.extend(self.scan_expr(
                        default_value,
                        class.source_path.as_path(),
                        visiting,
                    ));
                }
            }
            if let Some(delegation) = ctor.delegation.as_ref() {
                terms.extend(self.scan_call_args(
                    &delegation.args,
                    class.source_path.as_path(),
                    visiting,
                ));
            }
        }

        for step in &class.steps {
            match step {
                hir::ClassInitStep::PropertyInit { init, .. } => {
                    terms.extend(self.scan_expr(init, class.source_path.as_path(), visiting));
                }
                hir::ClassInitStep::InitBlock { block } => {
                    terms.extend(self.scan_block(block, class.source_path.as_path(), visiting));
                }
            }
        }
        if let Some(ctor) = selected_ctor
            && let Some(body) = ctor.body.as_ref()
        {
            terms.extend(self.scan_block(body, class.source_path.as_path(), visiting));
        }

        visiting.remove(&key);
        terms
    }

    fn object_init_effect_terms(
        &self,
        object_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(object_init) = self.lowered.object_inits.get(object_fqn) else {
            return Vec::new();
        };
        let key = format!("object:{object_fqn}");
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let mut terms = Vec::new();
        for step in &object_init.steps {
            match step {
                hir::ObjectInitStep::PropertyInit { init, .. } => {
                    terms.extend(self.scan_expr(init, object_init.source_path.as_path(), visiting));
                }
                hir::ObjectInitStep::InitBlock { block } => {
                    terms.extend(self.scan_block(
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
        &self,
        value_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let Some(value) = self.lowered.top_level_immutable_values.get(value_fqn) else {
            return Vec::new();
        };
        let key = format!("top-level-val:{value_fqn}");
        if !visiting.insert(key.clone()) {
            return Vec::new();
        }

        let terms = value
            .init
            .as_ref()
            .map(|init| self.scan_expr(init, value.source_path.as_path(), visiting))
            .unwrap_or_default();
        visiting.remove(&key);
        terms
    }

    fn lookup_class_init(&self, class_fqn: &str) -> Option<&'a hir::ClassInit> {
        self.lowered.class_inits.get(class_fqn).or_else(|| {
            self.lowered
                .class_inits
                .values()
                .find(|class| class.fqn == class_fqn)
        })
    }

    fn scan_block(
        &self,
        block: &hir::Block,
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let mut terms = Vec::new();
        for stmt in &block.stmts {
            terms.extend(self.scan_stmt(stmt, source_path, visiting));
        }
        terms
    }

    fn scan_stmt(
        &self,
        stmt: &hir::Stmt,
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => self.scan_expr(expr, source_path, visiting),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .map(|expr| self.scan_expr(expr, source_path, visiting))
                .unwrap_or_default(),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                let mut terms = self.scan_expr(lhs, source_path, visiting);
                terms.extend(self.scan_expr(rhs, source_path, visiting));
                terms
            }
            hir::StmtKind::While { cond, body } => {
                let mut terms = self.scan_expr(cond, source_path, visiting);
                terms.extend(self.scan_block(body, source_path, visiting));
                terms
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .map(|expr| self.scan_expr(expr, source_path, visiting))
                .unwrap_or_default(),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => Vec::new(),
        }
    }

    fn scan_expr(
        &self,
        expr: &hir::Expr,
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => Vec::new(),
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let mut terms = self.object_init_effect_terms(fqn, visiting);
                terms.extend(self.top_level_immutable_value_effect_terms(fqn, visiting));
                terms
            }
            hir::ExprKind::VarRef(_) => Vec::new(),
            hir::ExprKind::Block(block) => self.scan_block(block, source_path, visiting),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. } => self.scan_expr(expr, source_path, visiting),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                let mut terms = self.scan_expr(lhs, source_path, visiting);
                terms.extend(self.scan_expr(rhs, source_path, visiting));
                terms
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut terms = self.scan_expr(cond, source_path, visiting);
                terms.extend(self.scan_expr(then_branch, source_path, visiting));
                if let Some(else_branch) = else_branch.as_deref() {
                    terms.extend(self.scan_expr(else_branch, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::When { subject, arms } => {
                let mut terms = self.scan_expr(subject, source_path, visiting);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        terms.extend(self.scan_expr(guard, source_path, visiting));
                    }
                    terms.extend(self.scan_expr(&arm.body, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let mut terms = self.scan_expr(receiver, source_path, visiting);
                if let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref()
                    && let Some((owner_fqn, _)) = fqn.rsplit_once('.')
                {
                    terms.extend(self.object_init_effect_terms(owner_fqn, visiting));
                }
                terms
            }
            hir::ExprKind::StructLit { fields, .. } => {
                let mut terms = Vec::new();
                for field in fields {
                    terms.extend(self.scan_expr(&field.value, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::TupleLit { elements } => {
                let mut terms = Vec::new();
                for element in elements {
                    terms.extend(self.scan_expr(element, source_path, visiting));
                }
                terms
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                let mut terms = Vec::new();
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        terms.extend(self.scan_expr(expr, source_path, visiting));
                    }
                }
                terms
            }
            hir::ExprKind::Call { callee, args } => {
                let mut terms = self.scan_expr(callee, source_path, visiting);
                terms.extend(self.scan_call_args(args, source_path, visiting));
                if let Some(info) = self
                    .lowered
                    .ctor_call_sites
                    .get(&hir::CallSite::new(source_path.to_path_buf(), expr.span))
                {
                    terms.extend(self.class_ctor_effect_terms(
                        &info.class_fqn,
                        info.ctor_span,
                        visiting,
                    ));
                } else if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) =
                    self.lowered.types.kind(callee.ty)
                {
                    terms.extend(fun_ty.effects.terms.iter().copied());
                }
                terms
            }
            hir::ExprKind::Perform {
                effect_ty, args, ..
            } => {
                let mut terms = self.scan_call_args(args, source_path, visiting);
                terms.push(*effect_ty);
                terms
            }
            hir::ExprKind::Handle(handle) => {
                let mut terms = self.scan_block(&handle.body, source_path, visiting);
                for arm in &handle.arms {
                    terms.extend(self.scan_expr(&arm.body, source_path, visiting));
                }
                if let Some(finally) = handle.finally.as_ref() {
                    terms.extend(self.scan_block(finally, source_path, visiting));
                }
                terms
            }
        }
    }

    fn scan_call_args(
        &self,
        args: &[hir::CallArg],
        source_path: &std::path::Path,
        visiting: &mut HashSet<String>,
    ) -> Vec<TypeId> {
        let mut terms = Vec::new();
        for arg in args {
            match arg {
                hir::CallArg::Positional(expr) => {
                    terms.extend(self.scan_expr(expr, source_path, visiting));
                }
                hir::CallArg::Named { value, .. } => {
                    terms.extend(self.scan_expr(value, source_path, visiting));
                }
            }
        }
        terms
    }
}

/// MIR lowering 错误（当前阶段仅包装 HIR lowering 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum MirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Hir(#[from] hir::HirLowerError),
    #[error("refactor direct-style MIR validation failed for `{fqn}`: {error}")]
    InvalidRefactorMir {
        fqn: String,
        #[source]
        error: MirValidationError,
    },
}

/// 一次 lowering 的产物：MIR + 对应的 `TypeStore`。
///
/// 说明：MIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
#[derive(Debug)]
pub struct LoweredMir {
    pub file: File,
    pub types: TypeStore,
}

/// 新建 basic block 时使用的默认 terminator 标记。
///
/// 说明：builder 在 block 完成后应当覆盖该 terminator；若最终仍保留该值，说明 lowering 未覆盖到
/// 某条控制流路径（对 dump/fixtures 来说仍可接受，但在后续阶段应当更严格约束）。
const UNTERMINATED: &str = "unterminated";
/// `var` 可变捕获在 MIR dump 阶段使用的内部 box 类型名（T0714）。
const CAPTURE_BOX_FQN: &str = "scoop.__CaptureBox";
const ARRAY_BUILDER_NEW_FQN: &str = "scoop.core.__scoop_array_builder_new";
const ARRAY_BUILDER_PUSH_FQN: &str = "scoop.core.__scoop_array_builder_push";
const ARRAY_BUILDER_PUSH_STRING_FQN: &str = "scoop.core.__scoop_array_builder_push_string";
const ARRAY_BUILDER_BUILD_ARRAY_FQN: &str = "scoop.core.__scoop_array_builder_build_array";
const ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN: &str =
    "scoop.core.__scoop_array_builder_build_mutable_array";
const ARRAY_BUILDER_BUILD_ARRAY_STRING_FQN: &str =
    "scoop.core.__scoop_array_builder_build_array_string";
const THREAD_SPAWN_JOIN_RESUME_FQN: &str = "scoop.core.__scoop_thread_spawn_join_resume";
const THREAD_SPAWN_JOIN_RESUME_U64_FQN: &str = "scoop.core.__scoop_thread_spawn_join_resume_u64";

fn intrinsic_base_fqn(fqn: &str) -> &str {
    let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn top_level_callee_fqn(callee: &hir::Expr) -> Option<&str> {
    match &callee.kind {
        hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
        _ => None,
    }
}

fn top_level_binding_matches_callee(
    binding: &ast::TopLevelFunCallBinding,
    callee: &hir::Expr,
) -> bool {
    top_level_callee_fqn(callee)
        .is_none_or(|callee_fqn| intrinsic_base_fqn(&binding.fqn) == intrinsic_base_fqn(callee_fqn))
}

/// 为 `scoop dump-mir` / mir fixtures 生成 MIR（最小实现）。
///
/// 当前阶段 pipeline：
/// 1) parse/resolve 源文件并降到 HIR（复用 `hir::lower_for_dump`）；
/// 2) 把 HIR 再降到 MIR（本文件实现），并生成显式 CFG。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredMir, MirLowerError> {
    let mut lowered_hir = hir::lower_typed_for_dump(session, source)?;
    let builtins = lowered_hir.types.intern_builtins();
    let facts = MirLoweringFacts::from_lowered_hir(&lowered_hir);

    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    Ok(LoweredMir {
        file,
        types: lowered_hir.types,
    })
}

/// 将一份已构造的 HIR 文件降低为 MIR，并显式接入 typed/shared facts。
///
/// 说明：
/// - 调用方需要确保 `hir_file` 中的 `TypeId` 与 `types` 来自同一个 `TypeStore`；
/// - `facts` 负责把 `Continuation.resume`、virtual/interface dispatch 等已确认语义
///   从 HIR/typecheck side table 收口为 MIR lowering 可直接消费的最小输入。
pub(crate) fn lower_hir_file_for_dump_with_facts(
    builtins: BuiltinTypes,
    types: &mut TypeStore,
    hir_file: &hir::File,
    member_funs: &[hir::FunDecl],
    facts: &MirLoweringFacts,
) -> File {
    let mut lowering = MirLowering::new(builtins, types, facts);
    lowering.lower_file(hir_file, member_funs)
}

/// 文件级 lowering：负责遍历顶层 item 并为每个函数构造 MIR body。
struct MirLowering<'a> {
    builtins: BuiltinTypes,
    types: &'a mut TypeStore,
    facts: &'a MirLoweringFacts,
}

impl<'a> MirLowering<'a> {
    /// 创建一个 MIR lowering 上下文（仅保存 builtin type ids）。
    fn new(builtins: BuiltinTypes, types: &'a mut TypeStore, facts: &'a MirLoweringFacts) -> Self {
        Self {
            builtins,
            types,
            facts,
        }
    }

    /// 把 HIR 文件降到 MIR 文件。
    fn lower_file(&mut self, file: &hir::File, member_funs: &[hir::FunDecl]) -> File {
        let top_level_fun_return_tys = collect_top_level_fun_return_tys(file, member_funs);
        let top_level_fun_param_tys = collect_top_level_fun_param_tys(file, member_funs);
        let mut items = Vec::with_capacity(file.items.len() + member_funs.len());
        if self.facts.uses_refactor_typed_contracts() {
            items.extend(
                file.decls
                    .iter()
                    .map(|decl| Item::Metadata(lower_decl_metadata(decl))),
            );
            items.extend(
                self.facts
                    .refactor_top_level_init_roots()
                    .iter()
                    .map(|root| Item::InitializerRoot(self.lower_initializer_root(root))),
            );
            items.extend(
                self.facts
                    .refactor_extern_global_contracts()
                    .iter()
                    .map(|contract| Item::ExternGlobal(lower_extern_global_root(contract))),
            );
        }
        for item in &file.items {
            match item {
                hir::Item::Fun(fun) => {
                    let (primary, nested) =
                        self.lower_fun(fun, &top_level_fun_return_tys, &top_level_fun_param_tys);
                    items.push(Item::Fun(primary));
                    items.extend(nested.into_iter().map(Item::Fun));
                }
                hir::Item::Val(_) if self.facts.uses_refactor_typed_contracts() => {}
                hir::Item::Val(decl) => items.push(Item::Todo {
                    span: decl.span,
                    kind: "top-level val",
                }),
                hir::Item::Todo { span, kind } => items.push(Item::Todo { span: *span, kind }),
            }
        }

        // type/object body 中可 codegen 的 member fun 在 HIR 中以 side table 形式保存；
        // dump-mir / dump-ir 需要把它们也作为真正的 generic MIR root 发射出来。
        for fun in member_funs {
            let (primary, nested) =
                self.lower_fun(fun, &top_level_fun_return_tys, &top_level_fun_param_tys);
            items.push(Item::Fun(primary));
            items.extend(nested.into_iter().map(Item::Fun));
        }

        File { items }
    }

    fn lower_initializer_root(&self, root: &TopLevelInitRootContract) -> InitializerRoot {
        InitializerRoot {
            span: root.span(),
            fqn: root.fqn().to_string(),
            source_path: root.source_path().to_path_buf(),
            kind: lower_initializer_root_kind(root.kind()),
            ty: root.ty(),
            initializer_transport: root.initializer_ty().and_then(|source_ty| {
                root.ty().and_then(|target_ty| {
                    value_erasure_transport(
                        self.builtins,
                        self.types,
                        self.facts,
                        source_ty,
                        target_ty,
                    )
                })
            }),
            has_initializer: root.has_initializer(),
            dependencies: root
                .dependencies()
                .iter()
                .map(lower_initializer_dependency)
                .collect(),
            hidden_effects: self.facts.top_level_ref_hidden_effects(root.fqn()),
        }
    }

    /// 把一个函数降到 MIR。
    fn lower_fun(
        &mut self,
        fun: &hir::FunDecl,
        top_level_fun_return_tys: &HashMap<String, TypeId>,
        top_level_fun_param_tys: &HashMap<String, Vec<TypeId>>,
    ) -> (FunDecl, Vec<FunDecl>) {
        FnLowering::new(
            self.builtins,
            self.types,
            self.facts,
            top_level_fun_return_tys.clone(),
            top_level_fun_param_tys.clone(),
            fun.fqn.clone(),
            fun.source_path.clone(),
        )
        .lower_fun(fun)
    }
}

fn collect_top_level_fun_return_tys(
    file: &hir::File,
    member_funs: &[hir::FunDecl],
) -> HashMap<String, TypeId> {
    let mut return_tys = HashMap::new();
    for item in &file.items {
        if let hir::Item::Fun(fun) = item {
            return_tys.insert(fun.fqn.clone(), fun.return_ty);
        }
    }
    for fun in member_funs {
        return_tys.insert(fun.fqn.clone(), fun.return_ty);
    }
    return_tys
}

fn collect_top_level_fun_param_tys(
    file: &hir::File,
    member_funs: &[hir::FunDecl],
) -> HashMap<String, Vec<TypeId>> {
    let mut param_tys = HashMap::new();
    for item in &file.items {
        if let hir::Item::Fun(fun) = item {
            param_tys.insert(
                fun.fqn.clone(),
                fun.params.iter().map(|param| param.ty).collect(),
            );
        }
    }
    for fun in member_funs {
        param_tys.insert(
            fun.fqn.clone(),
            fun.params.iter().map(|param| param.ty).collect(),
        );
    }
    param_tys
}

fn mir_transport_kind_for_ty(
    types: &TypeStore,
    facts: &MirLoweringFacts,
    ty: TypeId,
) -> MirTransportKind {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => MirTransportKind::FunctionValue,
        TypeKind::Ref(_) => MirTransportKind::Reference,
        TypeKind::Value(ValueTypeKind::Tuple(_)) => MirTransportKind::Tuple,
        TypeKind::Value(ValueTypeKind::Option(_)) => MirTransportKind::EnumPayload,
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if facts.nominal_kind(&nominal.fqn) == Some(ast::TypeKind::Enum) =>
        {
            MirTransportKind::EnumPayload
        }
        TypeKind::Value(ValueTypeKind::Nominal(_)) => MirTransportKind::Struct,
        TypeKind::Value(_) => MirTransportKind::Scalar,
        TypeKind::Param(_) | TypeKind::StarProjection(_) => MirTransportKind::Unknown,
    }
}

fn mir_type_requires_trace(types: &TypeStore, ty: TypeId) -> bool {
    match types.kind(ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection(_) => true,
        TypeKind::Value(ValueTypeKind::Option(inner)) => mir_type_requires_trace(types, *inner),
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements
            .iter()
            .any(|ty| mir_type_requires_trace(types, *ty)),
        // Nominal value fields are not in `TypeKind`; keep the contract conservative and
        // force later layout to query declaration metadata instead of guessing scalar shape.
        TypeKind::Value(ValueTypeKind::Nominal(_)) => true,
        TypeKind::Value(
            ValueTypeKind::Unit
            | ValueTypeKind::Nothing
            | ValueTypeKind::Bool
            | ValueTypeKind::Char
            | ValueTypeKind::Float64
            | ValueTypeKind::Float32
            | ValueTypeKind::Int
            | ValueTypeKind::UInt
            | ValueTypeKind::IntN(_)
            | ValueTypeKind::UIntN(_),
        ) => false,
    }
}

pub(crate) fn mir_is_aggregate_transport_ty(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Value(
            ValueTypeKind::Tuple(_) | ValueTypeKind::Nominal(_) | ValueTypeKind::Option(_)
        )
    )
}

pub(crate) fn mir_transport_requirements(
    types: &TypeStore,
    ty: TypeId,
) -> MirTransportRequirements {
    let trace = mir_type_requires_trace(types, ty);
    MirTransportRequirements {
        trace,
        copy: true,
        drop: trace || mir_is_aggregate_transport_ty(types, ty),
    }
}

fn erasure_boxing_reason(
    builtins: BuiltinTypes,
    types: &TypeStore,
    source_ty: TypeId,
    target_ty: TypeId,
) -> Option<MirBoxingReason> {
    if source_ty == target_ty || !matches!(types.kind(source_ty), TypeKind::Value(_)) {
        return None;
    }
    if target_ty == builtins.any {
        return Some(MirBoxingReason::AnyErasure);
    }
    match types.kind(target_ty) {
        TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection(_) => {
            Some(MirBoxingReason::RefErasure)
        }
        TypeKind::Value(_) => None,
    }
}

fn value_erasure_transport(
    builtins: BuiltinTypes,
    types: &TypeStore,
    facts: &MirLoweringFacts,
    source_ty: TypeId,
    target_ty: TypeId,
) -> Option<ValueTransportMetadata> {
    let reason = erasure_boxing_reason(builtins, types, source_ty, target_ty)?;
    Some(ValueTransportMetadata {
        source_ty,
        kind: mir_transport_kind_for_ty(types, facts, source_ty),
        requirements: mir_transport_requirements(types, source_ty),
        boxing: Some(MirBoxingIntent {
            source_ty,
            target_ty: Some(target_ty),
            reason,
        }),
    })
}

fn lower_initializer_root_kind(kind: TopLevelInitRootKind) -> InitializerRootKind {
    match kind {
        TopLevelInitRootKind::ConstVal => InitializerRootKind::ConstVal,
        TopLevelInitRootKind::RuntimeImmutableVal => InitializerRootKind::RuntimeImmutableVal,
        TopLevelInitRootKind::RuntimeMutableVar { storage } => {
            InitializerRootKind::RuntimeMutableVar { storage }
        }
        TopLevelInitRootKind::ObjectSingleton => InitializerRootKind::ObjectSingleton,
    }
}

fn lower_initializer_dependency(
    dependency: &crate::effect_refactor_pipeline::TopLevelInitDependency,
) -> InitializerDependency {
    InitializerDependency {
        fqn: dependency.fqn().to_string(),
        kind: match dependency.kind() {
            TopLevelInitDependencyKind::TopLevelValue => InitializerDependencyKind::TopLevelValue,
            TopLevelInitDependencyKind::ObjectSingleton => {
                InitializerDependencyKind::ObjectSingleton
            }
        },
    }
}

fn lower_extern_global_root(contract: &ExternGlobalContract) -> ExternGlobalRoot {
    ExternGlobalRoot {
        span: contract.span(),
        fqn: contract.fqn().to_string(),
        source_path: contract.source_path().to_path_buf(),
        ty: contract.ty(),
        mutable: contract.mutable(),
        symbol: contract.symbol().to_string(),
        linkage: contract.linkage(),
        storage: contract.storage(),
        initializer_absent: contract.initializer_absent(),
        unsafe_required: contract.unsafe_required(),
    }
}

fn lower_decl_metadata(decl: &hir::Decl) -> MetadataRoot {
    match decl {
        hir::Decl::TypeAlias(alias) => MetadataRoot::TypeAlias(TypeAliasMetadata {
            span: alias.span,
            fqn: alias.fqn.clone(),
            name: alias.name.clone(),
            type_params: alias
                .type_params
                .iter()
                .map(lower_decl_type_param_metadata)
                .collect(),
            ty: alias.ty,
        }),
        hir::Decl::Nominal(nominal) => MetadataRoot::Nominal(NominalMetadata {
            span: nominal.span,
            fqn: nominal.fqn.clone(),
            name: nominal.name.clone(),
            kind: nominal.kind,
            type_params: nominal
                .type_params
                .iter()
                .map(lower_decl_type_param_metadata)
                .collect(),
            supertypes: nominal
                .supertypes
                .iter()
                .map(lower_supertype_metadata)
                .collect(),
            interfaces: nominal.interfaces.clone(),
            constructors: nominal
                .constructors
                .iter()
                .map(lower_ctor_metadata)
                .collect(),
            members: nominal
                .members
                .iter()
                .map(lower_decl_member_metadata)
                .collect(),
        }),
        hir::Decl::Object(object) => MetadataRoot::Object(ObjectMetadata {
            span: object.span,
            fqn: object.fqn.clone(),
            name: object.name.clone(),
            kind: object.kind,
            supertypes: object
                .supertypes
                .iter()
                .map(lower_supertype_metadata)
                .collect(),
            interfaces: object.interfaces.clone(),
            initializer_root: object.initializer_root.clone(),
            members: object
                .members
                .iter()
                .map(lower_decl_member_metadata)
                .collect(),
        }),
        hir::Decl::ExtensionProperty(prop) => {
            MetadataRoot::ExtensionProperty(ExtensionPropertyMetadata {
                span: prop.span,
                fqn: prop.fqn.clone(),
                name: prop.name.clone(),
                type_params: prop
                    .type_params
                    .iter()
                    .map(lower_decl_type_param_metadata)
                    .collect(),
                mutable: prop.mutable,
                receiver_ty: prop.receiver_ty,
                ty: prop.ty,
                getter: prop.getter.as_ref().map(lower_accessor_metadata),
                setter: prop.setter.as_ref().map(lower_accessor_metadata),
            })
        }
    }
}

fn lower_decl_type_param_metadata(param: &hir::DeclTypeParam) -> DeclTypeParamMetadata {
    DeclTypeParamMetadata {
        span: param.span,
        name: param.name.clone(),
        variance: param.variance,
        ty: param.ty,
    }
}

fn lower_supertype_metadata(supertype: &hir::SupertypeDecl) -> SupertypeMetadata {
    SupertypeMetadata {
        span: supertype.span,
        fqn: supertype.fqn.clone(),
        ty: supertype.ty,
        ctor_arg_count: supertype.ctor_arg_count,
    }
}

fn lower_ctor_metadata(ctor: &hir::CtorDecl) -> CtorMetadata {
    CtorMetadata {
        span: ctor.span,
        kind: ctor.kind,
        params: ctor.params.iter().map(lower_ctor_param_metadata).collect(),
        delegation: ctor.delegation,
    }
}

fn lower_ctor_param_metadata(param: &hir::CtorParamDecl) -> CtorParamMetadata {
    CtorParamMetadata {
        span: param.span,
        name: param.name.clone(),
        ty: param.ty,
        has_default: param.has_default,
        property: param.property,
    }
}

fn lower_decl_member_metadata(member: &hir::DeclMember) -> DeclMemberMetadata {
    match member {
        hir::DeclMember::Field(field) => DeclMemberMetadata::Field(lower_field_metadata(field)),
        hir::DeclMember::Property(prop) => DeclMemberMetadata::Property(PropertyMetadata {
            span: prop.span,
            fqn: prop.fqn.clone(),
            name: prop.name.clone(),
            mutable: prop.mutable,
            ty: prop.ty,
            has_backing_field: prop.has_backing_field,
            getter: prop.getter.as_ref().map(lower_accessor_metadata),
            setter: prop.setter.as_ref().map(lower_accessor_metadata),
        }),
        hir::DeclMember::Fun(fun) => DeclMemberMetadata::Fun(MemberFunMetadata {
            span: fun.span,
            fqn: fun.fqn.clone(),
            name: fun.name.clone(),
            type_params: fun
                .type_params
                .iter()
                .map(lower_decl_type_param_metadata)
                .collect(),
            params: fun.params.iter().map(lower_ctor_param_metadata).collect(),
            return_ty: fun.return_ty,
        }),
        hir::DeclMember::EnumVariant(variant) => {
            DeclMemberMetadata::EnumVariant(EnumVariantMetadata {
                span: variant.span,
                fqn: variant.fqn.clone(),
                name: variant.name.clone(),
                fields: variant.fields.iter().map(lower_field_metadata).collect(),
            })
        }
        hir::DeclMember::InitBlock { span } => DeclMemberMetadata::InitBlock { span: *span },
        hir::DeclMember::Nested(decl) => {
            DeclMemberMetadata::Nested(Box::new(lower_decl_metadata(decl)))
        }
    }
}

fn lower_field_metadata(field: &hir::FieldDecl) -> FieldMetadata {
    FieldMetadata {
        span: field.span,
        fqn: field.fqn.clone(),
        name: field.name.clone(),
        mutable: field.mutable,
        ty: field.ty,
        origin: field.origin,
    }
}

fn lower_accessor_metadata(accessor: &hir::AccessorContract) -> AccessorMetadata {
    AccessorMetadata {
        span: accessor.span,
        fqn: accessor.fqn.clone(),
    }
}

/// 函数体 lowering：负责为单个函数构造 `Body`、管理 locals、并生成显式 CFG。
#[derive(Debug)]
struct FnLowering<'a> {
    builtins: BuiltinTypes,
    types: &'a mut TypeStore,
    facts: &'a MirLoweringFacts,
    top_level_fun_return_tys: HashMap<String, TypeId>,
    top_level_fun_param_tys: HashMap<String, Vec<TypeId>>,
    owner_fqn: String,
    source_path: std::path::PathBuf,
    current_return_ty: TypeId,
    body: Body,
    current_bb: BasicBlockId,
    next_temp: u32,
    next_site_id: u32,
    symbol_locals: HashMap<hir::SymbolId, LocalId>,
    /// 值 local 的最小 provenance。
    ///
    /// 当前阶段主要为 call / member / unresolved callee / pattern canonicalization 保留最小来源信息；
    /// 一旦出现多路径/多来源冲突，就保守退化为 `UnknownCallable`。
    value_origins: HashMap<LocalId, ValueOrigin>,
    /// 当前函数内哪些 `SymbolId` 以 box 形式存储（用于 `var` 被 closure 捕获时的别名语义，T0714）。
    boxed_symbols: HashSet<hir::SymbolId>,
    cleanup_scopes: Vec<CleanupScope>,
    loop_stack: Vec<LoopContext>,
    nested_funs: Vec<FunDecl>,
}

/// 当前函数内的一个 loop 语境（用于 `break/continue` lowering）。
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    break_target: BasicBlockId,
    continue_target: BasicBlockId,
    cleanup_depth: usize,
}

/// 当前 lowering 语境里“离开该作用域前必须执行”的 finally/cleanup 块。
#[derive(Debug, Clone)]
struct CleanupScope {
    finally: hir::Block,
}

/// 一个 local 当前可观察到的最小 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ValueOrigin {
    Closure { fn_ptr: String },
    TopLevelRef { fqn: String },
    MemberAccess { member: MemberAccessMetadata },
    UnresolvedName { name: String },
    UnknownCallable,
}

fn gc_intrinsic_callee_from_origin(origin: Option<&ValueOrigin>) -> Option<&str> {
    let Some(ValueOrigin::MemberAccess {
        member:
            MemberAccessMetadata {
                resolved: Some(MemberTarget::Fun { fqn } | MemberTarget::ExtensionFun { fqn }),
                ..
            },
    }) = origin
    else {
        return None;
    };
    matches!(
        fqn.as_str(),
        "scoop.core.GC.pin"
            | "scoop.core.GC.unpin"
            | "scoop.core.GC.handleNew"
            | "scoop.core.GC.handleGet"
            | "scoop.core.GC.handleDrop"
    )
    .then_some(fqn.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoredContinuationRouteError {
    MissingSourceLocal,
    Ambiguous,
}

/// 一个 closure 捕获的外部局部变量在 env tuple 中的布局信息（T0711）。
#[derive(Debug, Clone)]
struct ClosureCaptureLayout {
    id: hir::SymbolId,
    name: String,
    decl_span: Span,
    ty: TypeId,
    mutable: bool,
    /// 在“创建该 closure 的函数”中，对应被捕获值的 local。
    source_local: LocalId,
}

#[derive(Debug, Clone)]
struct WhenPatternBinding {
    id: hir::SymbolId,
    span: Span,
    name: String,
    ty: TypeId,
    path: Vec<PatternBindingStep>,
}

impl<'a> FnLowering<'a> {
    /// 创建一个新的函数 lowering builder。
    fn new(
        builtins: BuiltinTypes,
        types: &'a mut TypeStore,
        facts: &'a MirLoweringFacts,
        top_level_fun_return_tys: HashMap<String, TypeId>,
        top_level_fun_param_tys: HashMap<String, Vec<TypeId>>,
        owner_fqn: String,
        source_path: std::path::PathBuf,
    ) -> Self {
        Self {
            builtins,
            types,
            facts,
            top_level_fun_return_tys,
            top_level_fun_param_tys,
            owner_fqn,
            source_path,
            current_return_ty: builtins.unit,
            body: Body::new_empty(),
            current_bb: BasicBlockId(0),
            next_temp: 0,
            next_site_id: 0,
            symbol_locals: HashMap::new(),
            value_origins: HashMap::new(),
            boxed_symbols: HashSet::new(),
            cleanup_scopes: Vec::new(),
            loop_stack: Vec::new(),
            nested_funs: Vec::new(),
        }
    }

    /// 把一个 HIR 函数声明降到 MIR（当前阶段仅关注 body 的 CFG 形态）。
    fn lower_fun(mut self, fun: &hir::FunDecl) -> (FunDecl, Vec<FunDecl>) {
        self.current_return_ty = fun.return_ty;
        // 1) 创建入口块。
        let entry = self.push_block(fun.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) 参数变为 locals，并建立 SymbolId → LocalId 映射。
        let mut params = Vec::with_capacity(fun.params.len());
        for p in &fun.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower 函数体。
        let mir_body = if let Some(block) = fun.body.as_ref() {
            // 先扫描函数体：若某个 `var` 被任意深度的嵌套 closure 捕获，则该 `var` 在本函数内需要 box 存储。
            self.boxed_symbols = boxed_symbols_in_block(block);
            if fun.return_ty == self.builtins.unit {
                self.lower_block_as_stmt(block);
                self.finish_function(fun.span);
            } else {
                let body_result = self.lower_block_as_expr(block);
                if !self.current_is_terminated() {
                    let value =
                        self.operand_for_current_return_ty(fun.span, Operand::Local(body_result));
                    self.set_terminator(
                        self.current_bb,
                        fun.span,
                        TerminatorKind::Return { value: Some(value) },
                    );
                }
            }
            self.assign_deferred_class_ctor_site_ids();
            Some(std::mem::replace(&mut self.body, Body::new_empty()))
        } else {
            None
        };

        let out = FunDecl {
            span: fun.span,
            fqn: fun.fqn.clone(),
            name: fun.name.clone(),
            ty: fun.ty,
            params,
            return_ty: fun.return_ty,
            body: mir_body,
        };

        (out, self.nested_funs)
    }

    /// 创建一个新的 basic block，并返回其 id。
    fn push_block(&mut self, span: Span) -> BasicBlockId {
        self.body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span,
                kind: TerminatorKind::Todo(UNTERMINATED),
                unwind: UnwindAction::NoUnwind,
            },
        })
    }

    fn push_cleanup_block(&mut self, span: Span) -> BasicBlockId {
        let bb = self.push_block(span);
        self.body.blocks[bb.as_usize()].is_cleanup = true;
        bb
    }

    fn fresh_site_id(&mut self) -> SiteId {
        let site_id = SiteId::from_raw(self.next_site_id);
        self.next_site_id = self
            .next_site_id
            .checked_add(1)
            .expect("too many MIR site ids in one body");
        site_id
    }

    fn assign_deferred_class_ctor_site_ids(&mut self) {
        for block in &mut self.body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign {
                    value: Rvalue::ClassCtor { site_id, .. },
                    ..
                } = &mut stmt.kind
                else {
                    continue;
                };
                if site_id.as_u32() != u32::MAX {
                    continue;
                }
                *site_id = SiteId::from_raw(self.next_site_id);
                self.next_site_id = self
                    .next_site_id
                    .checked_add(1)
                    .expect("too many MIR site ids in one body");
            }
        }
    }

    /// 在当前 basic block 末尾追加一条语句。
    fn push_stmt(&mut self, span: Span, kind: StatementKind) {
        let bb = self.current_bb;
        self.body.blocks[bb.as_usize()]
            .stmts
            .push(Statement { span, kind });
    }

    /// 覆盖指定 basic block 的 terminator。
    fn set_terminator_with_unwind(
        &mut self,
        bb: BasicBlockId,
        span: Span,
        kind: TerminatorKind,
        unwind: UnwindAction,
    ) {
        self.body.blocks[bb.as_usize()].terminator = Terminator { span, kind, unwind };
    }

    /// 覆盖指定 basic block 的 terminator（默认 `NoUnwind`）。
    fn set_terminator(&mut self, bb: BasicBlockId, span: Span, kind: TerminatorKind) {
        self.set_terminator_with_unwind(bb, span, kind, UnwindAction::NoUnwind);
    }

    /// 当前 basic block 是否已经被 terminator 结束。
    fn current_is_terminated(&self) -> bool {
        let bb = self.current_bb;
        !matches!(
            self.body.blocks[bb.as_usize()].terminator.kind,
            TerminatorKind::Todo(msg) if msg == UNTERMINATED
        )
    }

    /// 当前 block 若只是被占位式 effect terminator 截断，则为后续语句分配一个新的 continuation block。
    ///
    /// 说明：
    /// - 现阶段 `TerminatorKind::Handle` / `TerminatorKind::Perform` 仍未展开成真实 CFG；
    /// - 但像 async task body 这类形状会在 `handle { ... }` 之后继续出现普通 direct call
    ///   （例如 `__task_step_ready(...)`），并且 `await` 之后的恢复路径也仍需要在 generic MIR 中保形；
    /// - 若这里直接停止，generic MIR materializer 将看不到这些后续 call-site；
    /// - 因此仅当终止原因是占位式 `Handle` / `Perform` 时，允许把后续语句接到一个新的孤立 block 中继续保形。
    fn continue_after_placeholder_effect_terminator_if_needed(&mut self, next_span: Span) -> bool {
        if self.facts.uses_refactor_typed_contracts() {
            return !self.current_is_terminated();
        }
        if !self.current_is_terminated() {
            return true;
        }
        if !matches!(
            self.body.blocks[self.current_bb.as_usize()].terminator.kind,
            TerminatorKind::Handle { .. } | TerminatorKind::Perform { .. }
        ) {
            return false;
        }
        self.current_bb = self.push_block(next_span);
        true
    }

    fn with_cleanup_scope_len<T>(&mut self, len: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let mut tail = self.cleanup_scopes.split_off(len);
        let result = f(self);
        self.cleanup_scopes.append(&mut tail);
        result
    }

    fn lower_cleanup_block_to_target(
        &mut self,
        cleanup_bb: BasicBlockId,
        cleanup: &hir::Block,
        target: BasicBlockId,
        outer_cleanup_len: usize,
    ) {
        let saved_bb = self.current_bb;
        self.current_bb = cleanup_bb;
        self.with_cleanup_scope_len(outer_cleanup_len, |this| {
            this.lower_block_as_stmt(cleanup);
        });
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                cleanup.span,
                TerminatorKind::Goto { target },
            );
        }
        self.current_bb = saved_bb;
    }

    fn build_cleanup_route(
        &mut self,
        target: BasicBlockId,
        min_cleanup_depth: usize,
    ) -> BasicBlockId {
        let mut next_target = target;
        for scope_index in (min_cleanup_depth..self.cleanup_scopes.len()).rev() {
            let cleanup = self.cleanup_scopes[scope_index].finally.clone();
            let cleanup_bb = self.push_cleanup_block(cleanup.span);
            self.lower_cleanup_block_to_target(cleanup_bb, &cleanup, next_target, scope_index);
            next_target = cleanup_bb;
        }
        next_target
    }

    fn build_perform_unwind_action(&mut self, span: Span) -> UnwindAction {
        let Some(scope) = self.cleanup_scopes.last().cloned() else {
            return UnwindAction::Propagate;
        };

        let resume_unwind_bb = self.push_cleanup_block(span);
        self.set_terminator(resume_unwind_bb, span, TerminatorKind::ResumeUnwind);

        let cleanup_bb = self.push_cleanup_block(scope.finally.span);
        self.lower_cleanup_block_to_target(
            cleanup_bb,
            &scope.finally,
            resume_unwind_bb,
            self.cleanup_scopes.len() - 1,
        );
        UnwindAction::Cleanup { target: cleanup_bb }
    }

    /// 若函数尾部没有显式 terminator，则默认补一个 `return`（保持 body 可验证/可 dump）。
    fn finish_function(&mut self, span: Span) {
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Return { value: None },
            );
        }
    }

    /// 分配一个具名 local（用于参数与 `val/var` 声明）。
    fn push_named_local(&mut self, span: Span, name: &str, ty: TypeId) -> LocalId {
        self.body.push_local(LocalDecl {
            span,
            name: Some(name.to_string()),
            ty,
            source: LocalSourceKind::SourceLocal,
        })
    }

    /// 分配一个临时 local（用于表达式求值与 if/when merge）。
    fn push_temp_local(&mut self, span: Span, ty: TypeId) -> LocalId {
        let name = format!("tmp{}", self.next_temp);
        self.next_temp += 1;
        self.body.push_local(LocalDecl {
            span,
            name: Some(name),
            ty,
            source: LocalSourceKind::CompilerTemporary,
        })
    }

    /// 生成 `target = value` 赋值语句。
    fn assign(&mut self, span: Span, target: LocalId, value: Rvalue) {
        self.record_value_origin(target, &value);
        self.push_stmt(span, StatementKind::Assign { target, value });
    }

    fn value_erasure_transport(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> Option<ValueTransportMetadata> {
        value_erasure_transport(self.builtins, self.types, self.facts, source_ty, target_ty)
    }

    fn transporting_use_rvalue(&self, value: Operand, target_ty: TypeId) -> Rvalue {
        let source_ty = self.operand_ty(&value);
        if let Some(transport) = self.value_erasure_transport(source_ty, target_ty) {
            Rvalue::Transport { value, transport }
        } else {
            Rvalue::Use(value)
        }
    }

    fn assign_use_to_local(&mut self, span: Span, target: LocalId, value: Operand) {
        let target_ty = self.body.locals[target.as_u32() as usize].ty;
        let rvalue = self.transporting_use_rvalue(value, target_ty);
        self.assign(span, target, rvalue);
    }

    fn operand_for_target_ty(&mut self, span: Span, value: Operand, target_ty: TypeId) -> Operand {
        let source_ty = self.operand_ty(&value);
        let Some(transport) = self.value_erasure_transport(source_ty, target_ty) else {
            return value;
        };
        let tmp = self.push_temp_local(span, target_ty);
        self.assign(span, tmp, Rvalue::Transport { value, transport });
        Operand::Local(tmp)
    }

    fn operand_for_current_return_ty(&mut self, span: Span, value: Operand) -> Operand {
        self.operand_for_target_ty(span, value, self.current_return_ty)
    }

    fn is_function_value_ty(&self, ty: TypeId) -> bool {
        matches!(self.types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
    }

    fn is_funptr_value_ty(&self, ty: TypeId) -> bool {
        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.unsafe.FunPtr" && nominal.args.len() == 1
        )
    }

    fn is_callable_value_ty(&self, ty: TypeId) -> bool {
        self.is_function_value_ty(ty) || self.is_funptr_value_ty(ty)
    }

    fn value_origin_from_operand(&self, operand: &Operand) -> Option<ValueOrigin> {
        match operand {
            Operand::Local(local) => self.value_origins.get(local).cloned(),
            Operand::Const(_) => None,
        }
    }

    fn classify_value_assignment(&self, target: LocalId, value: &Rvalue) -> Option<ValueOrigin> {
        let target_ty = self.body.locals[target.as_u32() as usize].ty;
        match value {
            Rvalue::MakeClosure { fn_ptr, .. } => Some(ValueOrigin::Closure {
                fn_ptr: fn_ptr.clone(),
            }),
            Rvalue::TopLevelRef(TopLevelRef { fqn, .. }) => {
                Some(ValueOrigin::TopLevelRef { fqn: fqn.clone() })
            }
            Rvalue::MemberAccess { member, .. } => Some(ValueOrigin::MemberAccess {
                member: member.clone(),
            }),
            Rvalue::UnresolvedName { name } => {
                Some(ValueOrigin::UnresolvedName { name: name.clone() })
            }
            Rvalue::Transport { value, .. } => self.value_origin_from_operand(value),
            Rvalue::Use(operand) => self.value_origin_from_operand(operand).or_else(|| {
                self.is_callable_value_ty(target_ty)
                    .then_some(ValueOrigin::UnknownCallable)
            }),
            _ => self
                .is_callable_value_ty(target_ty)
                .then_some(ValueOrigin::UnknownCallable),
        }
    }

    fn merge_value_origin(
        current: Option<ValueOrigin>,
        next: Option<ValueOrigin>,
    ) -> Option<ValueOrigin> {
        match (current, next) {
            (None, None) => None,
            (_, None) => None,
            (None, Some(origin)) => Some(origin),
            (Some(left), Some(right)) if left == right => Some(left),
            (Some(_), Some(_)) => Some(ValueOrigin::UnknownCallable),
        }
    }

    fn record_value_origin(&mut self, target: LocalId, value: &Rvalue) {
        let next = self.classify_value_assignment(target, value);
        let merged = Self::merge_value_origin(self.value_origins.get(&target).cloned(), next);
        match merged {
            Some(origin) => {
                self.value_origins.insert(target, origin);
            }
            None => {
                self.value_origins.remove(&target);
            }
        }
    }

    /// 把一个 block 作为“语句块”来 lower（顺序执行；最后表达式结果被丢弃）。
    fn lower_block_as_stmt(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            if !self.continue_after_placeholder_effect_terminator_if_needed(stmt.span) {
                break;
            }
            self.lower_stmt(stmt);
        }
    }

    /// 把一个 block 作为“表达式块”来 lower，并返回 block 的结果 local。
    fn lower_block_as_expr(&mut self, block: &hir::Block) -> LocalId {
        let mut result: Option<LocalId> = None;
        for (idx, stmt) in block.stmts.iter().enumerate() {
            if !self.continue_after_placeholder_effect_terminator_if_needed(stmt.span) {
                break;
            }
            let is_last = idx + 1 == block.stmts.len();
            match (&stmt.kind, is_last) {
                (hir::StmtKind::Expr(expr), true) => result = Some(self.lower_expr_to_local(expr)),
                _ => self.lower_stmt(stmt),
            }
        }

        if self.current_is_terminated() {
            // block 由于 `return/break/continue` 等提前终止：结果永远不会被使用。
            // 为保持接口一致，仍返回一个临时 local，但不额外发射赋值语句（避免“终止后又生成语句”）。
            return self.push_temp_local(block.span, block.ty);
        }

        result.unwrap_or_else(|| self.emit_unit(block.span))
    }

    /// 把一条 HIR 语句降到 MIR（当前阶段只覆盖必要子集；未覆盖节点以 `Todo` 占位）。
    fn lower_stmt(&mut self, stmt: &hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty => {}
            hir::StmtKind::Expr(expr) => {
                let _ = self.lower_expr_to_local(expr);
            }
            hir::StmtKind::Val(decl) => self.lower_val_decl(decl),
            hir::StmtKind::Assign { lhs, rhs, .. } => self.lower_assign_stmt(stmt.span, lhs, rhs),
            hir::StmtKind::While { cond, body } => self.lower_while_stmt(stmt.span, cond, body),
            hir::StmtKind::Break { .. } => self.lower_break_stmt(stmt.span),
            hir::StmtKind::Continue { .. } => self.lower_continue_stmt(stmt.span),
            hir::StmtKind::Return { value } => {
                let return_value = if let Some(expr) = value {
                    let result = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return;
                    }
                    Some(self.operand_for_current_return_ty(stmt.span, Operand::Local(result)))
                } else {
                    None
                };

                if self.cleanup_scopes.is_empty() {
                    self.set_terminator(
                        self.current_bb,
                        stmt.span,
                        TerminatorKind::Return {
                            value: return_value,
                        },
                    );
                    return;
                }

                let return_bb = self.push_block(stmt.span);
                self.set_terminator(
                    return_bb,
                    stmt.span,
                    TerminatorKind::Return {
                        value: return_value,
                    },
                );
                let cleanup_target = self.build_cleanup_route(return_bb, 0);
                self.set_terminator(
                    self.current_bb,
                    stmt.span,
                    TerminatorKind::Goto {
                        target: cleanup_target,
                    },
                );
            }
            hir::StmtKind::Todo(kind) => self.push_stmt(stmt.span, StatementKind::Todo(kind)),
        }
    }

    /// 降低一个 `while` 语句：构造 loop CFG，并为 `break/continue` 建立跳转目标。
    fn lower_while_stmt(&mut self, span: Span, cond: &hir::Expr, body: &hir::Block) {
        // CFG 形态（无 label）：
        //
        //   parent ──goto──▶ cond_bb ──condbr──▶ body_bb ──goto──▶ cond_bb
        //                 └───────────────▶ exit_bb
        //
        // `break`    → exit_bb
        // `continue` → cond_bb

        let parent = self.current_bb;
        let cond_bb = self.push_block(cond.span);
        let body_bb = self.push_block(body.span);
        let exit_bb = self.push_block(span);

        self.set_terminator(parent, span, TerminatorKind::Goto { target: cond_bb });

        // 1) condition：在 cond_bb 中求值条件，并用 CondBr 结束。
        self.current_bb = cond_bb;
        let cond_local = self.lower_expr_to_local(cond);
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::CondBr {
                    cond: Operand::Local(cond_local),
                    then_target: body_bb,
                    else_target: exit_bb,
                },
            );
        }

        // 2) body：在 loop context 下 lower body；若 body 自然结束则回跳 cond_bb。
        self.current_bb = body_bb;
        self.loop_stack.push(LoopContext {
            break_target: exit_bb,
            continue_target: cond_bb,
            cleanup_depth: self.cleanup_scopes.len(),
        });
        self.lower_block_as_stmt(body);
        let _ = self.loop_stack.pop();

        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                body.span,
                TerminatorKind::Goto { target: cond_bb },
            );
        }

        // 3) 后续语句继续在 exit_bb 生成。
        self.current_bb = exit_bb;
    }

    /// 降低 `break`：跳转到当前 loop 的 exit block。
    fn lower_break_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            panic!("typecheck must reject `break` outside loops before MIR lowering: {span:?}");
        };
        if self.cleanup_scopes.len() == ctx.cleanup_depth {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto {
                    target: ctx.break_target,
                },
            );
            return;
        }
        let cleanup_target = self.build_cleanup_route(ctx.break_target, ctx.cleanup_depth);
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: cleanup_target,
            },
        );
    }

    /// 降低 `continue`：跳转到当前 loop 的 cond block。
    fn lower_continue_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            panic!("typecheck must reject `continue` outside loops before MIR lowering: {span:?}");
        };
        if self.cleanup_scopes.len() == ctx.cleanup_depth {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto {
                    target: ctx.continue_target,
                },
            );
            return;
        }
        let cleanup_target = self.build_cleanup_route(ctx.continue_target, ctx.cleanup_depth);
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: cleanup_target,
            },
        );
    }

    /// 降低一个 `val/var` 声明：分配 local，并 lower initializer（若存在）。
    fn lower_val_decl(&mut self, decl: &hir::ValDecl) {
        let id = decl.id.unwrap_or_else(|| {
            panic!(
                "typed HIR local declaration must have a symbol id: {:?}",
                decl.span
            )
        });

        let name = decl.name.as_deref().unwrap_or("<anon>");
        // `var` 若被 closure 捕获，需要在本函数内以 box 形式存储，保证后续读写别名一致（T0714）。
        if decl.mutable && self.boxed_symbols.contains(&id) {
            let box_ty = self.capture_box_ty(decl.ty);
            let local = self.push_named_local(decl.span, name, box_ty);
            self.symbol_locals.insert(id, local);

            if let Some(init) = &decl.init {
                let value = self.lower_expr_to_local(init);
                if self.current_is_terminated() {
                    return;
                }
                self.assign(
                    decl.span,
                    local,
                    Rvalue::CaptureBoxNew {
                        value: Operand::Local(value),
                        contract: self.capture_box_contract(box_ty, decl.ty),
                    },
                );
            } else {
                panic!(
                    "typecheck must reject captured mutable locals without initializer before MIR lowering: {:?}",
                    decl.span
                );
            }
            return;
        }

        let local = self.push_named_local(decl.span, name, decl.ty);
        self.symbol_locals.insert(id, local);

        if let Some(init) = &decl.init {
            let value = self.lower_expr_to_local(init);
            if self.current_is_terminated() {
                return;
            }
            self.assign_use_to_local(decl.span, local, Operand::Local(value));
        }
    }

    /// 降低一个赋值语句。
    fn lower_assign_stmt(&mut self, span: Span, lhs: &hir::Expr, rhs: &hir::Expr) {
        if self.facts.uses_refactor_typed_contracts() {
            self.lower_assign_stmt_with_place_contract(span, lhs, rhs);
            return;
        }

        match &lhs.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                let Some(target) = self.symbol_locals.get(id).copied() else {
                    self.push_stmt(span, StatementKind::Todo("assign lhs missing local"));
                    return;
                };

                let value = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, self.builtins.unit);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxSet {
                            box_operand: Operand::Local(target),
                            value: Operand::Local(value),
                            contract: self.capture_box_contract(
                                self.body.locals[target.as_u32() as usize].ty,
                                self.body.locals[value.as_u32() as usize].ty,
                            ),
                        },
                    );
                } else {
                    self.assign_use_to_local(span, target, Operand::Local(value));
                }
            }
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let value_ty = self.body.locals[value_local.as_u32() as usize].ty;
                let value = self.operand_for_target_ty(span, Operand::Local(value_local), value_ty);
                self.push_stmt(
                    span,
                    StatementKind::StoreTopLevelVar {
                        fqn: fqn.clone(),
                        value,
                        value_ty,
                    },
                );
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let receiver_local = self.lower_expr_to_local(receiver);
                if self.current_is_terminated() {
                    return;
                }
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
                let value_ty = self.body.locals[value_local.as_u32() as usize].ty;
                let value = self.operand_for_target_ty(span, Operand::Local(value_local), value_ty);
                self.push_stmt(
                    span,
                    StatementKind::StoreMember {
                        receiver: Operand::Local(receiver_local),
                        member: self.lower_member_access_metadata(member, receiver_ty),
                        value,
                        value_ty,
                        continuation_route: self.extract_stored_continuation_route(rhs),
                    },
                );
            }
            _ => {
                self.push_stmt(span, StatementKind::Todo("assign lhs lowering pending"));
            }
        }
    }

    fn lower_assign_stmt_with_place_contract(
        &mut self,
        span: Span,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) {
        let Some(contract) = self
            .facts
            .refactor_assign_place_contract(self.source_path.as_path(), span)
            .cloned()
        else {
            panic!("typed HIR assignment must have a place contract before MIR lowering: {span:?}");
        };

        match &contract.kind {
            hir::AssignPlaceKind::Local { id, .. } => {
                let target = self.symbol_locals.get(id).copied().unwrap_or_else(|| {
                    panic!("assignment place contract references an unallocated local: {id:?}")
                });

                let value = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, self.builtins.unit);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxSet {
                            box_operand: Operand::Local(target),
                            value: Operand::Local(value),
                            contract: self.capture_box_contract(
                                self.body.locals[target.as_u32() as usize].ty,
                                self.body.locals[value.as_u32() as usize].ty,
                            ),
                        },
                    );
                } else {
                    self.assign_use_to_local(span, target, Operand::Local(value));
                }
            }
            hir::AssignPlaceKind::TopLevel { fqn, .. } => {
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let value = self.operand_for_target_ty(
                    span,
                    Operand::Local(value_local),
                    contract.value_ty,
                );
                self.push_stmt(
                    span,
                    StatementKind::StoreTopLevelVar {
                        fqn: fqn.clone(),
                        value,
                        value_ty: contract.value_ty,
                    },
                );
            }
            hir::AssignPlaceKind::Member {
                receiver_ty,
                member_name,
                resolved,
                ..
            } => {
                let hir::ExprKind::MemberAccess { receiver, .. } = &lhs.kind else {
                    panic!(
                        "member assignment place contract must match a member-access lhs: {span:?}"
                    );
                };
                let receiver_local = self.lower_expr_to_local(receiver);
                if self.current_is_terminated() {
                    return;
                }
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let value = self.operand_for_target_ty(
                    span,
                    Operand::Local(value_local),
                    contract.value_ty,
                );
                self.push_stmt(
                    span,
                    StatementKind::StoreMember {
                        receiver: Operand::Local(receiver_local),
                        member: self.assign_place_member_metadata(
                            member_name,
                            *receiver_ty,
                            resolved.as_ref(),
                        ),
                        value,
                        value_ty: contract.value_ty,
                        continuation_route: self.extract_stored_continuation_route(rhs),
                    },
                );
            }
        }
    }

    fn assign_place_member_metadata(
        &self,
        member_name: &str,
        receiver_ty: TypeId,
        resolved: Option<&hir::MemberRef>,
    ) -> MemberAccessMetadata {
        let resolved = resolved.map(|resolved| match resolved {
            hir::MemberRef::Value { fqn, .. } => MemberTarget::Value { fqn: fqn.clone() },
            hir::MemberRef::Fun { fqn, .. } => MemberTarget::Fun { fqn: fqn.clone() },
            hir::MemberRef::ExtensionValue { fqn, .. } => {
                MemberTarget::ExtensionValue { fqn: fqn.clone() }
            }
            hir::MemberRef::ExtensionFun { fqn, .. } => {
                MemberTarget::ExtensionFun { fqn: fqn.clone() }
            }
        });
        let hidden_effects = match &resolved {
            Some(MemberTarget::Value { fqn }) => self.facts.object_member_hidden_effects(fqn),
            _ => EffectRow::pure(),
        };
        MemberAccessMetadata {
            name: member_name.to_string(),
            receiver_ty,
            resolved,
            hidden_effects,
        }
    }

    fn extract_stored_continuation_route(
        &self,
        expr: &hir::Expr,
    ) -> StoredContinuationRoutePublication {
        match self.try_extract_stored_continuation_route(expr) {
            Ok(Some(route)) => StoredContinuationRoutePublication::Unique(route),
            Ok(None) => StoredContinuationRoutePublication::None,
            Err(StoredContinuationRouteError::Ambiguous) => {
                StoredContinuationRoutePublication::Ambiguous
            }
            Err(StoredContinuationRouteError::MissingSourceLocal) => {
                StoredContinuationRoutePublication::None
            }
        }
    }

    fn try_extract_stored_continuation_route(
        &self,
        expr: &hir::Expr,
    ) -> Result<Option<StoredContinuationValueRoute>, StoredContinuationRouteError> {
        if continuation_contract_from_type(self.types, expr.ty).is_some() {
            match &expr.kind {
                hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                    let Some(local) = self.symbol_locals.get(id).copied() else {
                        return Err(StoredContinuationRouteError::MissingSourceLocal);
                    };
                    let source_ty = self.body.locals[local.as_u32() as usize].ty;
                    return Ok(Some(StoredContinuationValueRoute {
                        source_local: local,
                        source_ty,
                        path: Vec::new(),
                    }));
                }
                hir::ExprKind::Call { args, .. } => {
                    if let Some(binding) = self
                        .facts
                        .top_level_fun_call_binding(self.source_path.as_path(), expr.span)
                        && let Some(param_index) =
                            self.facts.continuation_identity_return_param(&binding.fqn)
                        && let Some(arg) = args.get(param_index)
                    {
                        let arg_expr = match arg {
                            hir::CallArg::Positional(value) => value,
                            hir::CallArg::Named { value, .. } => value,
                        };
                        return self.try_extract_stored_continuation_route(arg_expr);
                    }
                    return Ok(None);
                }
                _ => return Ok(None),
            }
        }

        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            return Ok(None);
        };
        let hir::ExprKind::UnresolvedIdent { name } = &callee.kind else {
            return Ok(None);
        };

        let mut found: Option<(usize, StoredContinuationValueRoute)> = None;
        for (field_index, arg) in args.iter().enumerate() {
            let arg_expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let Some(mut route) = self.try_extract_stored_continuation_route(arg_expr)? else {
                continue;
            };
            if found.is_some() {
                return Err(StoredContinuationRouteError::Ambiguous);
            }
            route.path.insert(
                0,
                PatternBindingStep::VariantField {
                    variant: name.clone(),
                    field_index,
                },
            );
            found = Some((field_index, route));
        }

        Ok(found.map(|(_, route)| route))
    }

    /// 把一个 HIR 表达式降为“产生值的 local”，并返回该 local。
    ///
    /// 说明：当前阶段优先保证 CFG 形态正确，因此表达式求值本身常以 `Todo` 占位。
    fn lower_expr_to_local(&mut self, expr: &hir::Expr) -> LocalId {
        match &expr.kind {
            hir::ExprKind::Missing => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo("missing expr"));
                tmp
            }
            hir::ExprKind::UnresolvedIdent { name } => {
                self.lower_unresolved_ident(expr.span, expr.ty, name)
            }
            hir::ExprKind::Todo(kind) => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo(kind));
                tmp
            }
            hir::ExprKind::Literal(lit) => self.lower_literal(expr.span, expr.ty, lit),
            hir::ExprKind::ClassLiteral(class_lit) => {
                self.lower_class_literal_expr(expr.span, expr.ty, class_lit)
            }
            hir::ExprKind::VarRef(v) => self.lower_var_ref(expr.span, expr.ty, v),
            hir::ExprKind::StructLit { fields, .. } => {
                self.lower_struct_lit_expr(expr.span, expr.ty, fields)
            }
            hir::ExprKind::TupleLit { elements } => {
                self.lower_tuple_lit_expr(expr.span, expr.ty, elements)
            }
            hir::ExprKind::InterpolatedString { raw, parts } => {
                self.lower_interpolated_string_expr(expr.span, expr.ty, *raw, parts)
            }
            hir::ExprKind::Unary {
                op, expr: operand, ..
            } => self.lower_unary_expr(expr.span, expr.ty, *op, operand),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.lower_binary_expr(expr.span, expr.ty, lhs, *op, rhs)
            }
            hir::ExprKind::TypeCheck {
                expr: value,
                op,
                target_ty: test_ty,
                ..
            } => self.lower_type_check_expr(expr.span, expr.ty, value, *op, *test_ty),
            hir::ExprKind::Cast {
                expr: value,
                op,
                target_ty,
                ..
            } => self.lower_cast_expr(expr.span, expr.ty, value, *op, *target_ty),
            hir::ExprKind::Block(block) => self.lower_block_as_expr(block),
            hir::ExprKind::Closure(closure) => self.lower_closure_expr(expr.span, expr.ty, closure),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
            ),
            hir::ExprKind::When { subject, arms } => {
                self.lower_when_expr(expr.span, expr.ty, subject, arms)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(expr.span, expr.ty, receiver, member)
            }
            hir::ExprKind::Call { callee, args } => {
                self.lower_call_expr(expr.span, expr.ty, callee, args)
            }
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.lower_perform_expr(expr.span, expr.ty, *effect_ty, op, args),
            hir::ExprKind::Handle(handle) => self.lower_handle_expr(expr.span, expr.ty, handle),
        }
    }

    fn lower_unresolved_ident(&mut self, span: Span, ty: TypeId, name: &str) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::UnresolvedName {
                name: name.to_string(),
            },
        );
        tmp
    }

    /// 生成一个 `Unit` 值，并返回其 local。
    fn emit_unit(&mut self, span: Span) -> LocalId {
        let tmp = self.push_temp_local(span, self.builtins.unit);
        self.assign(span, tmp, Rvalue::Use(Operand::Const(ConstValue::Unit)));
        tmp
    }

    fn lower_tuple_lit_expr(&mut self, span: Span, ty: TypeId, elements: &[hir::Expr]) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let mut lowered = Vec::with_capacity(elements.len());
        let mut field_tys = Vec::with_capacity(elements.len());
        for element in elements {
            let local = self.lower_expr_to_local(element);
            if self.current_is_terminated() {
                return result;
            }
            field_tys.push((None, self.body.locals[local.as_u32() as usize].ty));
            lowered.push(Operand::Local(local));
        }
        self.assign(
            span,
            result,
            Rvalue::MakeTuple {
                elements: lowered,
                transport: self.aggregate_transport(ty, AggregateTransportKind::Tuple, field_tys),
            },
        );
        result
    }

    fn lower_struct_lit_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let mut lowered = Vec::with_capacity(fields.len());
        let mut field_tys = Vec::with_capacity(fields.len());
        for field in fields {
            let local = self.lower_expr_to_local(&field.value);
            if self.current_is_terminated() {
                return result;
            }
            field_tys.push((
                Some(field.name.clone()),
                self.body.locals[local.as_u32() as usize].ty,
            ));
            lowered.push(crate::mir::StructLitField {
                span: field.value.span,
                name: field.name.clone(),
                value: Operand::Local(local),
            });
        }
        self.assign(
            span,
            result,
            Rvalue::StructLit {
                fields: lowered,
                transport: self.aggregate_transport(ty, AggregateTransportKind::Struct, field_tys),
            },
        );
        result
    }

    fn lower_interpolated_string_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        raw: bool,
        parts: &[hir::InterpolatedStringPart],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let mut lowered = Vec::with_capacity(parts.len());
        for part in parts {
            match part {
                hir::InterpolatedStringPart::Text { span } => {
                    lowered.push(InterpolatedStringPart::Text { span: *span });
                }
                hir::InterpolatedStringPart::Expr { expr } => {
                    let local = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return result;
                    }
                    lowered.push(InterpolatedStringPart::Expr {
                        span: expr.span,
                        value: Operand::Local(local),
                        ty: expr.ty,
                    });
                }
            }
        }
        self.assign(
            span,
            result,
            Rvalue::InterpolatedString {
                raw,
                parts: lowered,
            },
        );
        result
    }

    fn lower_unary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        op: ast::UnaryOp,
        operand: &hir::Expr,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let operand_local = self.lower_expr_to_local(operand);
        if self.current_is_terminated() {
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::Unary {
                op,
                operand: Operand::Local(operand_local),
            },
        );
        result
    }

    fn lower_binary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> LocalId {
        let result_ty = self.binary_result_ty(ty, op);
        match op {
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.lower_short_circuit_binary_expr(span, result_ty, lhs, op, rhs)
            }
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if let Some(result) =
                    self.try_lower_compare_to_binary_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }

                let result = self.push_temp_local(span, result_ty);
                let lhs_local = self.lower_expr_to_local(lhs);
                if self.current_is_terminated() {
                    return result;
                }
                let rhs_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return result;
                }
                self.assign(
                    span,
                    result,
                    Rvalue::Binary {
                        lhs: Operand::Local(lhs_local),
                        op,
                        rhs: Operand::Local(rhs_local),
                    },
                );
                result
            }
            _ => {
                let result = self.push_temp_local(span, result_ty);
                let lhs_local = self.lower_expr_to_local(lhs);
                if self.current_is_terminated() {
                    return result;
                }
                let rhs_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return result;
                }
                self.assign(
                    span,
                    result,
                    Rvalue::Binary {
                        lhs: Operand::Local(lhs_local),
                        op,
                        rhs: Operand::Local(rhs_local),
                    },
                );
                result
            }
        }
    }

    fn binary_result_ty(&self, fallback_ty: TypeId, op: ast::BinaryOp) -> TypeId {
        match op {
            ast::BinaryOp::Lt
            | ast::BinaryOp::Le
            | ast::BinaryOp::Gt
            | ast::BinaryOp::Ge
            | ast::BinaryOp::Eq
            | ast::BinaryOp::Ne
            | ast::BinaryOp::LogAnd
            | ast::BinaryOp::LogOr => self.builtins.bool_,
            _ => fallback_ty,
        }
    }

    fn runtime_type_test_metadata(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> RuntimeTypeTestMetadata {
        RuntimeTypeTestMetadata {
            source_ty,
            target_ty,
            descriptor: self.runtime_type_descriptor_key(target_ty),
            static_fold: self.runtime_type_static_fold(source_ty, target_ty),
            parameterized: self.runtime_type_parameterized_match(target_ty),
        }
    }

    fn runtime_cast_metadata(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
        result_ty: TypeId,
        op: ast::CastOp,
    ) -> RuntimeCastMetadata {
        let test = self.runtime_type_test_metadata(source_ty, target_ty);
        let (failure, result) = match op {
            ast::CastOp::As => (
                RuntimeCastFailure::Raise {
                    effect_ty: find_raise_runtime_error_effect(self.types),
                    error_fqn: "scoop.core.RuntimeError.ClassCastFailed".to_string(),
                },
                RuntimeCastResult::Target { ty: target_ty },
            ),
            ast::CastOp::AsQ => (
                RuntimeCastFailure::ReturnNone,
                RuntimeCastResult::Option {
                    option_ty: result_ty,
                    some_ty: target_ty,
                },
            ),
        };

        RuntimeCastMetadata {
            test,
            failure,
            result,
        }
    }

    fn runtime_pattern_type_test_metadata(
        &self,
        subject_ty: TypeId,
        target_ty: TypeId,
    ) -> RuntimePatternTypeTestMetadata {
        let descriptor = self.runtime_type_descriptor_key(target_ty);
        let parameterized = self.runtime_type_parameterized_match(target_ty);
        let match_kind = self.runtime_pattern_match_kind(&descriptor, &parameterized);
        RuntimePatternTypeTestMetadata {
            subject_ty,
            target_ty,
            descriptor,
            match_kind,
            static_fold: self.runtime_type_static_fold(subject_ty, target_ty),
            parameterized,
        }
    }

    fn runtime_type_descriptor_key(&self, ty: TypeId) -> RuntimeTypeDescriptorKey {
        let kind = match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Any) => RuntimeTypeDescriptorKind::Any,
            TypeKind::Ref(RefTypeKind::String) => RuntimeTypeDescriptorKind::String,
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                RuntimeTypeDescriptorKind::Nominal {
                    fqn: nominal.fqn.clone(),
                    kind: self.facts.nominal_kind(&nominal.fqn),
                }
            }
            TypeKind::Ref(RefTypeKind::Function(_)) => RuntimeTypeDescriptorKind::Function,
            TypeKind::Ref(RefTypeKind::Union(_)) => RuntimeTypeDescriptorKind::Union,
            TypeKind::Value(ValueTypeKind::Option(_)) => RuntimeTypeDescriptorKind::Option,
            TypeKind::Value(ValueTypeKind::Tuple(_)) => RuntimeTypeDescriptorKind::Tuple,
            TypeKind::Value(_) => RuntimeTypeDescriptorKind::Value,
            TypeKind::Param(_) => RuntimeTypeDescriptorKind::TypeParam,
            TypeKind::StarProjection(_) => RuntimeTypeDescriptorKind::StarProjection,
        };

        RuntimeTypeDescriptorKey { ty, kind }
    }

    fn runtime_type_parameterized_match(&self, ty: TypeId) -> RuntimeTypeParameterizedMatch {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if nominal.args.is_empty() && nominal.eff.is_none() {
                    RuntimeTypeParameterizedMatch::None
                } else {
                    RuntimeTypeParameterizedMatch::Nominal {
                        type_args: nominal.args.clone(),
                        effect_arg: nominal.eff.clone(),
                    }
                }
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => RuntimeTypeParameterizedMatch::Function {
                receiver: fun.receiver,
                params: fun.params.clone(),
                return_ty: fun.return_ty,
                effects: fun.effects.clone(),
                effects_closed: fun.effects_closed,
            },
            TypeKind::Ref(RefTypeKind::Union(union)) => RuntimeTypeParameterizedMatch::Union {
                variants: union.variants.clone(),
            },
            TypeKind::Value(ValueTypeKind::Option(payload_ty)) => {
                RuntimeTypeParameterizedMatch::Option {
                    payload_ty: *payload_ty,
                }
            }
            TypeKind::Value(ValueTypeKind::Tuple(element_tys)) => {
                RuntimeTypeParameterizedMatch::Tuple {
                    element_tys: element_tys.clone(),
                }
            }
            TypeKind::StarProjection(star) => RuntimeTypeParameterizedMatch::StarProjection {
                read_ty: star.read_ty,
            },
            TypeKind::Ref(RefTypeKind::Any)
            | TypeKind::Ref(RefTypeKind::String)
            | TypeKind::Value(_)
            | TypeKind::Param(_) => RuntimeTypeParameterizedMatch::None,
        }
    }

    fn runtime_type_static_fold(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> RuntimeTypeStaticFold {
        if source_ty == target_ty {
            return RuntimeTypeStaticFold::AlwaysTrue;
        }
        if target_ty == self.builtins.any {
            return RuntimeTypeStaticFold::AlwaysTrue;
        }
        if target_ty == self.builtins.nothing {
            return RuntimeTypeStaticFold::AlwaysFalse;
        }

        match (self.types.kind(source_ty), self.types.kind(target_ty)) {
            (TypeKind::Value(_), TypeKind::Value(_)) => RuntimeTypeStaticFold::AlwaysFalse,
            _ => RuntimeTypeStaticFold::Dynamic,
        }
    }

    fn runtime_pattern_match_kind(
        &self,
        descriptor: &RuntimeTypeDescriptorKey,
        parameterized: &RuntimeTypeParameterizedMatch,
    ) -> RuntimePatternTypeTestKind {
        if !matches!(parameterized, RuntimeTypeParameterizedMatch::None) {
            return RuntimePatternTypeTestKind::RuntimeParameterized;
        }

        match &descriptor.kind {
            RuntimeTypeDescriptorKind::Nominal {
                kind: Some(ast::TypeKind::Class),
                ..
            } => RuntimePatternTypeTestKind::RuntimeClass,
            RuntimeTypeDescriptorKind::Nominal {
                kind: Some(ast::TypeKind::Interface),
                ..
            } => RuntimePatternTypeTestKind::RuntimeInterface,
            RuntimeTypeDescriptorKind::Nominal { .. } => RuntimePatternTypeTestKind::RuntimeNominal,
            RuntimeTypeDescriptorKind::Any
            | RuntimeTypeDescriptorKind::String
            | RuntimeTypeDescriptorKind::Function
            | RuntimeTypeDescriptorKind::Union => RuntimePatternTypeTestKind::RuntimeRef,
            RuntimeTypeDescriptorKind::Option
            | RuntimeTypeDescriptorKind::Tuple
            | RuntimeTypeDescriptorKind::Value
            | RuntimeTypeDescriptorKind::TypeParam
            | RuntimeTypeDescriptorKind::StarProjection => RuntimePatternTypeTestKind::StaticValue,
        }
    }

    fn lower_short_circuit_binary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return result;
        }

        let rhs_bb = self.push_block(rhs.span);
        let short_bb = self.push_block(span);
        let merge_bb = self.push_block(span);
        let parent = self.current_bb;

        let (then_target, else_target, short_value) = match op {
            ast::BinaryOp::LogAnd => (rhs_bb, short_bb, false),
            ast::BinaryOp::LogOr => (short_bb, rhs_bb, true),
            _ => unreachable!("caller guarantees short-circuit op"),
        };

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(lhs_local),
                then_target,
                else_target,
            },
        );

        self.current_bb = short_bb;
        self.assign(
            span,
            result,
            Rvalue::Use(Operand::Const(ConstValue::Bool(short_value))),
        );
        self.set_terminator(short_bb, span, TerminatorKind::Goto { target: merge_bb });

        self.current_bb = rhs_bb;
        let rhs_local = self.lower_expr_to_local(rhs);
        if !self.current_is_terminated() {
            self.assign_use_to_local(span, result, Operand::Local(rhs_local));
            self.set_terminator(rhs_bb, span, TerminatorKind::Goto { target: merge_bb });
        }

        self.current_bb = merge_bb;
        result
    }

    fn lower_type_check_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        value: &hir::Expr,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let value_local = self.lower_expr_to_local(value);
        if self.current_is_terminated() {
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::TypeCheck {
                value: Operand::Local(value_local),
                op,
                test_ty,
                metadata: self.runtime_type_test_metadata(value.ty, test_ty),
            },
        );
        result
    }

    fn lower_cast_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        value: &hir::Expr,
        op: ast::CastOp,
        target_ty: TypeId,
    ) -> LocalId {
        let result_ty = if op == ast::CastOp::As { target_ty } else { ty };
        let result = self.push_temp_local(span, result_ty);
        let value_local = self.lower_expr_to_local(value);
        if self.current_is_terminated() {
            return result;
        }
        if op == ast::CastOp::As {
            self.lower_cast_as_expr_with_runtime_error_boundary(
                span,
                result,
                value,
                value_local,
                target_ty,
            );
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::Cast {
                value: Operand::Local(value_local),
                op,
                target_ty,
                metadata: self.runtime_cast_metadata(value.ty, target_ty, ty, op),
            },
        );
        result
    }

    fn lower_cast_as_expr_with_runtime_error_boundary(
        &mut self,
        span: Span,
        result: LocalId,
        value: &hir::Expr,
        value_local: LocalId,
        target_ty: TypeId,
    ) {
        let mut metadata =
            self.runtime_cast_metadata(value.ty, target_ty, target_ty, ast::CastOp::As);
        let test_local = self.push_temp_local(span, self.builtins.bool_);
        self.assign(
            span,
            test_local,
            Rvalue::TypeCheck {
                value: Operand::Local(value_local),
                op: ast::TypeCheckOp::Is,
                test_ty: target_ty,
                metadata: metadata.test.clone(),
            },
        );

        let ok_bb = self.push_block(span);
        let fail_bb = self.push_block(span);
        let merge_bb = self.push_block(span);
        let parent = self.current_bb;
        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(test_local),
                then_target: ok_bb,
                else_target: fail_bb,
            },
        );

        self.current_bb = ok_bb;
        metadata.test.static_fold = RuntimeTypeStaticFold::AlwaysTrue;
        self.assign(
            span,
            result,
            Rvalue::Cast {
                value: Operand::Local(value_local),
                op: ast::CastOp::As,
                target_ty,
                metadata,
            },
        );
        self.set_terminator(ok_bb, span, TerminatorKind::Goto { target: merge_bb });

        self.current_bb = fail_bb;
        self.lower_cast_as_failure_raise(span, result, merge_bb);

        self.current_bb = merge_bb;
    }

    fn lower_cast_as_failure_raise(&mut self, span: Span, result: LocalId, merge_bb: BasicBlockId) {
        let runtime_error_ty = find_runtime_error_type(self.types).unwrap_or(self.builtins.any);
        let effect_ty = find_raise_runtime_error_effect(self.types).unwrap_or(self.builtins.any);
        let error_local = self.push_temp_local(span, runtime_error_ty);
        self.assign(
            span,
            error_local,
            Rvalue::TopLevelRef(TopLevelRef {
                fqn: "scoop.core.RuntimeError.ClassCastFailed".to_string(),
                site_id: None,
                hidden_effects: EffectRow::pure(),
            }),
        );

        let perform_result = self.push_temp_local(span, self.builtins.nothing);
        self.assign(
            span,
            perform_result,
            Rvalue::PerformResult {
                op_fqn: "scoop.core.Raise.raise".to_string(),
                effect_ty,
            },
        );

        let resume_target = self.push_block(span);
        let site_id = self.fresh_site_id();
        let unwind = self.build_perform_unwind_action(span);
        let payload_transport = self.value_transport_with_boxing_reason(
            runtime_error_ty,
            MirTransportKind::EffectPayload,
            MirBoxingReason::EffectPayload,
            Some(runtime_error_ty),
        );
        self.set_terminator_with_unwind(
            self.current_bb,
            span,
            TerminatorKind::Perform {
                site_id,
                op_fqn: "scoop.core.Raise.raise".to_string(),
                metadata: PerformMetadata {
                    effect_ty,
                    result_ty: self.builtins.nothing,
                    payload_tuple_ty: Some(runtime_error_ty),
                    payload_component_tys: vec![runtime_error_ty],
                    payload_transport: vec![payload_transport],
                    arg_mapping: vec![0],
                },
                args: vec![PerformArg {
                    span,
                    source_arg_index: 0,
                    name: None,
                    value: Operand::Local(error_local),
                }],
                resume_target,
            },
            unwind,
        );

        self.current_bb = resume_target;
        self.assign_use_to_local(span, result, Operand::Local(perform_result));
        self.set_terminator(
            resume_target,
            span,
            TerminatorKind::Goto { target: merge_bb },
        );
    }

    fn lower_member_access_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> LocalId {
        let tuple_member = self.tuple_member_access(member, receiver.ty);
        let result_ty = tuple_member
            .map(|(_, elem_ty)| elem_ty)
            .or_else(|| self.member_value_ty(member))
            .unwrap_or(ty);
        let result = self.push_temp_local(span, result_ty);
        let receiver_local = self.lower_expr_to_local(receiver);
        if self.current_is_terminated() {
            return result;
        }
        let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        let tuple_member = tuple_member.or_else(|| self.tuple_member_access(member, receiver_ty));
        if let Some((index, _)) = tuple_member {
            self.assign(
                span,
                result,
                Rvalue::TupleGet {
                    tuple: Operand::Local(receiver_local),
                    index,
                },
            );
        } else {
            let member = self.lower_member_access_metadata(member, receiver_ty);
            let site_id = (!member.hidden_effects.is_pure()).then(|| self.fresh_site_id());
            self.assign(
                span,
                result,
                Rvalue::MemberAccess {
                    site_id,
                    receiver: Operand::Local(receiver_local),
                    member,
                },
            );
        }
        result
    }

    fn tuple_member_access(
        &self,
        member: &hir::MemberAccess,
        receiver_ty: TypeId,
    ) -> Option<(usize, TypeId)> {
        if member.resolved.is_some() {
            return None;
        }
        let index = parse_tuple_member_index(&member.name)?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(receiver_ty) else {
            return None;
        };
        elements.get(index).copied().map(|elem_ty| (index, elem_ty))
    }

    fn member_value_ty(&self, member: &hir::MemberAccess) -> Option<TypeId> {
        let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
            return None;
        };
        self.facts.member_value_tys.get(fqn).copied()
    }

    fn lower_member_access_metadata(
        &self,
        member: &hir::MemberAccess,
        receiver_ty: TypeId,
    ) -> MemberAccessMetadata {
        let resolved = member.resolved.as_ref().map(|resolved| match resolved {
            hir::MemberRef::Value { fqn, .. } => MemberTarget::Value { fqn: fqn.clone() },
            hir::MemberRef::Fun { fqn, .. } => MemberTarget::Fun { fqn: fqn.clone() },
            hir::MemberRef::ExtensionValue { fqn, .. } => {
                MemberTarget::ExtensionValue { fqn: fqn.clone() }
            }
            hir::MemberRef::ExtensionFun { fqn, .. } => {
                MemberTarget::ExtensionFun { fqn: fqn.clone() }
            }
        });
        let hidden_effects = match &resolved {
            Some(MemberTarget::Value { fqn }) => self.facts.object_member_hidden_effects(fqn),
            _ => EffectRow::pure(),
        };
        MemberAccessMetadata {
            name: member.name.clone(),
            receiver_ty,
            resolved,
            hidden_effects,
        }
    }

    fn lower_call_args(&mut self, args: &[hir::CallArg]) -> Option<Vec<CallArg>> {
        self.lower_call_args_with_expected(args, &[])
    }

    fn lower_call_args_with_expected(
        &mut self,
        args: &[hir::CallArg],
        expected_tys: &[Option<TypeId>],
    ) -> Option<Vec<CallArg>> {
        let mut out = Vec::with_capacity(args.len());
        for arg in args {
            if self.current_is_terminated() {
                return None;
            }
            let arg_index = out.len();
            match arg {
                hir::CallArg::Positional(expr) => {
                    let value = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return None;
                    }
                    let operand = expected_tys
                        .get(arg_index)
                        .and_then(|ty| *ty)
                        .map(|target_ty| {
                            self.operand_for_target_ty(expr.span, Operand::Local(value), target_ty)
                        })
                        .unwrap_or(Operand::Local(value));
                    out.push(CallArg {
                        span: expr.span,
                        name: None,
                        value: operand,
                    });
                }
                hir::CallArg::Named { name, value, .. } => {
                    let operand_local = self.lower_expr_to_local(value);
                    if self.current_is_terminated() {
                        return None;
                    }
                    let operand = expected_tys
                        .get(arg_index)
                        .and_then(|ty| *ty)
                        .map(|target_ty| {
                            self.operand_for_target_ty(
                                value.span,
                                Operand::Local(operand_local),
                                target_ty,
                            )
                        })
                        .unwrap_or(Operand::Local(operand_local));
                    out.push(CallArg {
                        span: value.span,
                        name: Some(name.clone()),
                        value: operand,
                    });
                }
            }
        }
        Some(out)
    }

    /// 将 HIR side table 发布的 call-arg binding 收口为稳定的 MIR 槽位顺序。
    ///
    /// 这里仅处理当前 refactor 主线已显式 contract 化的简单 receiver/explicit case；
    /// 对 default/vararg/spread 等更复杂形状维持原顺序，避免在 MIR lowering 现场猜测。
    fn canonicalize_call_args_from_binding(
        &self,
        args: Vec<CallArg>,
        binding: Option<&CallArgBindingContract>,
    ) -> Vec<CallArg> {
        let Some(binding) = binding else {
            return args;
        };

        let mut claimed_source_args = vec![false; args.len()];
        let mut ordered_source_indices = Vec::with_capacity(binding.params().len());
        let mut receiver_slot: Option<usize> = None;

        for (param_idx, param) in binding.params().iter().enumerate() {
            match param {
                CallArgParamContract::Explicit(element) => {
                    if element.spread() {
                        return args;
                    }
                    let source_arg_idx = element.arg_index();
                    if source_arg_idx >= args.len() || claimed_source_args[source_arg_idx] {
                        return args;
                    }
                    claimed_source_args[source_arg_idx] = true;
                    ordered_source_indices.push(source_arg_idx);
                }
                CallArgParamContract::Receiver => {
                    if receiver_slot.replace(param_idx).is_some() {
                        return args;
                    }
                    ordered_source_indices.push(usize::MAX);
                }
                CallArgParamContract::Default | CallArgParamContract::Vararg(_) => {
                    return args;
                }
            }
        }

        if ordered_source_indices.len() != args.len() {
            return args;
        }

        let receiver_source_arg_idx = if receiver_slot.is_some() {
            let mut unclaimed = claimed_source_args
                .iter()
                .enumerate()
                .filter_map(|(idx, claimed)| (!*claimed).then_some(idx));
            let Some(receiver_source_arg_idx) = unclaimed.next() else {
                return args;
            };
            if unclaimed.next().is_some() {
                return args;
            }
            Some(receiver_source_arg_idx)
        } else {
            if claimed_source_args.iter().any(|claimed| !*claimed) {
                return args;
            }
            None
        };

        let mut ordered = Vec::with_capacity(args.len());
        for source_arg_idx in ordered_source_indices {
            let source_arg_idx = if source_arg_idx == usize::MAX {
                receiver_source_arg_idx.expect("receiver slot should exist when placeholder is used")
            } else {
                source_arg_idx
            };
            let mut arg = args[source_arg_idx].clone();
            arg.name = None;
            ordered.push(arg);
        }
        ordered
    }

    fn source_arg_expected_tys_for_function(
        &self,
        function: &FunctionTargetContract,
        explicit_arg_count: usize,
        args_include_receiver: bool,
    ) -> Vec<Option<TypeId>> {
        let mut expected = vec![None; explicit_arg_count];
        let Some(param_tys) = self.top_level_fun_param_tys.get(function.fqn()) else {
            return expected;
        };
        if let Some(binding) = function.arg_binding() {
            if args_include_receiver && call_arg_binding_has_receiver(binding) {
                return expected;
            }
            self.fill_expected_tys_from_arg_binding(&mut expected, param_tys, binding);
            return expected;
        }
        for (index, target_ty) in param_tys.iter().copied().enumerate().take(expected.len()) {
            expected[index] = Some(target_ty);
        }
        expected
    }

    fn fill_expected_tys_from_arg_binding(
        &self,
        expected: &mut [Option<TypeId>],
        param_tys: &[TypeId],
        binding: &CallArgBindingContract,
    ) {
        let mut claimed_source_args = vec![false; expected.len()];
        let mut receiver_target_ty = None;
        for (param_index, param) in binding.params().iter().enumerate() {
            let Some(target_ty) = param_tys.get(param_index).copied() else {
                continue;
            };
            match param {
                CallArgParamContract::Receiver => receiver_target_ty = Some(target_ty),
                CallArgParamContract::Explicit(element) => {
                    if let Some(slot) = expected.get_mut(element.arg_index()) {
                        *slot = Some(target_ty);
                        claimed_source_args[element.arg_index()] = true;
                    }
                }
                CallArgParamContract::Vararg(elements) => {
                    for element in elements {
                        if let Some(slot) = expected.get_mut(element.arg_index()) {
                            *slot = Some(target_ty);
                            claimed_source_args[element.arg_index()] = true;
                        }
                    }
                }
                CallArgParamContract::Default => {}
            }
        }
        if let Some(target_ty) = receiver_target_ty {
            let mut unclaimed = claimed_source_args
                .iter()
                .enumerate()
                .filter_map(|(idx, claimed)| (!*claimed).then_some(idx));
            if let Some(receiver_idx) = unclaimed.next()
                && unclaimed.next().is_none()
                && let Some(slot) = expected.get_mut(receiver_idx)
            {
                *slot = Some(target_ty);
            }
        }
    }

    fn source_arg_expected_tys_for_callee_ty(
        &self,
        callee_ty: TypeId,
        explicit_arg_count: usize,
        binding: Option<&CallArgBindingContract>,
    ) -> Vec<Option<TypeId>> {
        let mut expected = vec![None; explicit_arg_count];
        let TypeKind::Ref(RefTypeKind::Function(fun)) = self.types.kind(callee_ty) else {
            return expected;
        };
        let mut param_tys = Vec::with_capacity(fun.params.len() + usize::from(fun.receiver.is_some()));
        if let Some(receiver_ty) = fun.receiver {
            param_tys.push(receiver_ty);
        }
        param_tys.extend(fun.params.iter().copied());
        if let Some(binding) = binding {
            self.fill_expected_tys_from_arg_binding(&mut expected, &param_tys, binding);
            return expected;
        }
        for (index, target_ty) in param_tys.iter().copied().enumerate().take(expected.len()) {
            expected[index] = Some(target_ty);
        }
        expected
    }

    fn lower_refactor_typed_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> bool {
        let Some(contract) = self
            .facts
            .refactor_call_site_contract(self.source_path.as_path(), span)
            .cloned()
        else {
            return false;
        };
        if !self.typed_call_contract_matches_callee(&contract, callee) {
            return false;
        }

        match contract {
            TypedCallSiteContract::DirectTopLevel(function) => {
                self.lower_refactor_direct_call_expr(
                    span,
                    result,
                    function.fqn(),
                    args,
                    Some(&function),
                );
                true
            }
            TypedCallSiteContract::MemberDirect(member) => {
                self.lower_refactor_direct_call_expr(
                    span,
                    result,
                    member.function().fqn(),
                    args,
                    Some(member.function()),
                );
                true
            }
            TypedCallSiteContract::Extension { function, .. } => {
                self.lower_refactor_direct_call_expr(
                    span,
                    result,
                    function.fqn(),
                    args,
                    Some(&function),
                );
                true
            }
            TypedCallSiteContract::Constructor(ctor) => {
                self.lower_refactor_constructor_call_expr(span, result, &ctor, args);
                true
            }
            TypedCallSiteContract::Closure { arg_binding, .. } => {
                self.lower_refactor_callable_value_call_expr(
                    span,
                    result,
                    callee,
                    args,
                    arg_binding.as_ref(),
                    true,
                );
                true
            }
            TypedCallSiteContract::FunValue { arg_binding, .. }
            | TypedCallSiteContract::FunPtr { arg_binding, .. } => {
                self.lower_refactor_callable_value_call_expr(
                    span,
                    result,
                    callee,
                    args,
                    arg_binding.as_ref(),
                    false,
                );
                true
            }
            TypedCallSiteContract::Virtual(member) => {
                self.lower_refactor_dispatch_call_expr_from_contract(
                    span,
                    result,
                    callee,
                    args,
                    DispatchTargetKind::Virtual,
                    &member,
                );
                true
            }
            TypedCallSiteContract::Interface(member) => {
                self.lower_refactor_dispatch_call_expr_from_contract(
                    span,
                    result,
                    callee,
                    args,
                    DispatchTargetKind::Interface,
                    &member,
                );
                true
            }
            TypedCallSiteContract::Intrinsic { kind, function } => {
                self.lower_refactor_intrinsic_call_expr(span, result, &kind, function.fqn(), args)
            }
            TypedCallSiteContract::EffectOp(_) | TypedCallSiteContract::ContinuationResume(_) => {
                false
            }
        }
    }

    fn typed_call_contract_matches_callee(
        &self,
        contract: &TypedCallSiteContract,
        callee: &hir::Expr,
    ) -> bool {
        let Some(callee_fqn) = top_level_callee_fqn(callee) else {
            return true;
        };
        let contract_fqn = match contract {
            TypedCallSiteContract::DirectTopLevel(function) => function.fqn(),
            TypedCallSiteContract::MemberDirect(member) => member.function().fqn(),
            TypedCallSiteContract::Extension { function, .. }
            | TypedCallSiteContract::Intrinsic { function, .. } => function.fqn(),
            TypedCallSiteContract::Constructor(_)
            | TypedCallSiteContract::Closure { .. }
            | TypedCallSiteContract::FunValue { .. }
            | TypedCallSiteContract::FunPtr { .. }
            | TypedCallSiteContract::Virtual(_)
            | TypedCallSiteContract::Interface(_)
            | TypedCallSiteContract::EffectOp(_)
            | TypedCallSiteContract::ContinuationResume(_) => return true,
        };
        intrinsic_base_fqn(contract_fqn) == intrinsic_base_fqn(callee_fqn)
    }

    fn lower_refactor_direct_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee_fqn: &str,
        args: &[hir::CallArg],
        function: Option<&FunctionTargetContract>,
    ) {
        let arg_binding = function
            .and_then(FunctionTargetContract::arg_binding)
            .filter(|binding| !call_arg_binding_has_receiver(binding));
        let expected_tys = function
            .map(|function| self.source_arg_expected_tys_for_function(function, args.len(), true))
            .unwrap_or_else(|| vec![None; args.len()]);
        let Some(args) = self.lower_call_args_with_expected(args, &expected_tys) else {
            return;
        };
        let args = self.canonicalize_call_args_from_binding(args, arg_binding);
        let kind = CallKind::Direct {
            callee_fqn: callee_fqn.to_string(),
        };
        let terminates_current_block = matches!(
            &kind,
            CallKind::Direct { callee_fqn } if callee_fqn == "scoop.core.panic"
        );
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        if terminates_current_block {
            self.set_terminator(self.current_bb, span, TerminatorKind::Unreachable);
        }
    }

    fn lower_refactor_constructor_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        ctor: &crate::effect_refactor_pipeline::ConstructorCallTargetContract,
        args: &[hir::CallArg],
    ) {
        let Some(args) = self.lower_call_args(args) else {
            return;
        };
        let hidden_effects = self
            .facts
            .class_ctor_hidden_effects(self.source_path.as_path(), span);
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::ClassCtor {
                site_id,
                class_fqn: ctor.owner_fqn().to_string(),
                ctor: ClassCtorCallMetadata {
                    selected_ctor_span: ctor.ctor_span(),
                    ordered_param_count: ctor.arg_mapping().len(),
                },
                args,
                hidden_effects,
            },
        );
    }

    fn lower_refactor_callable_value_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        arg_binding: Option<&CallArgBindingContract>,
        prefer_closure_kind: bool,
    ) {
        let callee_local = self.lower_expr_to_local(callee);
        if self.current_is_terminated() {
            return;
        }
        let callee_ty = self.body.locals[callee_local.as_u32() as usize].ty;
        let expected_tys =
            self.source_arg_expected_tys_for_callee_ty(callee_ty, args.len(), arg_binding);
        let Some(args) = self.lower_call_args_with_expected(args, &expected_tys) else {
            return;
        };
        let args = self.canonicalize_call_args_from_binding(args, arg_binding);
        let origin = self.value_origins.get(&callee_local).cloned();
        let gc_intrinsic_callee =
            gc_intrinsic_callee_from_origin(origin.as_ref()).map(str::to_string);
        let kind = match (prefer_closure_kind, origin) {
            (true, Some(ValueOrigin::Closure { fn_ptr })) => CallKind::Closure {
                callee: Operand::Local(callee_local),
                fn_ptr,
            },
            _ => CallKind::FunValue {
                callee: Operand::Local(callee_local),
            },
        };
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            gc_intrinsic_callee.as_deref(),
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    fn lower_refactor_intrinsic_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        kind: &TypedIntrinsicKind,
        callee_fqn: &str,
        args: &[hir::CallArg],
    ) -> bool {
        let intrinsic_fqn = intrinsic_base_fqn(callee_fqn);
        match (kind, intrinsic_fqn) {
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.sizeOf") if name == "sizeOf" => {
                let value_ty = args
                    .first()
                    .map(|arg| match arg {
                        hir::CallArg::Positional(value) => value.ty,
                        hir::CallArg::Named { value, .. } => value.ty,
                    })
                    .or_else(|| {
                        self.facts
                            .refactor_call_site_contract(self.source_path.as_path(), span)
                            .and_then(|contract| match contract {
                                TypedCallSiteContract::Intrinsic { function, .. } => {
                                    function.type_args().first().copied()
                                }
                                _ => None,
                            })
                    })
                    .expect("typed sizeOf intrinsic must publish a value or type argument");
                self.assign(span, result, Rvalue::SizeOf { value_ty });
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.nameOf") if name == "nameOf" => {
                let source_ty = self
                    .facts
                    .refactor_call_site_contract(self.source_path.as_path(), span)
                    .and_then(|contract| match contract {
                        TypedCallSiteContract::Intrinsic { function, .. } => {
                            function.type_args().first().copied()
                        }
                        _ => None,
                    })
                    .expect("typed nameOf intrinsic must publish a type argument");
                self.assign(
                    span,
                    result,
                    Rvalue::TypeMetadataLiteral(TypeMetadataLiteral {
                        source_ty,
                        source_fqn: self.nominal_fqn_for_ty(source_ty),
                        kind: TypeMetadataLiteralKind::TypeNameString,
                    }),
                );
                true
            }
            _ => {
                self.lower_refactor_direct_call_expr(span, result, callee_fqn, args, None);
                true
            }
        }
    }

    fn lower_refactor_dispatch_call_expr_from_contract(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        dispatch_kind: DispatchTargetKind,
        member: &MemberCallTargetContract,
    ) {
        let (receiver_expr, call_args) = self.dispatch_receiver_and_args(callee, args);
        let receiver_local = self.lower_expr_to_local(receiver_expr);
        if self.current_is_terminated() {
            return;
        }
        let expected_tys =
            self.source_arg_expected_tys_for_function(member.function(), call_args.len(), false);
        let Some(args) = self.lower_call_args_with_expected(call_args, &expected_tys) else {
            return;
        };
        let dispatch = DispatchMetadata {
            owner_fqn: member.owner_fqn().to_string(),
            member_name: member.member_name().to_string(),
            member_fqn: member.member_fqn().to_string(),
            member_decl_span: member.function().decl_span(),
            receiver_ty: member.receiver_ty(),
        };
        let kind = match dispatch_kind {
            DispatchTargetKind::Virtual => CallKind::Virtual {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
            DispatchTargetKind::Interface => CallKind::Interface {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
        };
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    fn dispatch_receiver_and_args<'b>(
        &self,
        callee: &'b hir::Expr,
        args: &'b [hir::CallArg],
    ) -> (&'b hir::Expr, &'b [hir::CallArg]) {
        match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {
                let Some((receiver_arg, remaining_args)) = args.split_first() else {
                    panic!("typed dispatch call contract must include a receiver argument")
                };
                let receiver_expr = match receiver_arg {
                    hir::CallArg::Positional(expr) => expr,
                    hir::CallArg::Named { value, .. } => value,
                };
                (receiver_expr, remaining_args)
            }
            hir::ExprKind::MemberAccess { receiver, .. } => (receiver.as_ref(), args),
            _ => panic!("typed dispatch call contract must match a dispatch callee shape"),
        }
    }

    fn nominal_fqn_for_ty(&self, ty: TypeId) -> Option<String> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.clone()),
            _ => None,
        }
    }

    fn operand_ty(&self, operand: &Operand) -> TypeId {
        match operand {
            Operand::Local(local) => self.body.locals[local.as_u32() as usize].ty,
            Operand::Const(ConstValue::Bool(_)) => self.builtins.bool_,
            Operand::Const(ConstValue::Char) => self.builtins.char_,
            Operand::Const(ConstValue::Unit) => self.builtins.unit,
            Operand::Const(ConstValue::Int) => self.builtins.int,
            Operand::Const(ConstValue::SynthInt(_)) => self.builtins.int,
            Operand::Const(ConstValue::Float64) => self.builtins.float64,
            Operand::Const(ConstValue::Float32) => self.builtins.float32,
            Operand::Const(ConstValue::String) => self.builtins.string,
        }
    }

    fn transport_kind_for_ty(&self, ty: TypeId) -> MirTransportKind {
        mir_transport_kind_for_ty(self.types, self.facts, ty)
    }

    fn is_aggregate_transport_ty(&self, ty: TypeId) -> bool {
        mir_is_aggregate_transport_ty(self.types, ty)
    }

    fn transport_requirements(&self, ty: TypeId) -> MirTransportRequirements {
        mir_transport_requirements(self.types, ty)
    }

    fn value_transport_with_kind(
        &self,
        ty: TypeId,
        kind: MirTransportKind,
    ) -> ValueTransportMetadata {
        ValueTransportMetadata {
            source_ty: ty,
            kind,
            requirements: self.transport_requirements(ty),
            boxing: None,
        }
    }

    fn value_transport(&self, ty: TypeId) -> ValueTransportMetadata {
        self.value_transport_with_kind(ty, self.transport_kind_for_ty(ty))
    }

    fn value_transport_with_boxing_reason(
        &self,
        ty: TypeId,
        kind: MirTransportKind,
        reason: MirBoxingReason,
        target_ty: Option<TypeId>,
    ) -> ValueTransportMetadata {
        let mut transport = self.value_transport_with_kind(ty, kind);
        if self.is_aggregate_transport_ty(ty) {
            transport.boxing = Some(MirBoxingIntent {
                source_ty: ty,
                target_ty,
                reason,
            });
        }
        transport
    }

    fn aggregate_transport(
        &self,
        aggregate_ty: TypeId,
        kind: AggregateTransportKind,
        fields: impl IntoIterator<Item = (Option<String>, TypeId)>,
    ) -> AggregateTransportMetadata {
        AggregateTransportMetadata {
            aggregate_ty,
            kind,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, (name, ty))| AggregateTransportField {
                    index,
                    name,
                    ty,
                    transport: self.value_transport(ty),
                })
                .collect(),
        }
    }

    fn capture_box_contract(
        &self,
        box_ty: TypeId,
        inner_ty: TypeId,
    ) -> CaptureBoxTransportMetadata {
        CaptureBoxTransportMetadata {
            box_ty,
            value: self.value_transport_with_boxing_reason(
                inner_ty,
                self.transport_kind_for_ty(inner_ty),
                MirBoxingReason::ClosureCapture,
                Some(box_ty),
            ),
        }
    }

    fn closure_env_contract(
        &self,
        env_ty: TypeId,
        captures: &[ClosureCaptureLayout],
    ) -> ClosureEnvTransportMetadata {
        ClosureEnvTransportMetadata {
            env_ty,
            captures: captures
                .iter()
                .map(|capture| {
                    let kind = if capture.mutable {
                        MirTransportKind::CaptureBox
                    } else {
                        self.transport_kind_for_ty(capture.ty)
                    };
                    let transport = if capture.mutable {
                        self.value_transport_with_kind(capture.ty, kind)
                    } else {
                        self.value_transport_with_boxing_reason(
                            capture.ty,
                            kind,
                            MirBoxingReason::ClosureCapture,
                            Some(env_ty),
                        )
                    };
                    ClosureCaptureTransportMetadata {
                        name: capture.name.clone(),
                        decl_span: capture.decl_span,
                        mutable: capture.mutable,
                        source_local: capture.source_local,
                        transport,
                    }
                })
                .collect(),
        }
    }

    fn call_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
        gc_intrinsic_callee: Option<&str>,
    ) -> CallTransportMetadata {
        let result = self.value_transport(result_ty);
        let aggregate_return = self
            .is_aggregate_transport_ty(result_ty)
            .then(|| result.clone());
        CallTransportMetadata {
            result,
            aggregate_return,
            array: self.array_transport_metadata(result_ty, kind, args),
            gc: self.gc_intrinsic_transport_metadata(result_ty, kind, args, gc_intrinsic_callee),
            thread_resume_payload: self.thread_resume_payload_transport_metadata(kind, args),
            abi: self.call_abi_handoff(kind),
        }
    }

    fn thread_resume_payload_transport_metadata(
        &self,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Option<Box<ValueTransportMetadata>> {
        let CallKind::Direct { callee_fqn } = kind else {
            return None;
        };
        let base = intrinsic_base_fqn(callee_fqn);
        if !matches!(
            base,
            THREAD_SPAWN_JOIN_RESUME_FQN | THREAD_SPAWN_JOIN_RESUME_U64_FQN
        ) {
            return None;
        }
        let payload_ty = args
            .first()
            .map(|arg| self.operand_ty(&arg.value))
            .and_then(|ty| {
                continuation_contract_from_type(self.types, ty).map(|(resume, _, _)| resume)
            })
            .or_else(|| args.get(1).map(|arg| self.operand_ty(&arg.value)))?;
        Some(Box::new(self.value_transport_with_boxing_reason(
            payload_ty,
            MirTransportKind::EffectPayload,
            MirBoxingReason::EffectPayload,
            Some(payload_ty),
        )))
    }

    fn gc_intrinsic_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
        gc_intrinsic_callee: Option<&str>,
    ) -> Option<GcIntrinsicTransportMetadata> {
        let callee_fqn = match gc_intrinsic_callee {
            Some(callee_fqn) => callee_fqn,
            None => match kind {
                CallKind::Direct { callee_fqn } => callee_fqn.as_str(),
                CallKind::Closure { .. }
                | CallKind::FunValue { .. }
                | CallKind::Virtual { .. }
                | CallKind::Interface { .. }
                | CallKind::Resume { .. } => return None,
            },
        };
        let subject_ty = args
            .first()
            .map(|arg| self.operand_ty(&arg.value))
            .unwrap_or(self.builtins.any);
        let subject = self.value_transport(subject_ty);

        match callee_fqn {
            "scoop.core.GC.pin" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::Pin,
                root_lifetime: GcRootLifetime::PinnedUntilUnpin,
                pairing: GcIntrinsicPairing::PinMustPairUnpin,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(result_ty),
                subject,
            }),
            "scoop.core.GC.unpin" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::Unpin,
                root_lifetime: GcRootLifetime::EndsPinnedRoot,
                pairing: GcIntrinsicPairing::UnpinMatchesPin,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(subject_ty),
                subject,
            }),
            "scoop.core.GC.handleNew" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::HandleNew,
                root_lifetime: GcRootLifetime::StableHandleUntilDrop,
                pairing: GcIntrinsicPairing::HandleNewMustPairDrop,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(result_ty),
                subject,
            }),
            "scoop.core.GC.handleGet" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::HandleGet,
                root_lifetime: GcRootLifetime::BorrowedFromStableHandle,
                pairing: GcIntrinsicPairing::HandleGetRequiresLiveHandle,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(subject_ty),
                subject,
            }),
            "scoop.core.GC.handleDrop" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::HandleDrop,
                root_lifetime: GcRootLifetime::EndsStableHandle,
                pairing: GcIntrinsicPairing::HandleDropMatchesHandleNew,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(subject_ty),
                subject,
            }),
            _ => None,
        }
    }

    fn call_abi_handoff(&self, kind: &CallKind) -> CallAbiHandoffMetadata {
        match kind {
            CallKind::Direct { callee_fqn } if Self::is_plain_no_outward_intrinsic(callee_fqn) => {
                CallAbiHandoffMetadata::plain_no_outward()
            }
            _ => CallAbiHandoffMetadata::deferred_to_effect_facts(),
        }
    }

    fn is_plain_no_outward_intrinsic(fqn: &str) -> bool {
        matches!(
            fqn,
            ARRAY_BUILDER_NEW_FQN
                | ARRAY_BUILDER_PUSH_FQN
                | ARRAY_BUILDER_PUSH_STRING_FQN
                | ARRAY_BUILDER_BUILD_ARRAY_FQN
                | ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN
                | ARRAY_BUILDER_BUILD_ARRAY_STRING_FQN
        )
    }

    fn array_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Option<ArrayElementTransportMetadata> {
        let CallKind::Direct { callee_fqn } = kind else {
            return None;
        };
        match callee_fqn.as_str() {
            ARRAY_BUILDER_PUSH_FQN | ARRAY_BUILDER_PUSH_STRING_FQN => {
                let builder_ty = args
                    .first()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                let element_ty = args
                    .get(1)
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::BuilderPush,
                    array_ty: builder_ty,
                    element_ty,
                    mutable: true,
                    element: self.value_transport_with_boxing_reason(
                        element_ty,
                        MirTransportKind::ArrayElement,
                        MirBoxingReason::ArrayElement,
                        Some(builder_ty),
                    ),
                })
            }
            ARRAY_BUILDER_BUILD_ARRAY_FQN | ARRAY_BUILDER_BUILD_ARRAY_STRING_FQN => {
                let element_ty = self.array_element_ty_from_array_ty(result_ty);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::BuilderBuildArray,
                    array_ty: result_ty,
                    element_ty,
                    mutable: false,
                    element: self
                        .value_transport_with_kind(element_ty, MirTransportKind::ArrayElement),
                })
            }
            ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN => {
                let element_ty = self.array_element_ty_from_array_ty(result_ty);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::BuilderBuildMutableArray,
                    array_ty: result_ty,
                    element_ty,
                    mutable: true,
                    element: self
                        .value_transport_with_kind(element_ty, MirTransportKind::ArrayElement),
                })
            }
            _ if callee_fqn.ends_with(".get") => Some(ArrayElementTransportMetadata {
                operation: ArrayTransportOperation::Get,
                array_ty: args
                    .first()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any),
                element_ty: result_ty,
                mutable: false,
                element: self.value_transport_with_kind(result_ty, MirTransportKind::ArrayElement),
            }),
            _ if callee_fqn.ends_with(".set") => {
                let array_ty = args
                    .first()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                let element_ty = args
                    .last()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::Set,
                    array_ty,
                    element_ty,
                    mutable: true,
                    element: self.value_transport_with_boxing_reason(
                        element_ty,
                        MirTransportKind::ArrayElement,
                        MirBoxingReason::ArrayElement,
                        Some(array_ty),
                    ),
                })
            }
            _ => None,
        }
    }

    fn array_element_ty_from_array_ty(&self, array_ty: TypeId) -> TypeId {
        match self.types.kind(array_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if matches!(
                    nominal.fqn.as_str(),
                    "scoop.core.Array"
                        | "scoop.core.MutableArray"
                        | "scoop.core.List"
                        | "scoop.core.MutableList"
                ) =>
            {
                nominal.args.first().copied().unwrap_or(self.builtins.any)
            }
            _ => self.builtins.any,
        }
    }

    fn canonicalize_perform_args(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lowered_args: Vec<CallArg>,
    ) -> Option<(Vec<PerformArg>, PerformMetadata)> {
        let uses_refactor_typed_contracts = self.facts.uses_refactor_typed_contracts();
        if let Some(mut metadata) = self
            .facts
            .refactor_perform_metadata(self.source_path.as_path(), span)
            .filter(|metadata| {
                if uses_refactor_typed_contracts {
                    metadata.arg_mapping.len() == lowered_args.len()
                } else {
                    metadata
                        .arg_mapping
                        .iter()
                        .all(|idx| *idx < lowered_args.len())
                }
            })
            .cloned()
        {
            let perform_args = if uses_refactor_typed_contracts {
                lowered_args
                    .iter()
                    .enumerate()
                    .map(|(param_idx, arg)| PerformArg {
                        span: arg.span,
                        source_arg_index: metadata.arg_mapping[param_idx],
                        name: arg.name.clone(),
                        value: arg.value.clone(),
                    })
                    .collect::<Vec<_>>()
            } else {
                metadata
                    .arg_mapping
                    .iter()
                    .copied()
                    .filter_map(|arg_idx| lowered_args.get(arg_idx).map(|arg| (arg_idx, arg)))
                    .map(|(source_arg_index, arg)| PerformArg {
                        span: arg.span,
                        source_arg_index,
                        name: arg.name.clone(),
                        value: arg.value.clone(),
                    })
                    .collect::<Vec<_>>()
            };
            metadata.payload_transport = perform_args
                .iter()
                .map(|arg| {
                    let ty = self.operand_ty(&arg.value);
                    self.value_transport_with_boxing_reason(
                        ty,
                        MirTransportKind::EffectPayload,
                        MirBoxingReason::EffectPayload,
                        metadata.payload_tuple_ty,
                    )
                })
                .collect();
            return Some((perform_args, metadata));
        }

        if self.facts.uses_refactor_typed_contracts() {
            return None;
        }

        let info = self.facts.legacy_perform_site_info(span);
        let arg_mapping = info
            .map(|site| site.arg_mapping.as_slice())
            .filter(|mapping| mapping.len() == lowered_args.len())
            .map(|mapping| mapping.to_vec())
            .unwrap_or_else(|| (0..lowered_args.len()).collect());

        let perform_args = lowered_args
            .iter()
            .enumerate()
            .map(|(param_idx, arg)| PerformArg {
                span: arg.span,
                source_arg_index: arg_mapping[param_idx],
                name: arg.name.clone(),
                value: arg.value.clone(),
            })
            .collect::<Vec<_>>();

        let payload_tuple_ty = info.and_then(|site| site.payload_tuple_ty).or_else(|| {
            (perform_args.len() > 1).then(|| {
                self.types.ty_tuple(
                    perform_args
                        .iter()
                        .map(|arg| self.operand_ty(&arg.value))
                        .collect(),
                )
            })
        });

        let payload_component_tys = perform_args
            .iter()
            .map(|arg| self.operand_ty(&arg.value))
            .collect();
        let payload_transport = perform_args
            .iter()
            .map(|arg| {
                let ty = self.operand_ty(&arg.value);
                self.value_transport_with_boxing_reason(
                    ty,
                    MirTransportKind::EffectPayload,
                    MirBoxingReason::EffectPayload,
                    payload_tuple_ty,
                )
            })
            .collect();

        Some((
            perform_args,
            PerformMetadata {
                effect_ty: self.builtins.any,
                result_ty,
                payload_tuple_ty,
                payload_component_tys,
                payload_transport,
                arg_mapping,
            },
        ))
    }

    fn lower_call_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> LocalId {
        let result_ty = self.call_result_ty_from_callee(span, callee).unwrap_or(ty);
        let result = self.push_temp_local(span, result_ty);

        if let Some(resume_info) = self
            .facts
            .refactor_resume_call_info(self.source_path.as_path(), span)
            .cloned()
        {
            self.lower_resume_call_expr(span, result, callee, args, Some(resume_info));
            return result;
        }

        if !self.facts.uses_refactor_typed_contracts()
            && self.facts.legacy_resume_site_matches(span)
        {
            self.lower_resume_call_expr(span, result, callee, args, None);
            return result;
        }

        if self.facts.uses_refactor_typed_contracts()
            && self.lower_refactor_typed_call_expr(span, result, callee, args)
        {
            return result;
        }

        if self.lower_dispatch_call_expr(span, result, callee, args) {
            return result;
        }

        if self.lower_reflection_intrinsic_call_expr(span, result, args) {
            return result;
        }

        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind
            && matches!(
                self.types.kind(ty),
                TypeKind::Value(ValueTypeKind::Option(_) | ValueTypeKind::Nominal(_))
            )
        {
            let Some(args) = self.lower_call_args(args) else {
                return result;
            };
            let payload = self.aggregate_transport(
                ty,
                AggregateTransportKind::EnumPayload,
                args.iter()
                    .map(|arg| (arg.name.clone(), self.operand_ty(&arg.value)))
                    .collect::<Vec<_>>(),
            );
            self.assign(
                span,
                result,
                Rvalue::EnumVariant {
                    enum_ty: ty,
                    variant_name: name.clone(),
                    args,
                    payload,
                },
            );
            return result;
        }

        let unresolved_class_ctor_fqn = match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => Some(nominal.fqn.clone()),
            _ => None,
        };
        if let hir::ExprKind::UnresolvedIdent { .. } = &callee.kind
            && let Some(callee_fqn) = unresolved_class_ctor_fqn
        {
            let Some(args) = self.lower_call_args(args) else {
                return result;
            };
            let site_id = SiteId::from_raw(u32::MAX);
            let hidden_effects = self
                .facts
                .class_ctor_hidden_effects(self.source_path.as_path(), span);
            let ctor = self
                .facts
                .class_ctor_call_info(self.source_path.as_path(), span)
                .filter(|info| info.class_fqn == callee_fqn)
                .map(|info| ClassCtorCallMetadata {
                    selected_ctor_span: info.ctor_span,
                    ordered_param_count: info.arg_mapping.len(),
                })
                .unwrap_or(ClassCtorCallMetadata {
                    selected_ctor_span: None,
                    ordered_param_count: args.len(),
                });
            self.assign(
                span,
                result,
                Rvalue::ClassCtor {
                    site_id,
                    class_fqn: callee_fqn,
                    ctor,
                    args,
                    hidden_effects,
                },
            );
            return result;
        }

        let callee_local = self.lower_expr_to_local(callee);
        if self.current_is_terminated() {
            return result;
        }
        let callee_ty = self.body.locals[callee_local.as_u32() as usize].ty;
        let mut callee_origin = self.value_origins.get(&callee_local).cloned();
        if callee_origin.is_none() && matches!(callee.kind, hir::ExprKind::Call { .. }) {
            callee_origin = Some(ValueOrigin::UnknownCallable);
        }
        let callee_can_lower = self.is_callable_value_ty(callee_ty)
            || matches!(
                callee_origin,
                Some(
                    ValueOrigin::Closure { .. }
                        | ValueOrigin::TopLevelRef { .. }
                        | ValueOrigin::MemberAccess { .. }
                        | ValueOrigin::UnknownCallable
                        | ValueOrigin::UnresolvedName { .. }
                )
            );
        if !callee_can_lower {
            self.assign(span, result, Rvalue::Todo("call callee lowering pending"));
            return result;
        }

        let kind = match callee_origin.as_ref() {
            Some(ValueOrigin::TopLevelRef { fqn }) => CallKind::Direct {
                callee_fqn: fqn.clone(),
            },
            Some(ValueOrigin::Closure { fn_ptr }) => CallKind::Closure {
                callee: Operand::Local(callee_local),
                fn_ptr: fn_ptr.clone(),
            },
            Some(ValueOrigin::UnresolvedName { .. }) => {
                self.assign(span, result, Rvalue::Todo("ctor call lowering pending"));
                return result;
            }
            Some(ValueOrigin::MemberAccess { .. }) | Some(ValueOrigin::UnknownCallable) | None => {
                CallKind::FunValue {
                    callee: Operand::Local(callee_local),
                }
            }
        };
        let arg_binding = self.facts.call_arg_binding(self.source_path.as_path(), span);
        let direct_arg_binding = arg_binding.filter(|binding| !call_arg_binding_has_receiver(binding));
        let expected_tys = match &kind {
            CallKind::Direct { callee_fqn } => self
                .top_level_fun_param_tys
                .get(callee_fqn)
                .map(|param_tys| {
                    param_tys
                        .iter()
                        .copied()
                        .map(Some)
                        .chain(std::iter::repeat(None))
                        .take(args.len())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![None; args.len()]),
            CallKind::Closure { .. } | CallKind::FunValue { .. } => {
                self.source_arg_expected_tys_for_callee_ty(callee_ty, args.len(), arg_binding)
            }
            CallKind::Virtual { .. } | CallKind::Interface { .. } | CallKind::Resume { .. } => {
                vec![None; args.len()]
            }
        };
        let Some(args) = self.lower_call_args_with_expected(args, &expected_tys) else {
            return result;
        };
        let args = self.canonicalize_call_args_from_binding(
            args,
            if matches!(kind, CallKind::Direct { .. }) {
                direct_arg_binding
            } else {
                arg_binding
            },
        );
        let terminates_current_block = matches!(
            &kind,
            CallKind::Direct { callee_fqn } if callee_fqn == "scoop.core.panic"
        );

        let site_id = self.fresh_site_id();
        let gc_intrinsic_callee =
            gc_intrinsic_callee_from_origin(callee_origin.as_ref()).map(str::to_string);
        let transport =
            self.call_transport_metadata(result_ty, &kind, &args, gc_intrinsic_callee.as_deref());
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        if terminates_current_block {
            self.set_terminator(self.current_bb, span, TerminatorKind::Unreachable);
        }
        result
    }

    fn call_result_ty_from_callee(&self, span: Span, callee: &hir::Expr) -> Option<TypeId> {
        if let Some(binding) = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)
            .filter(|binding| top_level_binding_matches_callee(binding, callee))
            && let Some(return_ty) = self.top_level_fun_return_tys.get(&binding.fqn)
        {
            return Some(*return_ty);
        }
        match self.types.kind(callee.ty) {
            TypeKind::Ref(RefTypeKind::Function(fun)) => Some(fun.return_ty),
            _ => None,
        }
    }

    fn lower_reflection_intrinsic_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        args: &[hir::CallArg],
    ) -> bool {
        let Some(binding) = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)
        else {
            return false;
        };
        if !binding.is_intrinsic {
            return false;
        }
        match intrinsic_base_fqn(&binding.fqn) {
            "scoop.core.sizeOf" => {
                let value_ty = match args {
                    [hir::CallArg::Positional(value)] => Some(value.ty),
                    [] => binding.type_args.first().copied(),
                    _ => None,
                };
                let Some(value_ty) = value_ty else {
                    self.assign(
                        span,
                        result,
                        Rvalue::Todo("sizeOf intrinsic requires value or type arg"),
                    );
                    return true;
                };
                self.assign(span, result, Rvalue::SizeOf { value_ty });
                true
            }
            "scoop.core.nameOf" => {
                let source_ty = match args {
                    [hir::CallArg::Positional(value)] => Some(value.ty),
                    [] => binding.type_args.first().copied(),
                    _ => None,
                };
                let Some(source_ty) = source_ty else {
                    self.assign(
                        span,
                        result,
                        Rvalue::Todo("nameOf intrinsic requires type arg"),
                    );
                    return true;
                };
                self.assign(
                    span,
                    result,
                    Rvalue::TypeMetadataLiteral(TypeMetadataLiteral {
                        source_ty,
                        source_fqn: self.nominal_fqn_for_ty(source_ty),
                        kind: TypeMetadataLiteralKind::TypeNameString,
                    }),
                );
                true
            }
            _ => false,
        }
    }

    fn lower_resume_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        resume_info: Option<RefactorResumeCallInfo>,
    ) {
        let (receiver, payload_args, metadata) = if let Some(info) = resume_info {
            let Some(receiver) = self.resume_receiver_from_contract(callee, args, &info) else {
                self.lower_malformed_refactor_resume_call(span, result, info.metadata);
                return;
            };
            let Some(payload_args) = self.resume_payload_args_from_contract(args, &info) else {
                self.lower_malformed_refactor_resume_call(span, result, info.metadata);
                return;
            };
            (receiver, payload_args, Some(info.metadata))
        } else {
            let (receiver, payload_args) = match &callee.kind {
                hir::ExprKind::MemberAccess { receiver, .. } => (receiver.as_ref(), args.to_vec()),
                hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {
                    let Some((hir::CallArg::Positional(receiver), payload_args)) =
                        args.split_first()
                    else {
                        self.assign(
                            span,
                            result,
                            Rvalue::Todo("resume lowering requires canonical callee shape"),
                        );
                        return;
                    };
                    (receiver, payload_args.to_vec())
                }
                _ => {
                    self.assign(
                        span,
                        result,
                        Rvalue::Todo("resume lowering requires canonical callee shape"),
                    );
                    return;
                }
            };
            (receiver, payload_args, None)
        };

        let continuation_local = self.lower_expr_to_local(receiver);
        if self.current_is_terminated() {
            return;
        }

        let Some(args) = self.lower_call_args(&payload_args) else {
            return;
        };
        let continuation_ty = self.body.locals[continuation_local.as_u32() as usize].ty;
        let resume = metadata.unwrap_or_else(|| {
            let (resume_ty, answer_ty, out_effects) = continuation_contract_from_type(
                self.types,
                continuation_ty,
            )
            .unwrap_or((self.builtins.any, self.builtins.any, EffectRow::pure()));
            ResumeMetadata {
                continuation_ty,
                resume_ty,
                answer_ty,
                return_ty: self.body.locals[result.as_u32() as usize].ty,
                out_effects: out_effects.clone(),
                runtime_error_effect_ty: find_raise_runtime_error_effect(self.types),
                suspends_outward: !out_effects.is_pure()
                    || self.facts.legacy_resume_site_suspends_outward(span),
            }
        });
        let site_id = self.fresh_site_id();
        let kind = CallKind::Resume {
            continuation: Operand::Local(continuation_local),
            resume,
        };
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    fn resume_receiver_from_contract<'b>(
        &self,
        callee: &'b hir::Expr,
        args: &'b [hir::CallArg],
        info: &RefactorResumeCallInfo,
    ) -> Option<&'b hir::Expr> {
        match info.receiver_route {
            ContinuationResumeReceiverRoute::CallArg { index } => {
                args.get(index).map(call_arg_expr)
            }
            ContinuationResumeReceiverRoute::MemberReceiver => match &callee.kind {
                hir::ExprKind::MemberAccess { receiver, .. } => Some(receiver.as_ref()),
                _ => None,
            },
        }
    }

    fn resume_payload_args_from_contract(
        &self,
        args: &[hir::CallArg],
        info: &RefactorResumeCallInfo,
    ) -> Option<Vec<hir::CallArg>> {
        info.payload_arg_indices
            .iter()
            .map(|index| args.get(*index).cloned())
            .collect()
    }

    fn lower_malformed_refactor_resume_call(
        &mut self,
        span: Span,
        result: LocalId,
        mut metadata: ResumeMetadata,
    ) {
        metadata.runtime_error_effect_ty = None;
        let site_id = self.fresh_site_id();
        let kind = CallKind::Resume {
            continuation: Operand::Const(ConstValue::Unit),
            resume: metadata,
        };
        let args = Vec::new();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    fn lower_dispatch_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> bool {
        if self.facts.uses_refactor_typed_contracts() {
            return self.lower_refactor_dispatch_call_expr(span, result, callee, args);
        }

        let dispatch_target = match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let Some((receiver_arg, remaining_args)) = args.split_first() else {
                    return false;
                };
                let receiver_expr = match receiver_arg {
                    hir::CallArg::Positional(expr) => expr,
                    hir::CallArg::Named { value, .. } => value,
                };
                let Some(kind) = self.facts.dispatch_target_kind(
                    self.source_path.as_path(),
                    span,
                    receiver_expr.ty,
                ) else {
                    return false;
                };
                (kind, fqn.as_str(), receiver_expr, remaining_args)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let Some(hir::MemberRef::Fun { fqn, .. }) = member.resolved.as_ref() else {
                    return false;
                };
                let Some(kind) =
                    self.facts
                        .dispatch_target_kind(self.source_path.as_path(), span, receiver.ty)
                else {
                    return false;
                };
                (kind, fqn.as_str(), receiver.as_ref(), args)
            }
            _ => return false,
        };

        let (dispatch_kind, callee_fqn, receiver_expr, call_args) = dispatch_target;
        let receiver_local = self.lower_expr_to_local(receiver_expr);
        if self.current_is_terminated() {
            return true;
        }
        let Some(args) = self.lower_call_args(call_args) else {
            return true;
        };
        let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        let Some((owner_fqn, member_name)) = callee_fqn.rsplit_once('.') else {
            self.assign(
                span,
                result,
                Rvalue::Todo("dispatch callee lowering pending"),
            );
            return true;
        };
        let member_decl_span = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)
            .filter(|binding| binding.fqn == callee_fqn)
            .map(|binding| binding.decl_span);
        let dispatch = DispatchMetadata {
            owner_fqn: owner_fqn.to_string(),
            member_name: member_name.to_string(),
            member_fqn: callee_fqn.to_string(),
            member_decl_span,
            receiver_ty,
        };
        let kind = match dispatch_kind {
            DispatchTargetKind::Virtual => CallKind::Virtual {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
            DispatchTargetKind::Interface => CallKind::Interface {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
        };
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        true
    }

    fn lower_refactor_dispatch_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> bool {
        let Some(dispatch_info) = self
            .facts
            .refactor_dispatch_contract(self.source_path.as_path(), span)
            .cloned()
        else {
            return false;
        };
        let (receiver_expr, call_args) = match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {
                let Some((receiver_arg, remaining_args)) = args.split_first() else {
                    return false;
                };
                let receiver_expr = match receiver_arg {
                    hir::CallArg::Positional(expr) => expr,
                    hir::CallArg::Named { value, .. } => value,
                };
                (receiver_expr, remaining_args)
            }
            hir::ExprKind::MemberAccess { receiver, .. } => (receiver.as_ref(), args),
            _ => return false,
        };

        let receiver_local = self.lower_expr_to_local(receiver_expr);
        if self.current_is_terminated() {
            return true;
        }
        let Some(args) = self.lower_call_args(call_args) else {
            return true;
        };
        let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        let dispatch = DispatchMetadata {
            owner_fqn: dispatch_info.owner_fqn,
            member_name: dispatch_info.member_name,
            member_fqn: dispatch_info.member_fqn,
            member_decl_span: dispatch_info.member_decl_span,
            receiver_ty: if receiver_ty == dispatch_info.receiver_ty {
                receiver_ty
            } else {
                dispatch_info.receiver_ty
            },
        };
        let kind = match dispatch_info.kind {
            DispatchTargetKind::Virtual => CallKind::Virtual {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
            DispatchTargetKind::Interface => CallKind::Interface {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
        };
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        true
    }

    fn capture_box_ty(&mut self, inner: TypeId) -> TypeId {
        self.types
            .intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: CAPTURE_BOX_FQN.to_string(),
                args: vec![inner],
                eff: None,
            })))
    }

    /// 降低一个 effect operation 调用（HIR `Perform`）到 MIR。
    ///
    /// 当前阶段会把 `perform` 同时显式化为：
    /// - 普通恢复后的 continuation block（`resume_target`）；
    /// - 若 outward propagation 需要先跑 cleanup，则通过 `UnwindAction::Cleanup` 连到 cleanup block；
    /// - 若当前无本地 cleanup，则用 `UnwindAction::Propagate` 明确表示“直接继续向外 unwind”。
    fn lower_perform_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        effect_ty: TypeId,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
    ) -> LocalId {
        let Some(lowered_args) = self.lower_call_args(args) else {
            return self.push_temp_local(span, ty);
        };

        if self.current_is_terminated() {
            // 实参 lowering 提前终止了 CFG：该 perform 永远不会发生。
            return self.push_temp_local(span, ty);
        }

        let Some((perform_args, mut metadata)) =
            self.canonicalize_perform_args(span, ty, lowered_args)
        else {
            let result = self.push_temp_local(span, ty);
            self.assign(
                span,
                result,
                Rvalue::PerformResult {
                    op_fqn: op.fqn.clone(),
                    effect_ty,
                },
            );
            let perform_args = Vec::new();
            let resume_target = self.push_block(span);
            let site_id = self.fresh_site_id();
            let unwind = self.build_perform_unwind_action(span);
            self.set_terminator_with_unwind(
                self.current_bb,
                span,
                TerminatorKind::Perform {
                    site_id,
                    op_fqn: String::new(),
                    metadata: PerformMetadata {
                        effect_ty,
                        result_ty: ty,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        payload_transport: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: perform_args,
                    resume_target,
                },
                unwind,
            );
            self.current_bb = resume_target;
            return result;
        };
        metadata.effect_ty = effect_ty;

        let result = self.push_temp_local(span, ty);
        self.assign(
            span,
            result,
            Rvalue::PerformResult {
                op_fqn: op.fqn.clone(),
                effect_ty,
            },
        );

        let resume_target = self.push_block(span);
        let site_id = self.fresh_site_id();
        let unwind = self.build_perform_unwind_action(span);
        self.set_terminator_with_unwind(
            self.current_bb,
            span,
            TerminatorKind::Perform {
                site_id,
                op_fqn: op.fqn.clone(),
                metadata,
                args: perform_args,
                resume_target,
            },
            unwind,
        );
        self.current_bb = resume_target;

        result
    }

    /// 降低一个 effect handler 表达式（HIR `Handle`）到 MIR。
    ///
    /// 当前阶段会把 `handle` 显式展开为 direct-style CFG：
    /// - 入口 block 以 `TerminatorKind::Handle` 指向 body/arms/finally/exit；
    /// - body 与 arm 正常完成后显式写回结果并跳向 `finally`/`exit_target`；
    /// - `finally` 自身作为 cleanup block 存在，`return` / `break` / `continue` 通过 cleanup chain
    ///   穿过它，而不是把这些续点留成 `Todo(...)`。
    fn lower_handle_expr(&mut self, span: Span, ty: TypeId, handle: &hir::HandleExpr) -> LocalId {
        let outer_bb = self.current_bb;

        let result = self.push_temp_local(span, ty);
        let refactor_handle_site = self
            .facts
            .refactor_handle_site_info(self.source_path.as_path(), span)
            .cloned();
        let handle_contract = if let Some(site) = refactor_handle_site {
            Some((site.metadata, site.arms))
        } else if self.facts.uses_refactor_typed_contracts() {
            None
        } else {
            Some(self.lower_handle_contract_from_hir(ty, handle))
        };
        let Some((metadata, mut arms)) = handle_contract else {
            let body_bb = self.push_block(handle.body.span);
            let exit_bb = self.push_block(span);
            let site_id = self.fresh_site_id();
            self.set_terminator(
                outer_bb,
                span,
                TerminatorKind::Handle {
                    site_id,
                    metadata: HandleMetadata {
                        result_ty: ty,
                        body_result_ty: handle.body.ty,
                        finally_result_ty: Some(ty),
                    },
                    arms: Vec::new(),
                    has_finally: false,
                    body_target: body_bb,
                    arm_targets: Vec::new(),
                    finally_target: None,
                    exit_target: exit_bb,
                },
            );
            self.current_bb = body_bb;
            self.set_terminator(body_bb, span, TerminatorKind::Goto { target: exit_bb });
            self.current_bb = exit_bb;
            return result;
        };
        for (hir_arm, lowered_arm) in handle.arms.iter().zip(arms.iter_mut()) {
            self.allocate_handle_arm_locals(hir_arm, lowered_arm);
        }

        let body_bb = self.push_block(handle.body.span);
        let arm_bbs = handle
            .arms
            .iter()
            .map(|arm| self.push_block(arm.span))
            .collect::<Vec<_>>();
        let finally_bb = handle
            .finally
            .as_ref()
            .map(|finally| self.push_cleanup_block(finally.span));
        let exit_bb = self.push_block(span);

        let site_id = self.fresh_site_id();
        self.set_terminator(
            outer_bb,
            span,
            TerminatorKind::Handle {
                site_id,
                metadata,
                arms: arms.clone(),
                has_finally: handle.finally.is_some(),
                body_target: body_bb,
                arm_targets: arm_bbs.clone(),
                finally_target: finally_bb,
                exit_target: exit_bb,
            },
        );

        let handle_cleanup_scope = handle
            .finally
            .as_ref()
            .cloned()
            .map(|finally| CleanupScope { finally });

        self.current_bb = body_bb;
        if let Some(scope) = handle_cleanup_scope.clone() {
            self.cleanup_scopes.push(scope);
        }
        let body_value = self.lower_block_as_expr(&handle.body);
        if handle_cleanup_scope.is_some() {
            let _ = self.cleanup_scopes.pop();
        }
        if !self.current_is_terminated() {
            self.assign_use_to_local(handle.body.span, result, Operand::Local(body_value));
            self.set_terminator(
                self.current_bb,
                handle.body.span,
                TerminatorKind::Goto {
                    target: finally_bb.unwrap_or(exit_bb),
                },
            );
        }

        for ((arm, lowered_arm), arm_bb) in handle.arms.iter().zip(arms.iter()).zip(arm_bbs) {
            self.current_bb = arm_bb;
            if let Some(scope) = handle_cleanup_scope.clone() {
                self.cleanup_scopes.push(scope);
            }
            let shadowed = self.bind_handle_arm_symbols(arm, lowered_arm);
            let arm_value = self.lower_expr_to_local(&arm.body);
            if handle_cleanup_scope.is_some() {
                let _ = self.cleanup_scopes.pop();
            }
            self.restore_shadowed_symbols(shadowed);
            if !self.current_is_terminated() {
                self.assign_use_to_local(arm.span, result, Operand::Local(arm_value));
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto {
                        target: finally_bb.unwrap_or(exit_bb),
                    },
                );
            }
        }

        if let Some((finally, finally_bb)) = handle.finally.as_ref().zip(finally_bb) {
            self.lower_cleanup_block_to_target(
                finally_bb,
                finally,
                exit_bb,
                self.cleanup_scopes.len(),
            );
        }

        self.current_bb = exit_bb;

        result
    }

    fn lower_handle_contract_from_hir(
        &mut self,
        result_ty: TypeId,
        handle: &hir::HandleExpr,
    ) -> (HandleMetadata, Vec<HandlerArm>) {
        let arms = handle
            .arms
            .iter()
            .map(|arm| {
                let payload_component_tys = arm
                    .op
                    .binders
                    .iter()
                    .map(|binder| binder.ty)
                    .collect::<Vec<_>>();
                let payload_tuple_ty = payload_tuple_ty_from_components(
                    self.types,
                    self.builtins.unit,
                    &payload_component_tys,
                );
                HandlerArm {
                    op_fqn: arm.op.op.fqn.clone(),
                    binder_count: arm.op.binders.len(),
                    handled_effect_ty: arm.op.effect_ty,
                    payload_tuple_ty,
                    binder_locals: Vec::new(),
                    continuation_local: None,
                    payload_component_tys,
                    body_ty: arm.body.ty,
                    kind: match arm.kind {
                        hir::HandleArmKind::NonResuming => HandlerArmKind::NonResuming,
                        hir::HandleArmKind::EscapeContinuation { .. } => {
                            HandlerArmKind::EscapeContinuation
                        }
                    },
                }
            })
            .collect();
        (
            HandleMetadata {
                result_ty,
                body_result_ty: handle.body.ty,
                finally_result_ty: handle.finally.as_ref().map(|finally| finally.ty),
            },
            arms,
        )
    }

    fn allocate_handle_arm_locals(&mut self, arm: &hir::HandleArm, lowered_arm: &mut HandlerArm) {
        lowered_arm.binder_locals = arm
            .op
            .binders
            .iter()
            .map(|binder| self.push_named_local(binder.span, &binder.name, binder.ty))
            .collect();
        lowered_arm.binder_count = lowered_arm.binder_locals.len();
        lowered_arm.continuation_local = match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                let ty = self
                    .infer_local_symbol_ty_in_expr(&arm.body, continuation)
                    .unwrap_or(self.builtins.any);
                Some(self.push_named_local(arm.span, "$continuation", ty))
            }
            hir::HandleArmKind::NonResuming => None,
        };
    }

    fn bind_handle_arm_symbols(
        &mut self,
        arm: &hir::HandleArm,
        lowered_arm: &HandlerArm,
    ) -> Vec<(hir::SymbolId, Option<LocalId>)> {
        let mut shadowed = Vec::with_capacity(
            lowered_arm.binder_locals.len() + usize::from(lowered_arm.continuation_local.is_some()),
        );
        for (binder, local) in arm
            .op
            .binders
            .iter()
            .zip(lowered_arm.binder_locals.iter().copied())
        {
            let previous = self.symbol_locals.insert(binder.id, local);
            shadowed.push((binder.id, previous));
        }
        if let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind
            && let Some(local) = lowered_arm.continuation_local
        {
            let previous = self.symbol_locals.insert(continuation, local);
            shadowed.push((continuation, previous));
        }
        shadowed
    }

    fn infer_local_symbol_ty_in_expr(
        &self,
        expr: &hir::Expr,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) if *id == symbol => {
                Some(expr.ty)
            }
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Todo(_) => None,
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| self.infer_local_symbol_ty_in_expr(&field.value, symbol)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .find_map(|element| self.infer_local_symbol_ty_in_expr(element, symbol)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                self.infer_local_symbol_ty_in_expr(expr, symbol)
            }),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::MemberAccess { receiver: expr, .. } => {
                self.infer_local_symbol_ty_in_expr(expr, symbol)
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => self
                .infer_local_symbol_ty_in_expr(lhs, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(rhs, symbol)),
            hir::ExprKind::Block(block) => block
                .stmts
                .iter()
                .find_map(|stmt| self.infer_local_symbol_ty_in_stmt(stmt, symbol)),
            hir::ExprKind::Call { callee, args } => self
                .infer_local_symbol_ty_in_expr(callee, symbol)
                .or_else(|| {
                    args.iter().find_map(|arg| match arg {
                        hir::CallArg::Positional(expr) => {
                            self.infer_local_symbol_ty_in_expr(expr, symbol)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.infer_local_symbol_ty_in_expr(value, symbol)
                        }
                    })
                }),
            hir::ExprKind::Closure(closure) => {
                self.infer_local_symbol_ty_in_expr(&closure.body, symbol)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self
                .infer_local_symbol_ty_in_expr(cond, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(then_branch, symbol))
                .or_else(|| {
                    else_branch
                        .as_ref()
                        .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol))
                }),
            hir::ExprKind::When { subject, arms } => self
                .infer_local_symbol_ty_in_expr(subject, symbol)
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(|guard| self.infer_local_symbol_ty_in_expr(guard, symbol))
                            .or_else(|| self.infer_local_symbol_ty_in_expr(&arm.body, symbol))
                    })
                }),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => self.infer_local_symbol_ty_in_expr(expr, symbol),
                hir::CallArg::Named { value, .. } => {
                    self.infer_local_symbol_ty_in_expr(value, symbol)
                }
            }),
            hir::ExprKind::Handle(handle) => self
                .infer_local_symbol_ty_in_block(&handle.body, symbol)
                .or_else(|| {
                    handle
                        .arms
                        .iter()
                        .find_map(|arm| self.infer_local_symbol_ty_in_expr(&arm.body, symbol))
                })
                .or_else(|| {
                    handle
                        .finally
                        .as_ref()
                        .and_then(|block| self.infer_local_symbol_ty_in_block(block, symbol))
                }),
        }
    }

    fn infer_local_symbol_ty_in_block(
        &self,
        block: &hir::Block,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        block
            .stmts
            .iter()
            .find_map(|stmt| self.infer_local_symbol_ty_in_stmt(stmt, symbol))
    }

    fn infer_local_symbol_ty_in_stmt(
        &self,
        stmt: &hir::Stmt,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
            hir::StmtKind::Expr(expr) => self.infer_local_symbol_ty_in_expr(expr, symbol),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol)),
            hir::StmtKind::Assign { lhs, rhs, .. } => self
                .infer_local_symbol_ty_in_expr(lhs, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(rhs, symbol)),
            hir::StmtKind::While { cond, body } => self
                .infer_local_symbol_ty_in_expr(cond, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_block(body, symbol)),
            hir::StmtKind::Return { value } => value
                .as_ref()
                .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol)),
        }
    }

    /// 降低字面量：把常量写入一个临时 local。
    fn lower_literal(&mut self, span: Span, ty: TypeId, lit: &hir::LiteralKind) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        let c = match lit {
            hir::LiteralKind::Bool(b) => ConstValue::Bool(*b),
            hir::LiteralKind::Char(_) => ConstValue::Char,
            hir::LiteralKind::Unit => ConstValue::Unit,
            hir::LiteralKind::Int => ConstValue::Int,
            hir::LiteralKind::SynthInt(value) => ConstValue::SynthInt(*value),
            hir::LiteralKind::Float64(_) => ConstValue::Float64,
            hir::LiteralKind::Float32(_) => ConstValue::Float32,
            hir::LiteralKind::String => ConstValue::String,
        };
        self.assign(span, tmp, Rvalue::Use(Operand::Const(c)));
        tmp
    }

    fn lower_class_literal_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        class_lit: &hir::ClassLiteralExpr,
    ) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        let kind = match class_lit.metadata_kind {
            hir::TypeMetadataLiteralKind::TypeNameString => TypeMetadataLiteralKind::TypeNameString,
        };
        self.assign(
            span,
            tmp,
            Rvalue::TypeMetadataLiteral(TypeMetadataLiteral {
                source_ty: class_lit.source_ty,
                source_fqn: class_lit.source_fqn.clone(),
                kind,
            }),
        );
        tmp
    }

    fn try_lower_compare_to_binary_expr(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> Option<LocalId> {
        let binding = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)?;
        let result = self.push_temp_local(span, result_ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return Some(result);
        }
        let rhs_local = self.lower_expr_to_local(rhs);
        if self.current_is_terminated() {
            return Some(result);
        }

        let compare_result = self.push_temp_local(span, self.builtins.int);
        let site_id = self.fresh_site_id();
        let kind = CallKind::Direct {
            callee_fqn: binding.fqn.clone(),
        };
        let args = vec![
            CallArg {
                span: lhs.span,
                name: None,
                value: Operand::Local(lhs_local),
            },
            CallArg {
                span: rhs.span,
                name: None,
                value: Operand::Local(rhs_local),
            },
        ];
        let transport = self.call_transport_metadata(self.builtins.int, &kind, &args, None);
        self.assign(
            span,
            compare_result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );

        let zero = self.push_temp_local(span, self.builtins.int);
        self.assign(
            span,
            zero,
            Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
        );
        self.assign(
            span,
            result,
            Rvalue::Binary {
                lhs: Operand::Local(compare_result),
                op,
                rhs: Operand::Local(zero),
            },
        );
        Some(result)
    }

    /// 降低变量引用：
    /// - 普通 local：直接返回其 local；
    /// - 被 capture 的 `var`（box 存储）：生成 `CaptureBoxGet` 并返回读取到的临时值 local；
    /// - 其它引用：降为 `Todo`。
    fn lower_var_ref(&mut self, span: Span, ty: TypeId, v: &hir::ValueRef) -> LocalId {
        match v {
            hir::ValueRef::Local { id, name, .. } => {
                let local = match self.symbol_locals.get(id).copied() {
                    Some(local) => local,
                    None => {
                        if let Some(member_local) =
                            self.lower_implicit_this_member_ref(span, ty, name)
                        {
                            return member_local;
                        }
                        panic!("typed HIR local reference must have an allocated MIR local: {id:?}")
                    }
                };

                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, ty);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxGet {
                            box_operand: Operand::Local(local),
                            contract: self.capture_box_contract(
                                self.body.locals[local.as_u32() as usize].ty,
                                ty,
                            ),
                        },
                    );
                    tmp
                } else {
                    local
                }
            }
            hir::ValueRef::TopLevel { .. } => {
                let hir::ValueRef::TopLevel { fqn, .. } = v else {
                    unreachable!("matched above");
                };
                let tmp = self.push_temp_local(span, ty);
                let hidden_effects = self.facts.top_level_ref_hidden_effects(fqn);
                let site_id = (!hidden_effects.is_pure()).then(|| self.fresh_site_id());
                self.assign(
                    span,
                    tmp,
                    Rvalue::TopLevelRef(TopLevelRef {
                        fqn: fqn.clone(),
                        site_id,
                        hidden_effects,
                    }),
                );
                tmp
            }
        }
    }

    fn lower_implicit_this_member_ref(
        &mut self,
        span: Span,
        ty: TypeId,
        member_name: &str,
    ) -> Option<LocalId> {
        let this_local = self
            .body
            .locals
            .iter()
            .enumerate()
            .find_map(|(idx, local)| {
                (local.name.as_deref() == Some("this")).then_some(LocalId::from_raw(idx as u32))
            })?;
        let receiver_ty = self.body.locals.get(this_local.as_u32() as usize)?.ty;
        let owner_fqn = self.owner_fqn.rsplit_once('.')?.0.to_string();
        let result = self.push_temp_local(span, ty);
        self.assign(
            span,
            result,
            Rvalue::MemberAccess {
                site_id: None,
                receiver: Operand::Local(this_local),
                member: MemberAccessMetadata {
                    name: member_name.to_string(),
                    receiver_ty,
                    resolved: Some(MemberTarget::Value {
                        fqn: format!("{owner_fqn}.{member_name}"),
                    }),
                    hidden_effects: EffectRow::pure(),
                },
            },
        );
        Some(result)
    }

    fn lower_closure_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        closure: &hir::ClosureExpr,
    ) -> LocalId {
        let name = format!("$lambda{}", closure.id.as_u32());
        let fqn = format!("{}.{}", self.owner_fqn, name);

        // 1) 计算 capture set，并决定 env 的 tuple 类型。
        let mut captures: Vec<ClosureCaptureLayout> = Vec::new();
        for cap in &closure.captures {
            let Some(source_local) = self.symbol_locals.get(&cap.id).copied() else {
                // 防御性：若当前函数未为该 symbol 分配 local（理论上不应发生），跳过该 capture。
                continue;
            };
            let source_ty = self.body.locals[source_local.as_u32() as usize].ty;
            captures.push(ClosureCaptureLayout {
                id: cap.id,
                name: cap.name.clone(),
                decl_span: cap.decl_span,
                ty: source_ty,
                mutable: cap.mutable,
                source_local,
            });
        }

        let (env_ty, env_operand) = if captures.is_empty() {
            (self.builtins.unit, Operand::Const(ConstValue::Unit))
        } else {
            let env_ty = self.types.ty_tuple(captures.iter().map(|c| c.ty).collect());
            let env_local = self.push_temp_local(span, env_ty);
            self.assign(
                span,
                env_local,
                Rvalue::MakeTuple {
                    elements: captures
                        .iter()
                        .map(|c| Operand::Local(c.source_local))
                        .collect(),
                    transport: self.aggregate_transport(
                        env_ty,
                        AggregateTransportKind::ClosureEnv,
                        captures
                            .iter()
                            .map(|c| (Some(c.name.clone()), c.ty))
                            .collect::<Vec<_>>(),
                    ),
                },
            );
            (env_ty, Operand::Local(env_local))
        };
        let env_contract = self.closure_env_contract(env_ty, &captures);

        let (fun, nested) = {
            let types = &mut *self.types;
            FnLowering::new(
                self.builtins,
                types,
                self.facts,
                self.top_level_fun_return_tys.clone(),
                self.top_level_fun_param_tys.clone(),
                fqn.clone(),
                self.source_path.clone(),
            )
            .lower_closure_fun(fqn.clone(), name, closure, env_ty, &captures)
        };
        self.nested_funs.push(fun);
        self.nested_funs.extend(nested);

        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::MakeClosure {
                env: env_operand,
                fn_ptr: fqn,
                env_contract,
            },
        );
        tmp
    }

    fn lower_closure_fun(
        mut self,
        closure_fqn: String,
        closure_name: String,
        closure: &hir::ClosureExpr,
        env_ty: TypeId,
        captures: &[ClosureCaptureLayout],
    ) -> (FunDecl, Vec<FunDecl>) {
        self.current_return_ty = closure.body.ty;
        // 0) 预扫描 closure body：本 closure 内部若存在嵌套 closure 捕获 `var`，则需要 box 存储（T0714）。
        self.boxed_symbols = boxed_symbols_in_expr(closure.body.as_ref());

        // 1) 创建入口块。
        let entry = self.push_block(closure.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) env + captures + 参数变为 locals。
        let mut params = Vec::with_capacity(closure.params.len() + 1);

        let env_local = self.push_named_local(closure.span, "$env", env_ty);
        params.push(Param {
            span: closure.span,
            name: "$env".to_string(),
            ty: env_ty,
            local: env_local,
        });

        // 把捕获字段从 `$env` 解包到局部 local，并写入 SymbolId → LocalId 映射，使得后续 body lowering
        // 可以像普通局部变量一样引用它们。
        for (idx, cap) in captures.iter().enumerate() {
            let local = self.push_named_local(cap.decl_span, &cap.name, cap.ty);
            self.symbol_locals.insert(cap.id, local);
            if cap.mutable {
                self.boxed_symbols.insert(cap.id);
            }
            self.assign(
                cap.decl_span,
                local,
                Rvalue::TupleGet {
                    tuple: Operand::Local(env_local),
                    index: idx,
                },
            );
        }

        for p in &closure.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower lambda body. A closure body is an expression, so its value is the callable
        // result unless the body already terminated through an explicit control-flow edge.
        let body_result = self.lower_expr_to_local(closure.body.as_ref());
        if !self.current_is_terminated() {
            let value =
                self.operand_for_current_return_ty(closure.span, Operand::Local(body_result));
            self.set_terminator(
                self.current_bb,
                closure.span,
                TerminatorKind::Return { value: Some(value) },
            );
        }

        let out = FunDecl {
            span: closure.span,
            fqn: closure_fqn,
            name: closure_name,
            ty: self.builtins.any,
            params,
            return_ty: closure.body.ty,
            body: Some(self.body),
        };

        (out, self.nested_funs)
    }

    /// 降低 `if` 表达式：生成 then/else/merge 基本块，并在 merge 点写回一个临时结果 local。
    fn lower_if_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        cond: &hir::Expr,
        then_branch: &hir::Expr,
        else_branch: Option<&hir::Expr>,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值条件，并以 CondBr 结束当前块。
        let cond_local = self.lower_expr_to_local(cond);
        let parent = self.current_bb;
        let then_bb = self.push_block(then_branch.span);
        let else_bb = self.push_block(else_branch.map(|e| e.span).unwrap_or(span));
        let merge_bb = self.push_block(span);

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(cond_local),
                then_target: then_bb,
                else_target: else_bb,
            },
        );

        // 2) then 分支：lower 表达式并写回 result，然后跳到 merge。
        self.current_bb = then_bb;
        let then_value = self.lower_expr_to_local(then_branch);
        if !self.current_is_terminated() {
            self.assign_use_to_local(then_branch.span, result, Operand::Local(then_value));
            self.set_terminator(
                self.current_bb,
                then_branch.span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 3) else 分支：同上；若缺省 else，则使用 Unit 占位。
        self.current_bb = else_bb;
        let else_value = else_branch
            .map(|e| self.lower_expr_to_local(e))
            .unwrap_or_else(|| self.emit_unit(span));
        if !self.current_is_terminated() {
            self.assign_use_to_local(span, result, Operand::Local(else_value));
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 4) merge：后续语句继续在 merge 块中生成。
        self.current_bb = merge_bb;
        result
    }

    fn lower_pattern(&self, pat: &hir::WhenPat, subject_ty: TypeId) -> Pattern {
        match pat {
            hir::WhenPat::Else { .. } => Pattern::Else,
            hir::WhenPat::Or { pats, .. } => Pattern::Or {
                pats: pats
                    .iter()
                    .map(|pat| self.lower_pattern(pat, subject_ty))
                    .collect(),
            },
            hir::WhenPat::Wildcard { .. } => Pattern::Wildcard,
            hir::WhenPat::Rest { .. } => Pattern::Rest,
            hir::WhenPat::Is { ty, .. } => Pattern::Is {
                ty: *ty,
                metadata: self.runtime_pattern_type_test_metadata(subject_ty, *ty),
            },
            hir::WhenPat::Bind { span, name, .. } => Pattern::Bind {
                name: name.clone(),
                ty: self
                    .facts
                    .when_pat_binding_ty(*span)
                    .unwrap_or(self.builtins.any),
            },
            hir::WhenPat::Tuple { elements, .. } => Pattern::Tuple {
                elements: elements
                    .iter()
                    .enumerate()
                    .map(|(index, pat)| {
                        let element_ty = self.tuple_pattern_element_ty(subject_ty, index);
                        self.lower_pattern(pat, element_ty)
                    })
                    .collect(),
            },
            hir::WhenPat::Variant { name, args, .. } => Pattern::Variant {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|pat| self.lower_pattern(pat, self.builtins.any))
                    .collect(),
            },
            hir::WhenPat::IntLit { raw, .. } => Pattern::IntLit { raw: raw.clone() },
            hir::WhenPat::CharLit { value, .. } => Pattern::CharLit { value: *value },
            hir::WhenPat::StringLit { value, .. } => Pattern::StringLit {
                value: value.clone(),
            },
            hir::WhenPat::BoolLit { value, .. } => Pattern::BoolLit { value: *value },
        }
    }

    fn tuple_pattern_element_ty(&self, subject_ty: TypeId, index: usize) -> TypeId {
        match self.types.kind(subject_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                elements.get(index).copied().unwrap_or(self.builtins.any)
            }
            _ => self.builtins.any,
        }
    }

    fn when_pat_is_irrefutable(&self, pat: &hir::WhenPat) -> bool {
        matches!(
            pat,
            hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. } | hir::WhenPat::Bind { .. }
        )
    }

    fn collect_when_pattern_bindings(
        &self,
        pat: &hir::WhenPat,
        path: &mut Vec<PatternBindingStep>,
        out: &mut Vec<WhenPatternBinding>,
    ) {
        match pat {
            hir::WhenPat::Bind { span, id, name } => {
                out.push(WhenPatternBinding {
                    id: *id,
                    span: *span,
                    name: name.clone(),
                    ty: self
                        .facts
                        .when_pat_binding_ty(*span)
                        .unwrap_or(self.builtins.any),
                    path: path.clone(),
                });
            }
            hir::WhenPat::Tuple { elements, .. } => {
                for (index, element) in elements.iter().enumerate() {
                    path.push(PatternBindingStep::TupleIndex(index));
                    self.collect_when_pattern_bindings(element, path, out);
                    let _ = path.pop();
                }
            }
            hir::WhenPat::Variant { name, args, .. } => {
                for (field_index, arg) in args.iter().enumerate() {
                    if matches!(arg, hir::WhenPat::Rest { .. }) {
                        continue;
                    }
                    path.push(PatternBindingStep::VariantField {
                        variant: name.clone(),
                        field_index,
                    });
                    self.collect_when_pattern_bindings(arg, path, out);
                    let _ = path.pop();
                }
            }
            hir::WhenPat::Or { pats, .. } => {
                for pat in pats {
                    self.collect_when_pattern_bindings(pat, path, out);
                }
            }
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::CharLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => {}
        }
    }

    fn bind_when_pattern_locals(
        &mut self,
        subject_local: LocalId,
        pat: &hir::WhenPat,
    ) -> Vec<(hir::SymbolId, Option<LocalId>)> {
        let mut bindings = Vec::new();
        self.collect_when_pattern_bindings(pat, &mut Vec::new(), &mut bindings);

        let mut shadowed = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let local = self.push_named_local(binding.span, &binding.name, binding.ty);
            self.assign(
                binding.span,
                local,
                Rvalue::PatternExtract {
                    subject: Operand::Local(subject_local),
                    path: binding.path,
                },
            );
            let previous = self.symbol_locals.insert(binding.id, local);
            shadowed.push((binding.id, previous));
        }
        shadowed
    }

    fn restore_shadowed_symbols(&mut self, shadowed: Vec<(hir::SymbolId, Option<LocalId>)>) {
        for (id, previous) in shadowed.into_iter().rev() {
            match previous {
                Some(local) => {
                    self.symbol_locals.insert(id, local);
                }
                None => {
                    self.symbol_locals.remove(&id);
                }
            }
        }
    }

    /// 降低 `when` 表达式：把每个 arm 降为显式 pattern test / binder extract / guard CFG。
    fn lower_when_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值 subject。
        let subject_local = self.lower_expr_to_local(subject);
        if self.current_is_terminated() {
            return result;
        }

        // 2) 构造 merge block，并从当前块开始链式生成“匹配测试块”。
        let merge_bb = self.push_block(span);
        let mut test_bb = self.current_bb;

        for arm in arms {
            let irrefutable = self.when_pat_is_irrefutable(&arm.pat);
            let needs_next_test_bb = !irrefutable || arm.guard.is_some();
            let body_bb = arm.guard.as_ref().map(|_| self.push_block(arm.span));
            let next_test_bb = needs_next_test_bb.then(|| self.push_block(arm.span));
            let match_bb = if irrefutable {
                self.current_bb = test_bb;
                let match_bb = self.push_block(arm.span);
                self.set_terminator(test_bb, arm.span, TerminatorKind::Goto { target: match_bb });
                match_bb
            } else {
                let match_bb = self.push_block(arm.span);
                self.current_bb = test_bb;
                let cond = self.push_temp_local(arm.span, self.builtins.bool_);
                self.assign(
                    arm.pat.span(),
                    cond,
                    Rvalue::PatternMatch {
                        subject: Operand::Local(subject_local),
                        pattern: self.lower_pattern(&arm.pat, subject.ty),
                    },
                );
                self.set_terminator(
                    test_bb,
                    arm.span,
                    TerminatorKind::CondBr {
                        cond: Operand::Local(cond),
                        then_target: match_bb,
                        else_target: next_test_bb
                            .expect("refutable when arm should allocate next test block"),
                    },
                );
                match_bb
            };

            self.current_bb = match_bb;
            let shadowed = self.bind_when_pattern_locals(subject_local, &arm.pat);
            if let Some(guard) = &arm.guard {
                let guard_local = self.lower_expr_to_local(guard);
                if !self.current_is_terminated() {
                    self.set_terminator(
                        self.current_bb,
                        guard.span,
                        TerminatorKind::CondBr {
                            cond: Operand::Local(guard_local),
                            then_target: body_bb
                                .expect("guarded when arm should allocate body block"),
                            else_target: next_test_bb
                                .expect("guarded when arm should allocate next test block"),
                        },
                    );
                }
                self.current_bb = body_bb.expect("guarded when arm should allocate body block");
            }

            let body_value = self.lower_expr_to_local(&arm.body);
            if !self.current_is_terminated() {
                self.assign_use_to_local(arm.span, result, Operand::Local(body_value));
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto { target: merge_bb },
                );
            }
            self.restore_shadowed_symbols(shadowed);

            // 继续下一个 arm 的测试块。
            if irrefutable && arm.guard.is_none() {
                self.current_bb = merge_bb;
                return result;
            }

            test_bb = next_test_bb.expect("fallthrough when arm should allocate next test block");
            self.current_bb = test_bb;
        }

        // 若没有兜底 arm，当前阶段以 `unreachable` 收束。
        self.set_terminator(test_bb, span, TerminatorKind::Unreachable);
        self.current_bb = merge_bb;
        result
    }
}

fn parse_tuple_member_index(text: &str) -> Option<usize> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn payload_tuple_ty_from_components(
    types: &mut TypeStore,
    unit_ty: TypeId,
    components: &[TypeId],
) -> Option<TypeId> {
    match components {
        [] => Some(unit_ty),
        [single] => Some(*single),
        _ => Some(types.ty_tuple(components.to_vec())),
    }
}

fn continuation_identity_return_param(types: &TypeStore, fun: &hir::FunDecl) -> Option<usize> {
    continuation_contract_from_type(types, fun.return_ty)?;
    let returned = block_identity_return_expr(fun.body.as_ref()?)?;
    let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &returned.kind else {
        return None;
    };
    let param_index = fun.params.iter().position(|param| param.id == *id)?;
    continuation_contract_from_type(types, fun.params[param_index].ty)?;
    Some(param_index)
}

fn block_identity_return_expr(block: &hir::Block) -> Option<&hir::Expr> {
    let [stmt] = block.stmts.as_slice() else {
        return None;
    };
    match &stmt.kind {
        hir::StmtKind::Return { value: Some(value) } | hir::StmtKind::Expr(value) => Some(value),
        _ => None,
    }
}

fn continuation_contract_from_type(
    types: &TypeStore,
    continuation_ty: TypeId,
) -> Option<(TypeId, TypeId, EffectRow)> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(continuation_ty) else {
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

fn find_raise_runtime_error_effect(types: &TypeStore) -> Option<TypeId> {
    let runtime_error_ty = find_runtime_error_type(types)?;
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Raise"
                    && nominal.args.as_slice() == [runtime_error_ty]
        )
    })
}

fn find_runtime_error_type(types: &TypeStore) -> Option<TypeId> {
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.RuntimeError"
        ) || matches!(
            types.kind(id),
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    })
}

fn boxed_symbols_in_block(block: &hir::Block) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_block(block, &mut out);
    out
}

fn boxed_symbols_in_expr(expr: &hir::Expr) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_expr(expr, &mut out);
    out
}

fn collect_boxed_symbols_in_block(block: &hir::Block, out: &mut HashSet<hir::SymbolId>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => collect_boxed_symbols_in_expr(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = &decl.init {
                    collect_boxed_symbols_in_expr(init, out);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                collect_boxed_symbols_in_expr(lhs, out);
                collect_boxed_symbols_in_expr(rhs, out);
            }
            hir::StmtKind::While { cond, body } => {
                collect_boxed_symbols_in_expr(cond, out);
                collect_boxed_symbols_in_block(body, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_boxed_symbols_in_expr(v, out);
                }
            }
        }
    }
}

fn collect_boxed_symbols_in_expr(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_boxed_symbols_in_expr(&f.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = p {
                    collect_boxed_symbols_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr, .. } => collect_boxed_symbols_in_expr(expr.as_ref(), out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_boxed_symbols_in_expr(lhs.as_ref(), out);
            collect_boxed_symbols_in_expr(rhs.as_ref(), out);
        }
        hir::ExprKind::TypeCheck { expr, .. } | hir::ExprKind::Cast { expr, .. } => {
            collect_boxed_symbols_in_expr(expr.as_ref(), out);
        }
        hir::ExprKind::Block(block) => collect_boxed_symbols_in_block(block, out),
        hir::ExprKind::Closure(closure) => {
            for cap in &closure.captures {
                if cap.mutable {
                    out.insert(cap.id);
                }
            }
            collect_boxed_symbols_in_expr(closure.body.as_ref(), out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_boxed_symbols_in_expr(cond, out);
            collect_boxed_symbols_in_expr(then_branch, out);
            if let Some(e) = else_branch.as_deref() {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_boxed_symbols_in_expr(subject, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_boxed_symbols_in_expr(g, out);
                }
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_boxed_symbols_in_expr(receiver, out)
        }
        hir::ExprKind::Call { callee, args } => {
            collect_boxed_symbols_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            collect_boxed_symbols_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
            if let Some(finally) = &handle.finally {
                collect_boxed_symbols_in_block(finally, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_refactor_pipeline::TypedHirEffectContracts;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use std::path::PathBuf;

    #[test]
    fn refactor_typed_contracts_clear_legacy_resume_and_perform_fallbacks() {
        let span = Span::new(1, 2);
        let legacy_effect_sites = std::iter::once((
            hir::CallSite::new(PathBuf::from("fixtures/mir_lower_facts.scoop"), span),
            hir::EffectOpCallInfo {
                arg_mapping: vec![0],
                payload_tuple_ty: None,
            },
        ))
        .collect::<hir::EffectOpCallSiteIndex>();
        let dispatch_sites = hir::DispatchCallSiteIndex::default();
        let when_pat_binding_tys = hir::WhenPatBindingTypeIndex::default();
        let top_level_fun_call_sites = hir::TopLevelFunCallSiteIndex::default();

        let facts = MirLoweringFacts::from_hir_side_tables_and_resume_spans(
            &dispatch_sites,
            [span],
            [span],
            &legacy_effect_sites,
            &when_pat_binding_tys,
            &top_level_fun_call_sites,
        )
        .with_refactor_typed_contracts(&TypedHirEffectContracts::default());

        assert!(facts.uses_refactor_typed_contracts());
        assert!(!facts.legacy_resume_site_matches(span));
        assert!(!facts.legacy_resume_site_suspends_outward(span));
        assert!(facts.legacy_perform_site_info(span).is_none());
    }

    #[test]
    fn dump_mir_emits_type_body_generic_member_fun_roots() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_member_root_generic.scoop",
            r#"
package fixtures.mirlower

class Box() {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun_fqns = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            fun_fqns.contains(&"fixtures.mirlower.Box.forward"),
            "generic MIR lowering 应显式发射 type-body generic member fun root"
        );
        assert!(
            fun_fqns.contains(&"fixtures.mirlower.wrap"),
            "顶层 generic fun root 仍应继续保留"
        );
    }

    #[test]
    fn dump_mir_emits_companion_generic_member_fun_roots() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_companion_member_root_generic.scoop",
            r#"
package fixtures.mirlower

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
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun_fqns = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            fun_fqns.contains(&"fixtures.mirlower.Box.Companion.forward"),
            "generic MIR lowering 应显式发射 companion generic member fun root"
        );
        assert!(
            fun_fqns.contains(&"fixtures.mirlower.wrap"),
            "顶层 generic fun root 仍应继续保留"
        );
    }

    #[test]
    fn dump_mir_types_comparison_condition_as_bool_in_generic_template() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_generic_compare_bool.scoop",
            r#"
package fixtures.mirlower

fun repeat<T>(x: T, n: Int): T {
    if (n <= 0) {
        return x
    }
    return repeat(x, n - 1)
}
"#,
        );

        let mut lowered = lower_for_dump(&sess, &source).unwrap();
        let builtins = lowered.types.intern_builtins();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mirlower.repeat" => Some(fun),
                _ => None,
            })
            .expect("expected generic repeat MIR root");
        let body = fun.body.as_ref().expect("repeat should have a MIR body");
        let TerminatorKind::CondBr { cond, .. } =
            &body.blocks[body.start.as_usize()].terminator.kind
        else {
            panic!("expected repeat entry block to branch on comparison");
        };
        let Operand::Local(cond_local) = cond else {
            panic!("comparison condition should be stored in a local");
        };
        let cond_ty = body.locals[cond_local.as_u32() as usize].ty;

        assert_eq!(
            cond_ty, builtins.bool_,
            "MIR comparison result local should be Bool, not an overly broad fallback type"
        );
    }

    #[test]
    fn dump_mir_lowers_user_defined_compare_to_as_direct_call_plus_zero_compare() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_compare_to_direct_call.scoop",
            r#"
package fixtures.mirlower

struct Num(val value: Int) {
    fun compareTo(other: Num): Int {
        return this.value - other.value
    }
}

fun entry(lhs: Num, rhs: Num): Bool {
    return lhs < rhs
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
                _ => None,
            })
            .expect("expected entry MIR root");
        let body = fun.body.as_ref().expect("entry should have a MIR body");
        let entry_block = &body.blocks[body.start.as_usize()];

        assert!(
            entry_block.stmts.iter().any(|stmt| matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        args,
                        ..
                    },
                    ..
                } if callee_fqn == "fixtures.mirlower.Num.compareTo" && args.len() == 2
            )),
            "generic MIR compareTo lowering 应显式发射 direct-call target"
        );
        assert!(
            entry_block.stmts.iter().any(|stmt| matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Binary { rhs: Operand::Local(local), .. },
                    ..
                } if matches!(
                    body.locals.get(local.as_u32() as usize),
                    Some(LocalDecl { .. })
                )
            )),
            "compareTo direct-call 结果仍应继续进入普通 MIR Binary 比较主线"
        );
        assert!(
            entry_block.stmts.iter().any(|stmt| matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
                    ..
                }
            )),
            "compareTo → 0 比较应在 MIR 中保留显式的合成整数常量"
        );
    }

    #[test]
    fn dump_mir_lowers_compare_to_in_if_condition_as_direct_call() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_compare_to_if_condition.scoop",
            r#"
package fixtures.mirlower

struct Num(val value: Int) {
    fun compareTo(other: Num): Int {
        return this.value - other.value
    }
}

fun entry(lhs: Num, rhs: Num): Int {
    if (lhs < rhs) {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
                _ => None,
            })
            .expect("expected entry MIR root");
        let body = fun.body.as_ref().expect("entry should have a MIR body");
        let direct_call_stmt = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                        ..
                    } if callee_fqn == "fixtures.mirlower.Num.compareTo" && args.len() == 2
                )
            });
        assert!(
            direct_call_stmt.is_some(),
            "if 条件里的 compareTo 比较也应显式发射 direct-call target"
        );
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
                        ..
                    }
                )),
            "if 条件里的 compareTo → 0 比较应保留显式 SynthInt(0)"
        );
    }

    #[test]
    fn typed_hir_fixture_preserves_compare_to_direct_call_binding() {
        let sess = Session::new().unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/operator_overload_struct_basic.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();

        let lowered = crate::hir::lower_typed_for_dump(&sess, &source).unwrap();
        assert!(
            lowered
                .top_level_fun_call_sites
                .values()
                .any(|binding| binding.fqn == "Num.compareTo"),
            "typed HIR side table 应保留 fixture compareTo 站点的 direct-call binding"
        );
    }

    #[test]
    fn dump_mir_canonicalizes_callable_receiver_named_args_by_binding() {
        let sess = Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "main" => Some(fun),
                _ => None,
            })
            .expect("expected main MIR root");
        let body = fun.body.as_ref().expect("main should have a MIR body");

        let call_args_at = |span: Span| {
            body.blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .find_map(|stmt| match &stmt.kind {
                    StatementKind::Assign {
                        value: Rvalue::Call { args, .. },
                        ..
                    } if stmt.span == span => Some(args.as_slice()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing call at span {span:?}"))
        };

        let when_fun_value_args = call_args_at(Span::new(1442, 1469));
        assert_eq!(when_fun_value_args.len(), 2);
        assert_eq!(when_fun_value_args[0].span, Span::new(1463, 1468));
        assert_eq!(when_fun_value_args[1].span, Span::new(1449, 1450));

        let top_funptr_args = call_args_at(Span::new(1770, 1797));
        assert_eq!(top_funptr_args.len(), 2);
        assert_eq!(top_funptr_args[0].span, Span::new(1795, 1796));
        assert_eq!(top_funptr_args[1].span, Span::new(1781, 1782));
    }

    #[test]
    fn dump_mir_publishes_member_write_contract_for_escape_continuation_cell() {
        let sess = Session::new().unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "main" => Some(fun),
                _ => None,
            })
            .expect("expected main MIR root");
        let body = fun.body.as_ref().expect("main should have a MIR body");

        assert!(
            body.blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .all(|stmt| !matches!(
                    stmt.kind,
                    StatementKind::Todo("assign lhs lowering pending")
                )),
            "member writes should no longer fall back to assign lhs TODO"
        );

        let mut saw_some_k_write = false;
        let mut saw_none_write = false;
        for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
            let StatementKind::StoreMember {
                member,
                continuation_route,
                ..
            } = &stmt.kind
            else {
                continue;
            };
            let Some(MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
                continue;
            };
            if fqn != "Cell.k" {
                continue;
            }
            match continuation_route {
                StoredContinuationRoutePublication::Unique(route)
                    if matches!(
                        route.path.as_slice(),
                        [PatternBindingStep::VariantField {
                            variant,
                            field_index: 0,
                        }] if variant == "Some"
                    ) =>
                {
                    saw_some_k_write = true;
                }
                StoredContinuationRoutePublication::None => {
                    saw_none_write = true;
                }
                StoredContinuationRoutePublication::Ambiguous
                | StoredContinuationRoutePublication::Unique(_) => {}
            }
        }

        assert!(
            saw_some_k_write,
            "cell.k = Some(k) 应发布 wrapper path + source local 的 continuation write contract"
        );
        assert!(
            saw_none_write,
            "cell.k = none_k 应发布显式 member write contract，而不是 TODO"
        );
    }
}
