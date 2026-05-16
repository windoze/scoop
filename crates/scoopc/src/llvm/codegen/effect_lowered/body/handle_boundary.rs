//! Handle-boundary lowering: dispatches handle steps, consumes arm payloads into binder slots, applies pending completions, and runs the per-region boundary action that selects an outward, continue, or goto outcome.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn lower_handle_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredHandleBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let handle_site_id = Self::handle_boundary_site_id(boundary).ok_or_else(|| {
            frontend_error(format!(
                "handle boundary bd{} 缺少 Handle site id",
                boundary.boundary_id().as_u32()
            ))
        })?;
        match lowering.outward_emissions() {
            [] => {
                self.restore_and_clear_handle_effect_ctx_slots(
                    handle_site_id,
                    "handle_exit_ctx",
                    "handle_exit_ctx_clear",
                )?;
                self.branch_to_state(boundary.resume_state())
            }
            [emission] => {
                self.restore_and_clear_handle_effect_ctx_slots(
                    handle_site_id,
                    "handle_outward_ctx",
                    "handle_outward_ctx_clear",
                )?;
                self.emit_or_consume_outward_case(
                    boundary,
                    emission.case_tag(),
                    None,
                    emission.payload_tuple_ty(),
                    None,
                    None,
                )
            }
            emissions => Err(frontend_error(format!(
                "handle boundary bd{} 发布了 {} 个 outward emission；需要 HandleDispatch contract 选择具体 case",
                boundary.boundary_id().as_u32(),
                emissions.len()
            ))),
        }
    }

    pub(super) fn dispatch_boundary_step(
        &mut self,
        boundary: &LateLoweredBoundary,
        input_step_schema: StepSchemaId,
        step: BasicValueEnum<'ctx>,
        dispatch: &crate::effect_lowered::ir::LateLoweredStepDispatchPlan,
        call_lowering: Option<&LateLoweredCallBoundaryLowering>,
        continuation_compositions: Option<&[LateLoweredCallBoundaryContinuationComposition]>,
    ) -> Result<(), LlvmEmitError> {
        let input_layout = self.abi.step_layout(input_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "boundary dispatch 缺少 input step schema s{} layout",
                input_step_schema.as_u32()
            ))
        })?;
        let function = self.function;
        let complete_bb = self.codegen.context.append_basic_block(
            function,
            &format!("bd{}_complete", boundary.boundary_id().as_u32()),
        );
        let unmatched_bb = self.codegen.context.append_basic_block(
            function,
            &format!("bd{}_unmatched", boundary.boundary_id().as_u32()),
        );
        let mut cases = Vec::new();
        for case in dispatch.outward_cases() {
            if let Some(case_layout) = input_layout.case_layout(case.input_case_tag()) {
                let bb = self.codegen.context.append_basic_block(
                    function,
                    &format!(
                        "bd{}_case{}",
                        boundary.boundary_id().as_u32(),
                        case.input_case_tag().as_u32()
                    ),
                );
                cases.push((
                    self.codegen
                        .context
                        .i32_type()
                        .const_int(case_layout.variant().tag_value() as u64, false),
                    bb,
                    case.input_case_tag(),
                    case.emission().case_tag(),
                    case.emission().payload_tuple_ty(),
                ));
            }
        }
        let local_runtime_error_case = match call_lowering
            .and_then(LateLoweredCallBoundaryLowering::consumed_runtime_error_case)
        {
            Some(contract) => {
                let case_layout = input_layout.case_layout(contract.input_case_tag()).ok_or_else(|| {
                    frontend_error(format!(
                        "call boundary bd{} local runtime-error case c{} 缺少 input Step layout",
                        boundary.boundary_id().as_u32(),
                        contract.input_case_tag().as_u32()
                    ))
                })?;
                let source = boundary_site(boundary, "Call")?;
                let runtime = self.local_runtime_error_runtime_for_call(source, contract)?;
                let bb = self.codegen.context.append_basic_block(
                    function,
                    &format!(
                        "bd{}_local_runtime_error_case{}",
                        boundary.boundary_id().as_u32(),
                        contract.input_case_tag().as_u32()
                    ),
                );
                Some((
                    self.codegen
                        .context
                        .i32_type()
                        .const_int(case_layout.variant().tag_value() as u64, false),
                    bb,
                    contract.input_case_tag(),
                    runtime,
                ))
            }
            None => None,
        };
        let tag = self.codegen.extract_step_tag(input_layout, step)?;
        let dispatch_bb = self.codegen.context.append_basic_block(
            function,
            &format!("bd{}_dispatch", boundary.boundary_id().as_u32()),
        );
        let is_complete = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.codegen
                .context
                .i32_type()
                .const_int(STEP_TAG_COMPLETE, false),
            "step_is_complete",
        )?;
        self.codegen
            .builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;
        self.codegen.builder.position_at_end(dispatch_bb);
        let mut switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        if let Some((tag, bb, _, _)) = &local_runtime_error_case {
            switch_cases.push((*tag, *bb));
        }
        self.codegen
            .builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        self.codegen.builder.position_at_end(complete_bb);
        let payload = self.codegen.extract_step_payload(
            input_layout,
            step,
            input_layout.complete_variant(),
            "boundary_complete_payload",
        )?;
        self.store_boundary_result(boundary.boundary_id(), payload, boundary.resume_state())?;
        if matches!(
            boundary.lowering(),
            Some(LateLoweredBoundaryLowering::Resume(_))
        ) {
            self.restore_frame_slots_to_locals()?;
        }
        if !self.try_route_boundary_complete_to_handle_completion(boundary)? {
            // Resume complete tails may still consult frame-owned locals / handle ctx even when
            // the reachable suffix has no further suspend or handle terminator, so keep the
            // frame root alive conservatively on this path.
            if !matches!(
                boundary.lowering(),
                Some(LateLoweredBoundaryLowering::Resume(_))
            ) {
                self.release_frame_root_for_frame_free_tail(boundary.resume_state())?;
            }
            self.branch_to_state(boundary.resume_state())?;
        }

        for (_, bb, input_case, output_case, payload_ty) in cases {
            self.codegen.builder.position_at_end(bb);
            let case_layout = input_layout.case_layout(input_case).ok_or_else(|| {
                frontend_error(format!(
                    "boundary dispatch 缺少 case c{}",
                    input_case.as_u32()
                ))
            })?;
            let (payload, callee_continuation) = self.codegen.extract_step_case_parts(
                input_layout,
                step,
                case_layout,
                "boundary_case_payload",
            )?;
            let composition = match continuation_compositions {
                Some(compositions) => {
                    let composition = compositions
                        .iter()
                        .find(|composition| composition.input_case_tag() == input_case)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "boundary bd{} case c{} 缺少 continuation composition contract",
                                boundary.boundary_id().as_u32(),
                                input_case.as_u32(),
                            ))
                        })?;
                    if composition.output_case_tag() != output_case {
                        return Err(frontend_error(format!(
                            "boundary bd{} case c{} continuation composition 输出 case 漂移：composition=c{} dispatch=c{}",
                            boundary.boundary_id().as_u32(),
                            input_case.as_u32(),
                            composition.output_case_tag().as_u32(),
                            output_case.as_u32(),
                        )));
                    }
                    Some(composition)
                }
                None => None,
            };
            let continuation_for_binder = if continuation_compositions.is_some() {
                composition.map(|_| callee_continuation)
            } else {
                Some(callee_continuation)
            };
            self.emit_or_consume_outward_case(
                boundary,
                output_case,
                payload,
                payload_ty,
                continuation_for_binder,
                composition,
            )?;
        }

        if let Some((_, bb, input_case, runtime)) = local_runtime_error_case {
            self.codegen.builder.position_at_end(bb);
            let case_layout = input_layout.case_layout(input_case).ok_or_else(|| {
                frontend_error(format!(
                    "boundary dispatch 缺少 local runtime-error case c{}",
                    input_case.as_u32()
                ))
            })?;
            let (payload, _continuation) = self.codegen.extract_step_case_parts(
                input_layout,
                step,
                case_layout,
                "local_runtime_error_payload",
            )?;
            self.emit_local_runtime_error_terminal(&runtime, payload)?;
        }

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn try_route_boundary_complete_to_handle_completion(
        &mut self,
        boundary: &LateLoweredBoundary,
    ) -> Result<bool, LlvmEmitError> {
        let Some(result_local) = boundary_complete_result_local(boundary) else {
            return Ok(false);
        };
        let mut matched_target = None;
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator()
            else {
                continue;
            };
            if contract.needs_completion_state()
                || contract.boundary_routing(boundary.boundary_id()).is_none()
            {
                continue;
            }
            let target = match contract.state_region(boundary.owner_state()) {
                LateLoweredHandleStateRegion::Body => contract
                    .body_completion_payload_source()
                    .filter(|source| completion_payload_source_is_local(source, result_local))
                    .map(|_| contract.body_complete_target()),
                LateLoweredHandleStateRegion::Arm {
                    handled_case,
                    arm_ordinal,
                } => contract
                    .handled_arms()
                    .iter()
                    .find(|arm| {
                        arm.arm_ordinal() == arm_ordinal && arm.handled_case() == handled_case
                    })
                    .filter(|arm| {
                        completion_payload_source_is_local(
                            arm.completion_payload_source(),
                            result_local,
                        )
                    })
                    .map(|_| contract.arm_complete_target()),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            if matched_target.replace(target).is_some() {
                return Err(frontend_error(format!(
                    "boundary bd{} complete 命中多个 handle completion target",
                    boundary.boundary_id().as_u32(),
                )));
            }
        }
        let Some(target) = matched_target else {
            return Ok(false);
        };
        self.copy_boundary_complete_to_handle_return_payload(result_local, target)?;
        self.branch_to_state(target)?;
        Ok(true)
    }

    pub(super) fn copy_boundary_complete_to_handle_return_payload(
        &mut self,
        result_local: LocalId,
        target: StateId,
    ) -> Result<(), LlvmEmitError> {
        let Some(binding) = self
            .callable
            .frame_schema()
            .completion_payload_binding_for_state(target)
        else {
            return Ok(());
        };
        let Some(source) = binding.payload_source().operand_source() else {
            return Ok(());
        };
        let LateLoweredOperandValueSource::Local(target_local) = source.value() else {
            return Ok(());
        };
        if *target_local == result_local {
            return Ok(());
        }
        let value = self.load_local_value(self.mir_fun.span, result_local)?;
        let _ = self.store_local_value(self.mir_fun.span, *target_local, value)?;
        if let Some(frame_slot) = binding.payload_frame_slot() {
            self.store_local_to_frame_slot(*target_local, frame_slot)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_handle_boundary_consume_to_arm(
        &mut self,
        boundary: &LateLoweredBoundary,
        action: &HandleConsumeArmRuntime,
        case_tag: CaseTag,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<(), LlvmEmitError> {
        let continuation_effect_ctx =
            self.load_current_effect_ctx("handle_resume_effect_ctx")?;
        let continuation_effect_ctx_root_slot = self.capture_gc_pointer_root_slot(
            continuation_effect_ctx,
            "handle_resume_effect_ctx_root",
        )?;
        let arm_ctx = self.load_handle_arm_effect_ctx(
            action.site_id,
            action.arm_ordinal,
            "handle_arm_effect_ctx_load",
        )?;
        let arm_ctx_root_slot =
            self.capture_gc_pointer_root_slot(arm_ctx, "handle_arm_effect_ctx_root")?;
        self.store_current_effect_ctx(arm_ctx, "handle_arm_effect_ctx_store")?;

        let deferred_callee_continuation = callee_continuation
            .map(|continuation| {
                self.codegen.defer_gc_ref_pointer(
                    self.mir_fun.span,
                    "outward_callee_continuation",
                    continuation,
                )
            })
            .transpose()?;

        let deferred_payload = if action.continuation_binder.is_some() {
            let payload_cg = self
                .codegen
                .cg_ty_of_mir_type(self.source_types, payload_ty)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "handle arm payload t{} 缺少 codegen type",
                        payload_ty.as_u32()
                    ))
                })?;
            payload
                .map(|raw| {
                    self.codegen.defer_gc_sensitive_cg_value(
                        self.mir_fun.span,
                        "handle_arm_payload",
                        CgValue {
                            ty: payload_cg,
                            value: Some(raw),
                        },
                    )
                })
                .transpose()?
        } else {
            None
        };

        if let Some(binder) = action.continuation_binder {
            let callee_continuation = if let Some(deferred) = deferred_callee_continuation {
                Some(
                    self.codegen
                        .materialize_deferred_cg_value(
                            self.mir_fun.span,
                            "handle_arm_callee_continuation_reload",
                            deferred,
                        )?
                        .value
                        .ok_or_else(|| {
                            frontend_error(
                                "handle arm callee continuation reload 缺少值".to_string(),
                            )
                        })?
                        .into_pointer_value(),
                )
            } else {
                callee_continuation
            };
            let continuation = if composition.is_some() {
                let continuation_effect_ctx = self.reload_gc_pointer_from_root_slot(
                    continuation_effect_ctx_root_slot,
                    "handle_resume_effect_ctx_root",
                )?;
                self.store_current_effect_ctx(
                    continuation_effect_ctx,
                    "handle_continuation_effect_ctx_store",
                )?;
                self.create_continuation_object(
                    boundary.resume_state(),
                    case_tag,
                    callee_continuation,
                    composition,
                )?
            } else if let Some(callee_continuation) = callee_continuation {
                callee_continuation
            } else {
                let continuation_effect_ctx = self.reload_gc_pointer_from_root_slot(
                    continuation_effect_ctx_root_slot,
                    "handle_resume_effect_ctx_root",
                )?;
                self.store_current_effect_ctx(
                    continuation_effect_ctx,
                    "handle_continuation_effect_ctx_store",
                )?;
                self.create_continuation_object(boundary.resume_state(), case_tag, None, None)?
            };
            let continuation_root_slot = self.capture_gc_pointer_root_slot(
                continuation,
                "handle_continuation_binder_root",
            )?;
            let arm_ctx = self.reload_gc_pointer_from_root_slot(
                arm_ctx_root_slot,
                "handle_arm_effect_ctx_root",
            )?;
            self.store_current_effect_ctx(arm_ctx, "handle_arm_effect_ctx_restore")?;
            let continuation = self.reload_gc_pointer_from_root_slot(
                continuation_root_slot,
                "handle_continuation_binder_root",
            )?;
            self.store_gc_ref_to_binder(binder, continuation)?;
            self.clear_root_gc_slot(
                continuation_root_slot,
                "handle_continuation_binder_root_clear",
            )?;
        }

        self.clear_root_gc_slot(
            continuation_effect_ctx_root_slot,
            "handle_resume_effect_ctx_root_clear",
        )?;
        self.clear_root_gc_slot(
            arm_ctx_root_slot,
            "handle_arm_effect_ctx_root_clear",
        )?;

        let payload = if let Some(deferred_payload) = deferred_payload {
            self.codegen
                .materialize_deferred_cg_value(
                    self.mir_fun.span,
                    "handle_arm_payload_reload",
                    deferred_payload,
                )?
                .value
        } else {
            payload
        };
        self.store_case_payload_to_arm_binders(&action.payload_binders, payload, payload_ty)?;
        self.branch_to_state(action.arm_state)
    }

    pub(super) fn apply_handle_boundary_pending_completion(
        &mut self,
        action: &HandlePendingCompletionRuntime,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.begin_handle_pending_completion(action.clone(), Some((payload, payload_ty)))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch_handle_boundary_from_ctx(
        &mut self,
        case_tag: CaseTag,
        boundary: &LateLoweredBoundary,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
        candidates: &[HandleBoundaryDispatchCandidate],
    ) -> Result<bool, LlvmEmitError> {
        if candidates.is_empty() {
            return Ok(false);
        }

        let function = self.function;
        let dispatch_index = boundary.boundary_id().as_u32();
        let dispatch_entry_bb = self.codegen.builder.get_insert_block().ok_or_else(|| {
            frontend_error("handle ctx dispatch 缺少 active insert block".to_string())
        })?;
        let loop_bb = self.codegen.context.append_basic_block(
            function,
            &format!(
                "handle_ctx_dispatch_loop_bd{dispatch_index}_c{}",
                case_tag.as_u32()
            ),
        );
        let scan_bb = self.codegen.context.append_basic_block(
            function,
            &format!(
                "handle_ctx_dispatch_scan_bd{dispatch_index}_c{}",
                case_tag.as_u32()
            ),
        );
        let advance_bb = self.codegen.context.append_basic_block(
            function,
            &format!(
                "handle_ctx_dispatch_advance_bd{dispatch_index}_c{}",
                case_tag.as_u32()
            ),
        );
        let switch_bb = self.codegen.context.append_basic_block(
            function,
            &format!(
                "handle_ctx_dispatch_switch_bd{dispatch_index}_c{}",
                case_tag.as_u32()
            ),
        );
        let no_match_bb = self.codegen.context.append_basic_block(
            function,
            &format!(
                "handle_ctx_dispatch_outward_bd{dispatch_index}_c{}",
                case_tag.as_u32()
            ),
        );

        let current_ctx = self.load_current_effect_ctx("handle_dispatch_ctx")?;
        let current_ctx_ptr =
            self.cast_gc_ref_to_effect_ctx_ptr(current_ctx, "handle_dispatch_ctx_ptr")?;
        let handler_top = self
            .codegen
            .load_effect_ctx_handler_top(current_ctx_ptr, "handle_dispatch_top")?;
        let current_frame_gc = self.current_frame_gc_ref("handle_dispatch_owner_frame")?;
        let word_ty = self.codegen.llvm_ptr_sized_int_type(None);
        let current_frame_int = self.codegen.builder.build_ptr_to_int(
            current_frame_gc,
            word_ty,
            "handle_dispatch_owner_frame_int",
        )?;
        let active_mask = self
            .codegen
            .context
            .i32_type()
            .const_int(u64::from(self.codegen.effect_handler_active_flag()), false);
        let expected_op_tag = self
            .codegen
            .context
            .i32_type()
            .const_int(u64::from(self.handle_case_op_tag(case_tag)?), false);

        self.codegen.builder.build_unconditional_branch(loop_bb)?;

        self.codegen.builder.position_at_end(loop_bb);
        let node_phi = self.codegen.builder.build_phi(
            self.codegen.llvm_gc_i8_ptr_type(),
            "handle_dispatch_node",
        )?;
        node_phi.add_incoming(&[(&handler_top, dispatch_entry_bb)]);
        let node_gc = node_phi.as_basic_value().into_pointer_value();
        let is_null = self
            .codegen
            .builder
            .build_is_null(node_gc, "handle_dispatch_node_is_null")?;
        self.codegen
            .builder
            .build_conditional_branch(is_null, no_match_bb, scan_bb)?;

        self.codegen.builder.position_at_end(scan_bb);
        let node_ptr = self
            .cast_gc_ref_to_effect_handler_node_ptr(node_gc, "handle_dispatch_node_ptr")?;
        let node_flags = self
            .codegen
            .load_effect_handler_flags(node_ptr, "handle_dispatch_flags")?;
        let node_op_tag = self
            .codegen
            .load_effect_handler_op_tag(node_ptr, "handle_dispatch_op_tag")?;
        let node_owner = self
            .codegen
            .load_effect_handler_owner_frame_ref(node_ptr, "handle_dispatch_owner")?;
        let node_owner_int = self.codegen.builder.build_ptr_to_int(
            node_owner,
            word_ty,
            "handle_dispatch_owner_int",
        )?;
        let node_dispatch_identity = self
            .codegen
            .load_effect_handler_dispatch_identity(node_ptr, "handle_dispatch_identity")?;
        let node_active_bits = self.codegen.builder.build_and(
            node_flags,
            active_mask,
            "handle_dispatch_active_bits",
        )?;
        let is_active = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            node_active_bits,
            self.codegen.context.i32_type().const_zero(),
            "handle_dispatch_is_active",
        )?;
        let owner_matches = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            node_owner_int,
            current_frame_int,
            "handle_dispatch_owner_matches",
        )?;
        let op_matches = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            node_op_tag,
            expected_op_tag,
            "handle_dispatch_op_matches",
        )?;
        let active_owner = self.codegen.builder.build_and(
            is_active,
            owner_matches,
            "handle_dispatch_active_owner",
        )?;
        let should_switch = self.codegen.builder.build_and(
            active_owner,
            op_matches,
            "handle_dispatch_should_switch",
        )?;

        let mut switch_cases = Vec::with_capacity(candidates.len());
        let mut action_blocks = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let bb = self.codegen.context.append_basic_block(
                function,
                &format!(
                    "handle_ctx_dispatch_site_action_bd{dispatch_index}_c{}_id{:x}",
                    case_tag.as_u32(),
                    candidate.dispatch_identity
                ),
            );
            switch_cases.push((
                self.codegen
                    .context
                    .i64_type()
                    .const_int(candidate.dispatch_identity, false),
                bb,
            ));
            action_blocks.push((candidate.clone(), bb));
        }
        self.codegen
            .builder
            .build_conditional_branch(should_switch, switch_bb, advance_bb)?;

        self.codegen.builder.position_at_end(switch_bb);
        self.codegen
            .builder
            .build_switch(node_dispatch_identity, advance_bb, &switch_cases)?;

        for (candidate, bb) in action_blocks {
            self.codegen.builder.position_at_end(bb);
            match &candidate.action {
                HandleBoundaryRuntimeAction::ConsumeToArm(action) => {
                    self.apply_handle_boundary_consume_to_arm(
                        boundary,
                        action,
                        case_tag,
                        payload,
                        payload_ty,
                        callee_continuation,
                        composition,
                    )?;
                }
                HandleBoundaryRuntimeAction::PendingCompletion(action) => {
                    self.apply_handle_boundary_pending_completion(action, payload, payload_ty)?;
                }
                HandleBoundaryRuntimeAction::EmitOutward => {
                    self.codegen
                        .builder
                        .build_unconditional_branch(advance_bb)?;
                }
            }
        }

        self.codegen.builder.position_at_end(advance_bb);
        let prev_ref = self
            .codegen
            .load_effect_handler_prev_ref(node_ptr, "handle_dispatch_prev")?;
        self.codegen.builder.build_unconditional_branch(loop_bb)?;
        node_phi.add_incoming(&[(&prev_ref, advance_bb)]);

        self.codegen.builder.position_at_end(no_match_bb);
        Ok(true)
    }

    pub(super) fn emit_or_consume_outward_case(
        &mut self,
        boundary: &LateLoweredBoundary,
        case_tag: CaseTag,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<(), LlvmEmitError> {
        let origin_bb = self.codegen.builder.get_insert_block().ok_or_else(|| {
            frontend_error(format!(
                "boundary bd{} case c{} lowering 缺少 active insert block",
                boundary.boundary_id().as_u32(),
                case_tag.as_u32(),
            ))
        })?;
        if composition.is_some() && callee_continuation.is_none() {
            return Err(frontend_error(format!(
                "boundary bd{} case c{} 的 callee continuation 与 composition contract 不一致",
                boundary.boundary_id().as_u32(),
                case_tag.as_u32(),
            )));
        }
        let deferred_callee_continuation = callee_continuation
            .map(|continuation| {
                self.codegen.defer_gc_ref_pointer(
                    self.mir_fun.span,
                    "outward_callee_continuation",
                    continuation,
                )
            })
            .transpose()?;
        self.sync_frame_slots_from_locals()?;
        let routed_action = self.handle_boundary_action(boundary.boundary_id(), case_tag)?;
        let skip_finalized_site =
            if let Some(HandleBoundaryRuntimeAction::PendingCompletion(action)) =
                &routed_action
            {
                self.composed_resume_already_ran_handle_finally(action, composition)?
                    .then_some(action.site_id)
            } else {
                None
            };
        if let Some(action) = routed_action.as_ref()
            && skip_finalized_site.is_none()
        {
            match action {
                HandleBoundaryRuntimeAction::ConsumeToArm(action) => {
                    return self.apply_handle_boundary_consume_to_arm(
                        boundary,
                        action,
                        case_tag,
                        payload,
                        payload_ty,
                        callee_continuation,
                        composition,
                    );
                }
                HandleBoundaryRuntimeAction::PendingCompletion(action) => {
                    return self
                        .apply_handle_boundary_pending_completion(action, payload, payload_ty);
                }
                HandleBoundaryRuntimeAction::EmitOutward => {}
            }
        }
        let dispatch_candidates = self.handle_boundary_dispatch_candidates_excluding(
            boundary.boundary_id(),
            case_tag,
            skip_finalized_site,
        )?;
        let has_dispatch_candidates = !dispatch_candidates.is_empty();
        if self.dispatch_handle_boundary_from_ctx(
            case_tag,
            boundary,
            payload,
            payload_ty,
            callee_continuation,
            composition,
            &dispatch_candidates,
        )? {
            // The helper leaves the builder positioned at the explicit "no local match" block,
            // so the fallback outward path below now runs with the innermost matching local
            // handler (if any) already consumed via explicit `EffectCtx`.
        }
        if has_dispatch_candidates && self.codegen.builder.get_insert_block() == Some(origin_bb) {
            return Err(frontend_error(format!(
                "boundary bd{} case c{} 已解析到显式 handle dispatch candidate，但 origin block 仍未切到 dispatch loop；不能继续生成 fallback outward path",
                boundary.boundary_id().as_u32(),
                case_tag.as_u32(),
            )));
        }
        if !self.current_block_is_terminated()
            && let Some(action) = routed_action.as_ref()
            && skip_finalized_site.is_some()
        {
            match action {
                HandleBoundaryRuntimeAction::ConsumeToArm(action) => {
                    return self.apply_handle_boundary_consume_to_arm(
                        boundary,
                        action,
                        case_tag,
                        payload,
                        payload_ty,
                        callee_continuation,
                        composition,
                    );
                }
                HandleBoundaryRuntimeAction::PendingCompletion(action) => {
                    return self
                        .apply_handle_boundary_pending_completion(action, payload, payload_ty);
                }
                HandleBoundaryRuntimeAction::EmitOutward => {}
            }
        }
        if matches!(self.return_mode, CallableReturnMode::Plain { .. }) {
            if !has_dispatch_candidates {
                return Err(frontend_error(format!(
                    "plain callable `{}` 的 boundary bd{} case c{} 没有任何本地 handle/catch dispatch candidate；NoOutward plain body 不应回退到 outward Step_F path",
                    self.callable.root_fqn(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            self.codegen.builder.build_unreachable()?;
            return Ok(());
        }
        let deferred_payload = payload
            .map(|raw| {
                let payload_cg = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, payload_ty)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "outward payload t{} 缺少 codegen type",
                            payload_ty.as_u32()
                        ))
                    })?;
                self.codegen.defer_gc_sensitive_cg_value(
                    self.mir_fun.span,
                    "outward_payload",
                    CgValue {
                        ty: payload_cg,
                        value: Some(raw),
                    },
                )
            })
            .transpose()?;
        let callee_continuation = if let Some(deferred) = deferred_callee_continuation {
            Some(
                self.codegen
                    .materialize_deferred_cg_value(
                        self.mir_fun.span,
                        "outward_callee_continuation_reload",
                        deferred,
                    )?
                    .value
                    .ok_or_else(|| {
                        frontend_error(
                            "outward callee continuation reload 缺少值".to_string(),
                        )
                    })?
                    .into_pointer_value(),
            )
        } else {
            callee_continuation
        };
        let continuation = self.create_continuation_object(
            boundary.resume_state(),
            case_tag,
            callee_continuation,
            composition,
        )?;
        let payload = if let Some(deferred_payload) = deferred_payload {
            self.codegen
                .materialize_deferred_cg_value(
                    self.mir_fun.span,
                    "outward_payload_reload",
                    deferred_payload,
                )?
                .value
        } else {
            payload
        };
        match self.return_mode {
            CallableReturnMode::EffectOutcome => {
                let outcome = self.build_propagating_effect_outcome_for_case(
                    case_tag,
                    payload,
                    payload_ty,
                    continuation,
                )?;
                self.emit_effect_outcome_return(outcome)
            }
            _ => {
                let out_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
                    frontend_error(format!(
                        "callable `{}` step schema s{} 缺少 outward case c{}",
                        self.callable.root_fqn(),
                        self.abi_step_schema.as_u32(),
                        case_tag.as_u32()
                    ))
                })?;
                let step = self.codegen.build_step_case(
                    self.step_layout,
                    out_layout,
                    payload,
                    continuation,
                )?;
                self.return_step(step)
            }
        }
    }

    pub(super) fn composed_resume_already_ran_handle_finally(
        &self,
        action: &HandlePendingCompletionRuntime,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<bool, LlvmEmitError> {
        let Some(composition) = composition else {
            return Ok(false);
        };
        let dispatch = self
            .abi
            .surface_resume_dispatch_layout(composition.callee_continuation_schema())?;
        for target in dispatch.target().owner_trampolines() {
            if target
                .handle_binder_routes()
                .iter()
                .any(|route| route.site_id() == action.site_id)
            {
                return Ok(true);
            }
            if let Some(projection) = target.wrapper_projection()
                && matches!(
                    projection.underlying_route().publication(),
                    LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                        site_id,
                        ..
                    } if *site_id == action.site_id
                )
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
