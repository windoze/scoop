//! Composed-call boundary lowering: dispatch and replay logic for call boundaries that are sequenced behind another resume-producing boundary, including double-resume detection and the prefix-replay analysis.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn double_resume_runtime_error_case(
        &self,
    ) -> Result<(CaseTag, TypeId), LlvmEmitError> {
        let mut selected = None::<(CaseTag, TypeId)>;
        for boundary in self.callable.boundary_map().entries() {
            let Some(LateLoweredBoundaryLowering::RuntimeError(lowering)) = boundary.lowering()
            else {
                continue;
            };
            let emission = lowering.emitted_step().clone();
            if self.step_layout.case_layout(emission.case_tag()).is_none() {
                continue;
            }
            let candidate = (emission.case_tag(), emission.payload_tuple_ty());
            match &selected {
                Some(existing) if existing != &candidate => {
                    return Err(frontend_error(format!(
                        "callable `{}` 存在多义 double resume runtime error emission：{:?} 与 {:?}",
                        self.callable.root_fqn(),
                        existing,
                        candidate,
                    )));
                }
                Some(_) => {}
                None => selected = Some(candidate),
            }
        }
        if selected.is_none() {
            for (case_tag, case_layout) in self.step_layout.cases() {
                let payload_ty = case_layout.variant().payload_source_ty();
                if !self.source_ty_is_runtime_error(payload_ty) {
                    continue;
                }
                let candidate = (*case_tag, payload_ty);
                match &selected {
                    Some(existing) if existing != &candidate => {
                        return Err(frontend_error(format!(
                            "callable `{}` 存在多义 double resume runtime error Step case：{:?} 与 {:?}",
                            self.callable.root_fqn(),
                            existing,
                            candidate,
                        )));
                    }
                    Some(_) => {}
                    None => selected = Some(candidate),
                }
            }
        }
        selected.ok_or_else(|| {
            frontend_error(format!(
                "callable `{}` 缺少 double resume 可用的 ordinary runtime error boundary emission",
                self.callable.root_fqn(),
            ))
        })
    }

    pub(super) fn dispatch_composed_call_boundary_resume(
        &mut self,
        resume_state_tag: IntValue<'ctx>,
        callee_continuation: PointerValue<'ctx>,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<bool, LlvmEmitError> {
        let mut composition_entries = Vec::new();
        for boundary in self.callable.boundary_map().entries() {
            match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                    for composition in
                        lowering
                            .continuation_compositions()
                            .iter()
                            .filter(|composition| {
                                composition.caller_continuation_contract().resume_tuple_ty()
                                    == resume_tuple_ty
                            })
                    {
                        composition_entries.push((
                            boundary.clone(),
                            Some(lowering.clone()),
                            lowering.dispatch().clone(),
                            lowering.continuation_compositions().to_vec(),
                            composition.clone(),
                        ));
                    }
                }
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                    for composition in
                        lowering
                            .continuation_compositions()
                            .iter()
                            .filter(|composition| {
                                composition.caller_continuation_contract().resume_tuple_ty()
                                    == resume_tuple_ty
                            })
                    {
                        composition_entries.push((
                            boundary.clone(),
                            None,
                            lowering.dispatch().clone(),
                            lowering.continuation_compositions().to_vec(),
                            composition.clone(),
                        ));
                    }
                }
                _ => {}
            }
        }
        if composition_entries.is_empty() {
            return Ok(false);
        }
        let invalid_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_composed_invalid_state");
        let mut cases = Vec::with_capacity(composition_entries.len());
        let mut seen_states = BTreeMap::new();
        for (boundary, call_lowering, dispatch, compositions, composition) in composition_entries {
            let resume_state = composition.caller_resume_state();
            let candidate = (
                boundary.boundary_id(),
                composition.callee_continuation_schema(),
                composition.input_step_schema(),
            );
            if let Some(existing) = seen_states.get(&resume_state) {
                if *existing != candidate {
                    return Err(frontend_error(format!(
                        "callable `{}` resume state st{} 存在多义 call-boundary continuation composition origin：{:?} 与 {:?}",
                        self.callable.root_fqn(),
                        resume_state.as_u32(),
                        existing,
                        candidate,
                    )));
                }
                continue;
            }
            seen_states.insert(resume_state, candidate);
            let bb = self.codegen.context.append_basic_block(
                self.function,
                &format!(
                    "resume_composed_bd{}_case{}",
                    boundary.boundary_id().as_u32(),
                    composition.input_case_tag().as_u32(),
                ),
            );
            cases.push((
                self.codegen
                    .context
                    .i32_type()
                    .const_int(composition.caller_resume_state().as_u32() as u64, false),
                bb,
                boundary,
                call_lowering,
                dispatch,
                compositions,
                composition,
            ));
        }
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _, _, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.codegen
            .builder
            .build_switch(resume_state_tag, invalid_bb, &switch_cases)?;

        for (_, bb, boundary, call_lowering, dispatch, compositions, composition) in cases {
            self.codegen.builder.position_at_end(bb);
            let dispatch_context = ComposedBoundaryDispatchContext {
                call_lowering: call_lowering.as_ref(),
                dispatch: &dispatch,
                continuation_compositions: &compositions,
            };
            self.resume_composed_call_boundary_case(
                &boundary,
                dispatch_context,
                &composition,
                callee_continuation,
                resume_tuple_ty,
                payload,
            )?;
        }

        self.codegen.builder.position_at_end(invalid_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(true)
    }

    pub(super) fn resume_composed_call_boundary_case(
        &mut self,
        boundary: &LateLoweredBoundary,
        dispatch_context: ComposedBoundaryDispatchContext<'_>,
        composition: &LateLoweredCallBoundaryContinuationComposition,
        callee_continuation: PointerValue<'ctx>,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if composition.boundary_id() != boundary.boundary_id()
            || composition.caller_resume_state() != boundary.resume_state()
        {
            return Err(frontend_error(format!(
                "composed call boundary bd{} continuation composition 与 boundary resume state 漂移：composition={:?}",
                boundary.boundary_id().as_u32(),
                composition,
            )));
        }
        let surface = self
            .abi
            .surface_resume_layout(composition.callee_continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "composed call boundary bd{} 缺少 callee continuation schema k{} surface ABI",
                    boundary.boundary_id().as_u32(),
                    composition.callee_continuation_schema().as_u32(),
                ))
            })?;
        if surface.resume_tuple_ty() != resume_tuple_ty {
            return Err(frontend_error(format!(
                "composed call boundary bd{} callee surface ABI 漂移：surface_resume=t{} surface_out=s{} composition_resume=t{} composition_out=s{}",
                boundary.boundary_id().as_u32(),
                surface.resume_tuple_ty().as_u32(),
                surface.return_step_schema().as_u32(),
                resume_tuple_ty.as_u32(),
                composition.input_step_schema().as_u32(),
            )));
        }
        let deferred_callee_continuation = self.codegen.defer_gc_ref_pointer(
            self.mir_fun.span,
            "composed_resume_callee_continuation",
            callee_continuation,
        )?;
        let deferred_payload = payload
            .map(|raw| {
                let payload_cg = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, resume_tuple_ty)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "composed call boundary bd{} payload t{} 缺少 codegen type",
                            boundary.boundary_id().as_u32(),
                            resume_tuple_ty.as_u32(),
                        ))
                    })?;
                self.codegen.defer_gc_sensitive_cg_value(
                    self.mir_fun.span,
                    "composed_resume_payload",
                    CgValue {
                        ty: payload_cg,
                        value: Some(raw),
                    },
                )
            })
            .transpose()?;
        if let Some(call_lowering) = dispatch_context.call_lowering
            && self
                .call_boundary_prefix_replay_matches_prior_resuming_route(boundary, call_lowering)?
            && !self.call_boundary_tail_has_later_resuming_boundary(boundary, call_lowering)?
        {
            self.replay_call_boundary_prefix(boundary, call_lowering)?;
        }
        let callee = self
            .codegen
            .surface_resume_outcome_function(surface);
        let callee_continuation = self.codegen.reload_deferred_gc_ref_without_clearing(
            self.mir_fun.span,
            "composed_resume_callee_continuation_reload",
            &deferred_callee_continuation,
        )?;
        let mut args = vec![callee_continuation.into()];
        if !surface.resume_payload_abi().is_elided() {
            args.push(
                deferred_payload
                    .as_ref()
                    .map(|value| {
                        self.codegen.reload_deferred_cg_value_without_clearing(
                            self.mir_fun.span,
                            "composed_resume_payload_reload",
                            value,
                        )
                    })
                    .transpose()?
                    .and_then(|value| value.value)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "composed call boundary bd{} callee resume 需要 non-elided payload（function=`{}`, surface=`{}`, resume_tuple_ty=t{} `{}`, payload_present={}）",
                            boundary.boundary_id().as_u32(),
                            self.function.get_name().to_str().unwrap_or("<invalid>"),
                            surface.symbol_name(),
                            resume_tuple_ty.as_u32(),
                            self.source_types.display(resume_tuple_ty),
                            payload.is_some(),
                        ))
                    })?
                    .into(),
            );
        }
        let outcome_slot = self
            .codegen
            .alloc_effect_outcome_slot(self.mir_fun.span, "composed_resume")?;
        args.push(outcome_slot.into());
        self.codegen.build_call_preserving_gc_local_roots(
            self.mir_fun.span,
            callee,
            &args,
            "composed_callee_resume_outcome",
        )?;
        self.codegen.clear_deferred_cg_value_root_homes(
            self.mir_fun.span,
            "composed_resume_callee_continuation_clear",
            &deferred_callee_continuation,
        )?;
        if let Some(deferred_payload) = &deferred_payload {
            self.codegen.clear_deferred_cg_value_root_homes(
                self.mir_fun.span,
                "composed_resume_payload_clear",
                deferred_payload,
            )?;
        }
        let step_layout = self
            .abi
            .step_layout(surface.return_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "composed call boundary bd{} 缺少 callee step schema s{} layout",
                    boundary.boundary_id().as_u32(),
                    surface.return_step_schema().as_u32(),
                ))
            })?;
        let step = self.build_step_from_effect_outcome(
            step_layout,
            outcome_slot,
            "composed_resume_outcome",
        )?;
        self.dispatch_boundary_step(
            boundary,
            surface.return_step_schema(),
            step,
            dispatch_context.dispatch,
            dispatch_context.call_lowering,
            Some(dispatch_context.continuation_compositions),
        )
    }

    pub(super) fn call_boundary_prefix_replay_matches_prior_resuming_route(
        &self,
        boundary: &LateLoweredBoundary,
        lowering: &LateLoweredCallBoundaryLowering,
    ) -> Result<bool, LlvmEmitError> {
        let output_cases = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|case| case.emission().case_tag())
            .collect::<BTreeSet<_>>();
        if output_cases.is_empty() {
            return Ok(false);
        }

        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator()
            else {
                continue;
            };
            if contract.boundary_routing(boundary.boundary_id()).is_none() {
                continue;
            }

            let matched_cases = contract
                .boundary_routings()
                .iter()
                .filter(|routing| routing.boundary_id() != boundary.boundary_id())
                .filter(|routing| routing.resume_state() == boundary.owner_state())
                .flat_map(|routing| routing.case_routings())
                .filter(|case| output_cases.contains(&case.case_tag()))
                .filter(|case| match case.action() {
                    LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                        arm_ordinal,
                        ..
                    } => contract
                        .handled_arm_by_ordinal(arm_ordinal)
                        .and_then(|arm| arm.continuation_binder())
                        .is_some(),
                    LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { .. }
                    | LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => false,
                })
                .map(|case| case.case_tag())
                .collect::<BTreeSet<_>>();

            if output_cases.iter().all(|case| matched_cases.contains(case)) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(super) fn call_boundary_tail_has_later_resuming_boundary(
        &self,
        boundary: &LateLoweredBoundary,
        lowering: &LateLoweredCallBoundaryLowering,
    ) -> Result<bool, LlvmEmitError> {
        let LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            ..
        } = lowering.operand_contract().source_consumption()
        else {
            return Ok(false);
        };
        for candidate in self.callable.boundary_map().entries() {
            if candidate.boundary_id() == boundary.boundary_id() {
                continue;
            }
            let Some(LateLoweredBoundaryLowering::Perform(perform)) = candidate.lowering() else {
                continue;
            };
            let candidate_consumption = perform.operand_contract().source_consumption();
            if candidate_consumption.source_slice().block_id() != source_slice.block_id() {
                continue;
            }
            let starts_after_call = match candidate_consumption {
                LateLoweredBoundarySourceConsumption::Statement {
                    statement_index: candidate_index,
                    ..
                } => candidate_index > statement_index,
                LateLoweredBoundarySourceConsumption::Terminator {
                    source_slice: candidate_slice,
                } => candidate_slice.start_statement_index() >= source_slice.end_statement_index(),
            };
            if !starts_after_call {
                continue;
            }
            let Some(HandleBoundaryRuntimeAction::ConsumeToArm(action)) = self
                .handle_boundary_action(
                    candidate.boundary_id(),
                    perform.emitted_step().case_tag(),
                )?
            else {
                continue;
            };
            if action.continuation_binder.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn replay_call_boundary_prefix(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &LateLoweredCallBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let Some(owner_state) = self.callable.state_graph().state(boundary.owner_state()) else {
            return Err(frontend_error(format!(
                "composed call replay bd{} 缺少 owner state st{}",
                boundary.boundary_id().as_u32(),
                boundary.owner_state().as_u32(),
            )));
        };
        if !matches!(owner_state.role(), LateLoweredStateRole::Resume) {
            return Ok(());
        }
        let LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            ..
        } = lowering.operand_contract().source_consumption()
        else {
            return Ok(());
        };
        if source_slice.start_statement_index() != 0 {
            return Ok(());
        }
        let block = self
            .body
            .blocks
            .get(source_slice.block_id().as_u32() as usize)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "composed call replay block",
                at: self.mir_fun.span.into(),
            })?;
        for stmt_index in source_slice.start_statement_index()..statement_index {
            let stmt =
                block
                    .stmts
                    .get(stmt_index as usize)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "composed call replay statement",
                        at: self.mir_fun.span.into(),
                    })?;
            let classification = self
                .callable
                .source_statement_classification(source_slice, stmt_index)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "composed call replay bb{} stmt{} 缺少 published classification",
                        source_slice.block_id().as_u32(),
                        stmt_index,
                    ))
                })?;
            match classification.kind() {
                LateLoweredSourceStatementClassificationKind::EffectNeutralValue
                | LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
                    ..
                }
                | LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
                    ..
                } => {
                    if !self.lower_published_call_statement(stmt)? {
                        self.lower_effect_neutral_statement(stmt)?;
                    }
                }
                LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { .. }
                | LateLoweredSourceStatementClassificationKind::ResumePayloadInjection { .. }
                | LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
                    ..
                }
                | LateLoweredSourceStatementClassificationKind::ElidedUnreachable => {}
                LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
                    return Err(frontend_error(format!(
                        "composed call replay bb{} stmt{} classified unsupported: {reason}",
                        source_slice.block_id().as_u32(),
                        stmt_index,
                    )));
                }
            }
        }
        Ok(())
    }

    pub(super) fn resume_payload_binding_accepts_tuple(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
    ) -> Result<bool, LlvmEmitError> {
        let Some(resume_cg) = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, resume_tuple_ty)
        else {
            return Ok(false);
        };
        let slot = self.codegen.mir_local_slot(
            self.mir_fun.span,
            &self.slots,
            binding.consumer_local(),
        )?;
        Ok(slot.cg_ty == resume_cg || self.is_task_transport_tuple_ty(resume_tuple_ty)?)
    }

    pub(super) fn is_task_transport_tuple_ty(&self, ty: TypeId) -> Result<bool, LlvmEmitError> {
        let Some(codegen_ty) = self
            .codegen
            .equivalent_codegen_type_id(self.source_types, ty)
        else {
            return Ok(false);
        };
        Ok(self.codegen.is_task_transport_tuple_ty(codegen_ty))
    }
}
