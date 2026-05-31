//! MirLoweringFacts impl: lowering side-table accumulation.

#![allow(dead_code)]

use super::*;

impl MirLoweringFacts {
    pub fn from_hir_facts(lowered: &hir::LoweredHir, hir_facts: &HirFacts) -> Self {
        let mut facts = Self::default();
        facts = facts
            .with_source_site_facts(hir_facts)
            .with_declaration_facts(&lowered.types, hir_facts)
            .with_continuation_identity_return_funs(lowered)
            .with_class_ctor_hidden_effects(lowered);
        facts
    }

    pub(in crate::mir::lower) fn with_declaration_facts(
        mut self,
        types: &TypeStore,
        hir_facts: &HirFacts,
    ) -> Self {
        let facts = HirFactResolver::new(types, hir_facts);
        self.member_value_tys.extend(facts.member_value_tys());
        self.nominal_kinds.extend(facts.nominal_kinds());
        self.enum_has_payload.extend(facts.enum_payload_kinds());
        let mut variant_owners: HashMap<String, Option<String>> = HashMap::new();
        for variant in &hir_facts.declarations.enum_variants {
            let owner = variant.enum_owner.as_str().to_string();
            variant_owners
                .entry(variant.name.clone())
                .and_modify(|existing| {
                    if existing.as_ref() != Some(&owner) {
                        *existing = None;
                    }
                })
                .or_insert(Some(owner));
        }
        self.enum_variant_owner_fqns.extend(
            variant_owners
                .into_iter()
                .filter_map(|(name, owner)| owner.map(|owner| (name, owner))),
        );

        self
    }

