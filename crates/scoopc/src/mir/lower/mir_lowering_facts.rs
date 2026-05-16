//! MirLoweringFacts impl: lowering side-table accumulation.

#![allow(dead_code)]

use super::*;

impl MirLoweringFacts {
    pub(crate) fn from_lowered_hir(
        lowered: &hir::LoweredHir,
        default_source_path: &std::path::Path,
    ) -> Result<Self, hir::HirLowerError> {
        let mut facts = Self::from_hir_side_tables_and_resume_spans(
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
        .with_enum_payload_kinds(lowered)
        .with_class_ctor_call_sites(lowered)
        .with_continuation_identity_return_funs(lowered)
        .with_class_ctor_hidden_effects(lowered);

        let contracts = TypedHirEffectContracts::from_lowered_hir(lowered, default_source_path)
            .map_err(hir::HirLowerError::from)?;
        facts.top_level_init_roots = contracts.top_level_init_roots().to_vec();
        facts.extern_global_contracts = contracts.extern_global_contracts().to_vec();
        for (call_site, contract) in contracts.continuation_resume_sites() {
            facts.resume_sites.insert(
                call_site.clone(),
                ResumeCallInfo {
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
        facts
            .call_sites
            .extend(contracts.call_site_contracts().clone());
        facts
            .assign_places
            .extend(contracts.assign_place_contracts().clone());

        Ok(facts)
    }

    pub(crate) fn from_typed_handoff(
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
            .with_enum_payload_kinds(lowered)
            .with_class_ctor_call_sites(lowered)
            .with_continuation_identity_return_funs(lowered)
            .with_class_ctor_hidden_effects(lowered);

        facts.with_typed_contracts(contracts)
    }

    pub(crate) fn from_hir_side_tables_and_resume_spans(
        dispatch_call_sites: &hir::DispatchCallSiteIndex,
        fallback_resume_site_spans: impl IntoIterator<Item = Span>,
        fallback_outward_resume_site_spans: impl IntoIterator<Item = Span>,
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

        facts.fallback_resume_site_spans = fallback_resume_site_spans.into_iter().collect();
        facts.fallback_outward_resume_site_spans =
            fallback_outward_resume_site_spans.into_iter().collect();
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
            self.fallback_perform_sites.insert(
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

    pub(in crate::mir::lower) fn with_member_value_types(
        mut self,
        lowered: &hir::LoweredHir,
    ) -> Self {
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

    pub(in crate::mir::lower) fn with_call_arg_bindings(
        mut self,
        lowered: &hir::LoweredHir,
    ) -> Self {
        self.call_arg_bindings.extend(
            lowered
                .call_arg_bindings
                .iter()
                .map(|(site, binding)| (site.clone(), lowered_call_arg_binding_contract(binding))),
        );
        self
    }

    pub(in crate::mir::lower) fn member_fqn_matches_owner(
        member_fqn: &str,
        owner_fqn: &str,
    ) -> bool {
        member_fqn
            .strip_prefix(owner_fqn)
            .is_some_and(|suffix| suffix.starts_with('.'))
    }

    pub(in crate::mir::lower) fn with_class_ctor_call_sites(
        mut self,
        lowered: &hir::LoweredHir,
    ) -> Self {
        self.class_ctor_call_sites
            .extend(lowered.ctor_call_sites.clone());
        self
    }

    pub(in crate::mir::lower) fn with_nominal_kinds(mut self, lowered: &hir::LoweredHir) -> Self {
        self.nominal_kinds.extend(lowered.nominal_kinds.clone());
        self
    }

    pub(in crate::mir::lower) fn with_enum_payload_kinds(
        mut self,
        lowered: &hir::LoweredHir,
    ) -> Self {
        self.enum_has_payload
            .extend(lowered.enum_layouts.iter().map(|(fqn, layout)| {
                (
                    fqn.clone(),
                    layout
                        .variants
                        .iter()
                        .any(|variant| !variant.fields.is_empty()),
                )
            }));
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

    pub(crate) fn with_typed_contracts(mut self, contracts: &TypedHirEffectContracts) -> Self {
        self.site_contract_source = MirSiteContractSource::Typed;
        self.fallback_resume_site_spans.clear();
        self.fallback_outward_resume_site_spans.clear();
        self.fallback_perform_sites.clear();
        self.resume_sites.clear();
        self.perform_sites.clear();
        self.handle_sites.clear();
        self.call_sites.clear();
        self.assign_places.clear();
        self.top_level_init_roots = contracts.top_level_init_roots().to_vec();
        self.extern_global_contracts = contracts.extern_global_contracts().to_vec();

        for (call_site, contract) in contracts.continuation_resume_sites() {
            self.resume_sites.insert(
                call_site.clone(),
                ResumeCallInfo {
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
            self.perform_sites.insert(
                call_site.clone(),
                PerformMetadata {
                    effect_ty: contract.effect_ty(),
                    op_type_args: Vec::new(),
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
                    op_type_args: Vec::new(),
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
            self.handle_sites.insert(
                call_site.clone(),
                HandleSiteInfo {
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
            self.call_sites.insert(call_site.clone(), contract.clone());
        }

        self.assign_places
            .extend(contracts.assign_place_contracts().clone());

        self
    }

    pub(in crate::mir::lower) fn uses_typed_contracts(&self) -> bool {
        self.site_contract_source == MirSiteContractSource::Typed
    }

    pub(in crate::mir::lower) fn nominal_kind(&self, fqn: &str) -> Option<ast::TypeKind> {
        self.nominal_kinds.get(fqn).copied()
    }

    pub(in crate::mir::lower) fn enum_has_payload(&self, fqn: &str) -> Option<bool> {
        self.enum_has_payload.get(fqn).copied()
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

    pub(in crate::mir::lower) fn top_level_fun_call_binding(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&ast::TopLevelFunCallBinding> {
        self.top_level_fun_call_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    pub(in crate::mir::lower) fn continuation_identity_return_param(
        &self,
        fqn: &str,
    ) -> Option<usize> {
        self.continuation_identity_return_funs.get(fqn).copied()
    }

    pub(in crate::mir::lower) fn fallback_resume_site_matches(&self, span: Span) -> bool {
        self.fallback_resume_site_spans.contains(&span)
    }

    pub(in crate::mir::lower) fn fallback_resume_site_suspends_outward(&self, span: Span) -> bool {
        self.fallback_outward_resume_site_spans.contains(&span)
    }

    pub(in crate::mir::lower) fn fallback_perform_site_info(
        &self,
        span: Span,
    ) -> Option<&PerformCallSiteInfo> {
        self.fallback_perform_sites.get(&span)
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