    pub(in crate::mir::lower) fn with_source_site_facts(mut self, hir_facts: &HirFacts) -> Self {
        self.resume_sites.clear();
        self.perform_sites.clear();
        self.handle_sites.clear();
        self.call_sites.clear();
        self.assign_places.clear();
        self.call_arg_bindings.clear();
        self.class_ctor_call_sites.clear();
        self.top_level_fun_call_fqns.clear();
        self.when_pat_binding_tys.clear();

        for fact in &hir_facts.source_sites.call_sites {
            let call_site = hir_call_site(&fact.identity);
            match &fact.contract {
                hir_site_facts::CallSiteContractKind::Constructor(ctor) => {
                    self.class_ctor_call_sites
                        .insert(call_site.clone(), ctor_call_info_from_fact(ctor));
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::Constructor(constructor_contract_from_fact(ctor)),
                    );
                }
                hir_site_facts::CallSiteContractKind::Virtual(member) => {
                    self.dispatch_call_sites.insert(
                        hir::DispatchCallSite::new(
                            fact.identity.source_path.clone(),
                            fact.identity.span,
                            member.receiver_ty,
                        ),
                        DispatchTargetKind::Virtual,
                    );
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::Virtual(member_contract_from_fact(member)),
                    );
                }
                hir_site_facts::CallSiteContractKind::Interface(member) => {
                    self.dispatch_call_sites.insert(
                        hir::DispatchCallSite::new(
                            fact.identity.source_path.clone(),
                            fact.identity.span,
                            member.receiver_ty,
                        ),
                        DispatchTargetKind::Interface,
                    );
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::Interface(member_contract_from_fact(member)),
                    );
                }
                hir_site_facts::CallSiteContractKind::DirectTopLevel(function) => {
                    self.top_level_fun_call_fqns
                        .insert(call_site.clone(), function.fqn.clone());
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::DirectTopLevel(function_contract_from_fact(
                            function,
                        )),
                    );
                }
                hir_site_facts::CallSiteContractKind::MemberDirect(member) => {
                    self.top_level_fun_call_fqns
                        .insert(call_site.clone(), member.function.fqn.clone());
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::MemberDirect(member_contract_from_fact(member)),
                    );
                }
                hir_site_facts::CallSiteContractKind::Extension {
                    receiver_ty,
                    function,
                } => {
                    self.top_level_fun_call_fqns
                        .insert(call_site.clone(), function.fqn.clone());
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::Extension {
                            receiver_ty: *receiver_ty,
                            function: function_contract_from_fact(function),
                        },
                    );
                }
                hir_site_facts::CallSiteContractKind::Intrinsic { kind, function } => {
                    self.top_level_fun_call_fqns
                        .insert(call_site.clone(), function.fqn.clone());
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::Intrinsic {
                            kind: intrinsic_kind_from_fact(kind),
                            function: function_contract_from_fact(function),
                        },
                    );
                }
                hir_site_facts::CallSiteContractKind::Closure {
                    callee_ty,
                    return_ty,
                    abi,
                    arg_binding,
                } => {
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::Closure {
                            callee_ty: *callee_ty,
                            return_ty: *return_ty,
                            abi_identity: callable_abi_from_fact(*abi),
                            arg_binding: arg_binding.as_ref().map(call_arg_binding_from_fact),
                        },
                    );
                }
                hir_site_facts::CallSiteContractKind::FunValue {
                    callee_ty,
                    return_ty,
                    abi,
                    arg_binding,
                } => {
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::FunValue {
                            callee_ty: *callee_ty,
                            return_ty: *return_ty,
                            abi_identity: callable_abi_from_fact(*abi),
                            arg_binding: arg_binding.as_ref().map(call_arg_binding_from_fact),
                        },
                    );
                }
                hir_site_facts::CallSiteContractKind::FunPtr {
                    callee_ty,
                    return_ty,
                    abi,
                    arg_binding,
                } => {
                    self.call_sites.insert(
                        call_site.clone(),
                        TypedCallSiteContract::FunPtr {
                            callee_ty: *callee_ty,
                            return_ty: *return_ty,
                            abi_identity: callable_abi_from_fact(*abi),
                            arg_binding: arg_binding.as_ref().map(call_arg_binding_from_fact),
                        },
                    );
                }
                hir_site_facts::CallSiteContractKind::EffectOp(_)
                | hir_site_facts::CallSiteContractKind::ContinuationResume(_) => {}
            }
        }

        for fact in &hir_facts.source_sites.argument_bindings {
            self.call_arg_bindings.insert(
                hir_call_site(&fact.identity),
                call_arg_binding_from_fact(&fact.binding),
            );
        }
        for fact in &hir_facts.source_sites.continuation_resumes {
            self.resume_sites.insert(
                hir_call_site(&fact.identity),
                resume_call_info_from_fact(fact),
            );
        }
        for fact in &hir_facts.source_sites.perform_sites {
            self.perform_sites.insert(
                hir_call_site(&fact.identity),
                perform_metadata_from_fact(fact),
            );
        }
        for fact in &hir_facts.source_sites.handle_sites {
            self.handle_sites.insert(
                hir_call_site(&fact.identity),
                handle_site_info_from_fact(fact),
            );
        }
        for fact in &hir_facts.source_sites.assignments {
            self.assign_places.insert(
                hir_call_site(&fact.identity),
                assign_place_contract_from_fact(fact),
            );
        }
        for fact in &hir_facts.source_sites.pattern_bindings {
            self.when_pat_binding_tys
                .insert(fact.identity.span, fact.binding_ty);
        }
        self.top_level_init_roots = hir_facts
            .source_sites
            .top_level_init_roots
            .iter()
            .map(top_level_init_root_from_fact)
            .collect();
        self.extern_global_contracts = hir_facts
            .source_sites
            .extern_globals
            .iter()
            .map(extern_global_contract_from_fact)
            .collect();

        self
    }

    pub(in crate::mir::lower) fn with_continuation_identity_return_funs(
        mut self,
        lowered: &hir::LoweredHir,
    ) -> Self {
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

    pub(in crate::mir::lower) fn with_class_ctor_hidden_effects(
        mut self,
        lowered: &hir::LoweredHir,
    ) -> Self {
        let analyzer = HiddenInitEffectAnalyzer::new(lowered);
        for (site, info) in &self.class_ctor_call_sites {
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

    pub(in crate::mir::lower) fn nominal_kind(&self, fqn: &str) -> Option<ast::TypeKind> {
        self.nominal_kinds.get(fqn).copied()
    }

    pub(in crate::mir::lower) fn enum_has_payload(&self, fqn: &str) -> Option<bool> {
        self.enum_has_payload.get(fqn).copied()
    }

    pub(in crate::mir::lower) fn enum_variant_owner_fqn(&self, name: &str) -> Option<&str> {
        self.enum_variant_owner_fqns.get(name).map(String::as_str)
    }

    pub(in crate::mir::lower) fn dispatch_target_kind(
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

    pub(in crate::mir::lower) fn dispatch_site_kind_for_call(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Option<DispatchTargetKind> {
        let receiver_ty = match &callee.kind {
            hir::ExprKind::MemberAccess { receiver, .. } => Some(receiver.ty),
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {
                args.first().map(call_arg_expr).map(|receiver| receiver.ty)
            }
            _ => None,
        }?;
        self.dispatch_target_kind(source_path, call_span, receiver_ty)
    }

    pub(in crate::mir::lower) fn assign_place_contract(
        &self,
        source_path: &std::path::Path,
        assign_span: Span,
    ) -> Option<&hir::AssignPlaceContract> {
        self.assign_places
            .get(&hir::CallSite::new(source_path.to_path_buf(), assign_span))
    }

    pub(in crate::mir::lower) fn call_site_contract(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&TypedCallSiteContract> {
        self.call_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    pub(in crate::mir::lower) fn top_level_fun_call_fqn(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&str> {
        self.top_level_fun_call_fqns
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
            .map(String::as_str)
    }

    pub(in crate::mir::lower) fn continuation_identity_return_param(
        &self,
        fqn: &str,
    ) -> Option<usize> {
        self.continuation_identity_return_funs.get(fqn).copied()
    }

    pub(in crate::mir::lower) fn resume_call_info(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&ResumeCallInfo> {
        self.resume_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    pub(in crate::mir::lower) fn perform_metadata(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&PerformMetadata> {
        self.perform_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    pub(in crate::mir::lower) fn handle_site_info(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&HandleSiteInfo> {
        self.handle_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    pub(in crate::mir::lower) fn class_ctor_hidden_effects(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> EffectRow {
        self.class_ctor_hidden_effects
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
            .cloned()
            .unwrap_or_else(EffectRow::pure)
    }

    pub(in crate::mir::lower) fn object_member_hidden_effects(&self, fqn: &str) -> EffectRow {
        self.object_member_hidden_effects
            .get(fqn)
            .cloned()
            .unwrap_or_else(EffectRow::pure)
    }

    pub(in crate::mir::lower) fn top_level_ref_hidden_effects(&self, fqn: &str) -> EffectRow {
        self.top_level_ref_hidden_effects
            .get(fqn)
            .cloned()
            .unwrap_or_else(EffectRow::pure)
    }

    pub(in crate::mir::lower) fn top_level_init_roots(&self) -> &[TopLevelInitRootContract] {
        &self.top_level_init_roots
    }

    pub(in crate::mir::lower) fn extern_global_contracts(&self) -> &[ExternGlobalContract] {
        &self.extern_global_contracts
    }

    pub(in crate::mir::lower) fn when_pat_binding_ty(&self, span: Span) -> Option<TypeId> {
        self.when_pat_binding_tys.get(&span).copied()
    }
}

fn hir_call_site(identity: &hir_site_facts::SourceSiteIdentity) -> hir::CallSite {
    hir::CallSite::new(identity.source_path.clone(), identity.span)
}

fn function_contract_from_fact(fact: &hir_site_facts::FunctionTarget) -> FunctionTargetContract {
    FunctionTargetContract {
        fqn: fact.fqn.clone(),
        decl_file: fact.decl_file.clone(),
        decl_span: fact.decl_span,
        abi_identity: callable_abi_from_fact(fact.abi),
        param_tys: fact.param_tys.clone(),
        return_ty: fact.return_ty,
        type_args: fact.type_args.clone(),
        eff_args: fact.eff_args.clone(),
        arg_binding: fact.arg_binding.as_ref().map(call_arg_binding_from_fact),
    }
}

fn member_contract_from_fact(fact: &hir_site_facts::MemberCallTarget) -> MemberCallTargetContract {
    MemberCallTargetContract {
        owner_fqn: fact.owner_fqn.clone(),
        member_name: fact.member_name.clone(),
        member_fqn: fact.member_fqn.clone(),
        receiver_ty: fact.receiver_ty,
        function: function_contract_from_fact(&fact.function),
    }
}

fn constructor_contract_from_fact(
    fact: &hir_site_facts::ConstructorCallTarget,
) -> ConstructorCallTargetContract {
    ConstructorCallTargetContract {
        owner_fqn: fact.owner_fqn.clone(),
        ctor_span: fact.ctor_span,
        result_ty: fact.result_ty,
        arg_mapping: fact.arg_mapping.clone(),
    }
}

fn ctor_call_info_from_fact(fact: &hir_site_facts::ConstructorCallTarget) -> hir::CtorCallInfo {
    hir::CtorCallInfo {
        class_fqn: fact.owner_fqn.clone(),
        ctor_span: fact.ctor_span,
        arg_mapping: fact.arg_mapping.clone(),
    }
}

fn callable_abi_from_fact(abi: hir_site_facts::CallableAbi) -> hir::CallableAbiIdentity {
    match abi {
        hir_site_facts::CallableAbi::ManagedOrdinary => hir::CallableAbiIdentity::ManagedOrdinary,
        hir_site_facts::CallableAbi::NativeExtern => hir::CallableAbiIdentity::NativeExtern,
        hir_site_facts::CallableAbi::ManagedExtern => hir::CallableAbiIdentity::ManagedExtern,
        hir_site_facts::CallableAbi::EffectBridge => hir::CallableAbiIdentity::EffectBridge,
    }
}

fn intrinsic_kind_from_fact(fact: &hir_site_facts::IntrinsicKind) -> TypedIntrinsicKind {
    match fact {
        hir_site_facts::IntrinsicKind::Reflection { name } => {
            TypedIntrinsicKind::Reflection { name: name.clone() }
        }
        hir_site_facts::IntrinsicKind::Platform { name } => {
            TypedIntrinsicKind::Platform { name: name.clone() }
        }
        hir_site_facts::IntrinsicKind::Gc { name } => TypedIntrinsicKind::Gc { name: name.clone() },
        hir_site_facts::IntrinsicKind::Runtime { name } => {
            TypedIntrinsicKind::Runtime { name: name.clone() }
        }
        hir_site_facts::IntrinsicKind::Compiler { name } => {
            TypedIntrinsicKind::Compiler { name: name.clone() }
        }
        hir_site_facts::IntrinsicKind::NamedTable {
            entry_name,
            uses_runtime_call,
        } => TypedIntrinsicKind::NamedTable {
            entry_name: entry_name.clone(),
            uses_runtime_call: *uses_runtime_call,
        },
    }
}

fn call_arg_binding_from_fact(
    fact: &hir_site_facts::CallArgBindingContract,
) -> CallArgBindingContract {
    CallArgBindingContract::new(fact.params.iter().map(call_arg_param_from_fact).collect())
}

fn call_arg_param_from_fact(fact: &hir_site_facts::CallArgParamContract) -> CallArgParamContract {
    match fact {
        hir_site_facts::CallArgParamContract::Receiver => CallArgParamContract::Receiver,
        hir_site_facts::CallArgParamContract::Explicit(element) => {
            CallArgParamContract::Explicit(call_arg_element_from_fact(element))
        }
        hir_site_facts::CallArgParamContract::Default => CallArgParamContract::Default,
        hir_site_facts::CallArgParamContract::Vararg(elements) => {
            CallArgParamContract::Vararg(elements.iter().map(call_arg_element_from_fact).collect())
        }
    }
}

fn call_arg_element_from_fact(
    fact: &hir_site_facts::CallArgElementContract,
) -> CallArgElementContract {
    CallArgElementContract::new(fact.arg_index, fact.spread)
}

fn resume_call_info_from_fact(fact: &hir_site_facts::ContinuationResumeContract) -> ResumeCallInfo {
    ResumeCallInfo {
        receiver_route: match fact.receiver_route {
            hir_site_facts::ContinuationResumeReceiverRoute::CallArg { index } => {
                ContinuationResumeReceiverRoute::CallArg { index }
            }
            hir_site_facts::ContinuationResumeReceiverRoute::MemberReceiver => {
                ContinuationResumeReceiverRoute::MemberReceiver
            }
        },
        payload_arg_indices: fact.payload_arg_indices.clone(),
        metadata: ResumeMetadata {
            continuation_ty: fact.receiver_ty,
            resume_ty: fact.resume_ty,
            answer_ty: fact.answer_ty,
            return_ty: fact.return_ty,
            out_effects: fact.out_effects.clone(),
            runtime_error_effect_ty: fact.runtime_error_effect_ty,
            suspends_outward: fact.resumes_outward(),
        },
    }
}

fn perform_metadata_from_fact(fact: &hir_site_facts::PerformSiteContract) -> PerformMetadata {
    PerformMetadata {
        effect_ty: fact.effect_ty,
        op_type_args: Vec::new(),
        result_ty: fact.result_ty,
        payload_tuple_ty: fact.payload.ty,
        payload_component_tys: fact.payload.components.clone(),
        payload_transport: Vec::new(),
        arg_mapping: fact.arg_mapping.clone(),
    }
}

fn handle_site_info_from_fact(fact: &hir_site_facts::HandleSiteContract) -> HandleSiteInfo {
    let arms = fact
        .arm_contracts
        .iter()
        .map(|arm| HandlerArm {
            op_fqn: arm.op_fqn.clone(),
            op_type_args: Vec::new(),
            binder_count: arm.payload.components.len(),
            binder_locals: Vec::new(),
            continuation_local: None,
            handled_effect_ty: arm.handled_effect_ty,
            payload_tuple_ty: arm.payload.ty,
            payload_component_tys: arm.payload.components.clone(),
            body_ty: arm.body_ty,
            kind: match arm.kind {
                hir_site_facts::HandleArmContractKind::NonResuming => HandlerArmKind::NonResuming,
                hir_site_facts::HandleArmContractKind::EscapeContinuation => {
                    HandlerArmKind::EscapeContinuation
                }
            },
        })
        .collect();
    HandleSiteInfo {
        metadata: HandleMetadata {
            result_ty: fact.result_ty,
            body_result_ty: fact.body_result_ty,
            finally_result_ty: fact.finally_result_ty,
        },
        arms,
    }
}

fn assign_place_contract_from_fact(
    fact: &hir_site_facts::AssignmentContract,
) -> hir::AssignPlaceContract {
    hir::AssignPlaceContract {
        span: fact.span,
        kind: assign_place_kind_from_fact(&fact.kind),
        place_ty: fact.place_ty,
        value_ty: fact.value_ty,
        mutable: fact.mutable,
        write_barrier: assign_write_barrier_from_fact(&fact.write_barrier),
        unsafe_required: fact.unsafe_required,
    }
}

fn assign_place_kind_from_fact(fact: &hir_site_facts::AssignPlaceKind) -> hir::AssignPlaceKind {
    match fact {
        hir_site_facts::AssignPlaceKind::Local {
            symbol_id,
            name,
            decl_span,
        } => hir::AssignPlaceKind::Local {
            id: hir::SymbolId::from_raw(*symbol_id),
            name: name.clone(),
            decl_span: *decl_span,
        },
        hir_site_facts::AssignPlaceKind::TopLevel { symbol_id, fqn } => {
            hir::AssignPlaceKind::TopLevel {
                id: hir::SymbolId::from_raw(*symbol_id),
                fqn: fqn.clone(),
            }
        }
        hir_site_facts::AssignPlaceKind::Member {
            receiver_ty,
            owner_fqn,
            member_fqn,
            member_name,
            member_span,
            resolved,
        } => hir::AssignPlaceKind::Member {
            receiver_ty: *receiver_ty,
            owner_fqn: owner_fqn.clone(),
            member_fqn: member_fqn.clone(),
            member_name: member_name.clone(),
            member_span: *member_span,
            resolved: resolved.as_ref().map(member_ref_from_fact),
        },
    }
}

fn member_ref_from_fact(fact: &hir_site_facts::MemberRef) -> hir::MemberRef {
    match fact {
        hir_site_facts::MemberRef::Value { symbol_id, fqn } => hir::MemberRef::Value {
            id: hir::SymbolId::from_raw(*symbol_id),
            fqn: fqn.clone(),
        },
        hir_site_facts::MemberRef::Fun { symbol_id, fqn } => hir::MemberRef::Fun {
            id: hir::SymbolId::from_raw(*symbol_id),
            fqn: fqn.clone(),
        },
        hir_site_facts::MemberRef::ExtensionValue { symbol_id, fqn } => {
            hir::MemberRef::ExtensionValue {
                id: hir::SymbolId::from_raw(*symbol_id),
                fqn: fqn.clone(),
            }
        }
        hir_site_facts::MemberRef::ExtensionFun { symbol_id, fqn } => {
            hir::MemberRef::ExtensionFun {
                id: hir::SymbolId::from_raw(*symbol_id),
                fqn: fqn.clone(),
            }
        }
    }
}

fn assign_write_barrier_from_fact(
    fact: &hir_site_facts::AssignWriteBarrierRequirement,
) -> ast::AssignWriteBarrierRequirement {
    match fact {
        hir_site_facts::AssignWriteBarrierRequirement::NotRequired => {
            ast::AssignWriteBarrierRequirement::NotRequired
        }
        hir_site_facts::AssignWriteBarrierRequirement::StorageSlot { slot_ty } => {
            ast::AssignWriteBarrierRequirement::StorageSlot { slot_ty: *slot_ty }
        }
    }
}

fn top_level_init_root_from_fact(
    fact: &hir_site_facts::TopLevelInitRootContract,
) -> TopLevelInitRootContract {
    TopLevelInitRootContract {
        fqn: fact.fqn.clone(),
        source_path: fact.source_path.clone(),
        span: fact.span,
        kind: match fact.kind {
            hir_site_facts::TopLevelInitRootKind::RuntimeImmutableVal => {
                TopLevelInitRootKind::RuntimeImmutableVal
            }
            hir_site_facts::TopLevelInitRootKind::RuntimeMutableVar { storage } => {
                TopLevelInitRootKind::RuntimeMutableVar {
                    storage: top_level_storage_from_fact(storage),
                }
            }
            hir_site_facts::TopLevelInitRootKind::ObjectSingleton => {
                TopLevelInitRootKind::ObjectSingleton
            }
        },
        ty: fact.ty,
        initializer_ty: fact.initializer_ty,
        has_initializer: fact.has_initializer,
        dependencies: fact
            .dependencies
            .iter()
            .map(|dependency| TopLevelInitDependency {
                fqn: dependency.fqn.clone(),
                kind: match dependency.kind {
                    hir_site_facts::TopLevelInitDependencyKind::TopLevelValue => {
                        TopLevelInitDependencyKind::TopLevelValue
                    }
                    hir_site_facts::TopLevelInitDependencyKind::ObjectSingleton => {
                        TopLevelInitDependencyKind::ObjectSingleton
                    }
                },
            })
            .collect(),
    }
}

fn extern_global_contract_from_fact(
    fact: &hir_site_facts::ExternGlobalContract,
) -> ExternGlobalContract {
    ExternGlobalContract {
        fqn: fact.fqn.clone(),
        source_path: fact.source_path.clone(),
        span: fact.span,
        ty: fact.ty,
        mutable: fact.mutable,
        symbol: fact.symbol.clone(),
        linkage: match fact.linkage {
            hir_site_facts::ExternGlobalLinkage::External => hir::ExternGlobalLinkage::External,
        },
        storage: top_level_storage_from_fact(fact.storage),
        initializer_absent: fact.initializer_absent,
        unsafe_required: fact.unsafe_required,
    }
}

fn top_level_storage_from_fact(
    storage: scoopc_hir_facts::globals::GlobalStoragePolicy,
) -> hir::TopLevelVarStorage {
    match storage {
        scoopc_hir_facts::globals::GlobalStoragePolicy::Global => hir::TopLevelVarStorage::Global,
        scoopc_hir_facts::globals::GlobalStoragePolicy::ThreadLocal => {
            hir::TopLevelVarStorage::ThreadLocal
        }
    }
}
