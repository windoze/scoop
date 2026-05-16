//! Wrapper completion and return-path lowering: projects owner-step values onto wrapper completions, handles the routing decisions that turn a handle completion into a wrapper return / goto, and cleans up handle contexts before function exit.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn return_step(&mut self, step: BasicValueEnum<'ctx>) -> Result<(), LlvmEmitError> {
        match self.return_mode {
            CallableReturnMode::Plain { .. } => {
                return Err(frontend_error(format!(
                    "plain callable `{}` 的本地 effect/control path 尝试向外返回 Step_F；P5 handoff 应保证 NoOutward body 的 case 被本地 handle/catch 消费",
                    self.callable.root_fqn()
                )));
            }
            CallableReturnMode::EffectOutcome => {
                return Err(frontend_error(format!(
                    "outcome core `{}` 不应再直接返回 Step_F",
                    self.callable.root_fqn()
                )));
            }
            CallableReturnMode::Step => {}
        }
        self.sync_frame_slots_from_locals()?;
        if let Some(projection) = self.return_projection {
            self.project_owner_step_to_wrapper(projection, step)
        } else if let Some(return_step_schema) = self.return_step_schema {
            let projected = if return_step_schema == self.abi_step_schema {
                step
            } else {
                self.codegen.project_step_to_schema(
                    self.abi,
                    step,
                    self.abi_step_schema,
                    return_step_schema,
                )?
            };
            self.codegen.builder.build_return(Some(&projected))?;
            Ok(())
        } else {
            self.codegen.builder.build_return(Some(&step))?;
            Ok(())
        }
    }

    pub(super) fn project_owner_step_to_wrapper(
        &mut self,
        projection: &crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection,
        owner_step: BasicValueEnum<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let owner_step_schema = projection.owner_step_schema();
        let wrapper_step_schema = projection.wrapper_step_schema();
        let owner_layout = self.abi.step_layout(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "wrapper projection 缺少 owner step schema s{} layout",
                owner_step_schema.as_u32()
            ))
        })?;
        let wrapper_layout = self.abi.step_layout(wrapper_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "wrapper projection 缺少 wrapper step schema s{} layout",
                wrapper_step_schema.as_u32()
            ))
        })?;
        let tag = self.codegen.extract_step_tag(owner_layout, owner_step)?;
        let complete_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "wrapper_project_complete");
        let unmatched_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "wrapper_project_unmatched");
        let cases = projection
            .outward_cases()
            .iter()
            .map(|case| {
                let owner_case_tag = case.owner_case_tag();
                let wrapper_case_tag = case.wrapper_case_tag();
                let owner_case = owner_layout
                    .case_layout(owner_case_tag)
                    .expect("projection case was validated by helper");
                (
                    self.codegen
                        .context
                        .i32_type()
                        .const_int(owner_case.variant().tag_value() as u64, false),
                    self.codegen.context.append_basic_block(
                        self.function,
                        &format!("wrapper_project_case{}", wrapper_case_tag.as_u32()),
                    ),
                    owner_case_tag,
                    wrapper_case_tag,
                )
            })
            .collect::<Vec<_>>();
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        let complete_tag = self
            .codegen
            .context
            .i32_type()
            .const_int(STEP_TAG_COMPLETE, false);
        let is_complete = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            complete_tag,
            "wrapper_project_is_complete",
        )?;
        let dispatch_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "wrapper_project_dispatch");
        self.codegen
            .builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;

        self.codegen.builder.position_at_end(dispatch_bb);
        self.codegen
            .builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        self.codegen.builder.position_at_end(complete_bb);
        let payload = self.lower_wrapper_complete_payload(
            projection.complete().payload_source(),
            owner_layout,
            owner_step,
        )?;
        let projected = self.codegen.build_step_complete(wrapper_layout, payload)?;
        self.codegen.builder.build_return(Some(&projected))?;

        for (_, bb, owner_case, wrapper_case) in cases {
            self.codegen.builder.position_at_end(bb);
            let owner_case_layout = owner_layout.case_layout(owner_case).ok_or_else(|| {
                frontend_error(format!(
                    "wrapper projection 缺少 owner case c{}",
                    owner_case.as_u32()
                ))
            })?;
            let wrapper_case_layout =
                wrapper_layout.case_layout(wrapper_case).ok_or_else(|| {
                    frontend_error(format!(
                        "wrapper projection 缺少 wrapper case c{}",
                        wrapper_case.as_u32()
                    ))
                })?;
            let (payload, continuation) = self.codegen.extract_step_case_parts(
                owner_layout,
                owner_step,
                owner_case_layout,
                "wrapper_project_case_payload",
            )?;
            let projected = self.codegen.build_step_case(
                wrapper_layout,
                wrapper_case_layout,
                payload,
                continuation,
            )?;
            self.codegen.builder.build_return(Some(&projected))?;
        }

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn lower_wrapper_complete_payload(
        &mut self,
        source: &LateLoweredSurfaceResumeWrapperCompletePayloadSource,
        owner_layout: &StepLayout<'ctx>,
        owner_step: BasicValueEnum<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        match source {
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { .. } => {
                self.codegen.extract_step_payload(
                    owner_layout,
                    owner_step,
                    owner_layout.complete_variant(),
                    "wrapper_project_complete_payload",
                )
            }
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
                self.lower_completion_payload(source)
            }
        }
    }

    pub(super) fn try_return_wrapper_complete_from_handle_completion(
        &mut self,
        state: &LateLoweredState,
        target: StateId,
    ) -> Result<bool, LlvmEmitError> {
        let Some(projection) = self.return_projection else {
            return Ok(false);
        };
        let LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            site_id, ..
        } = projection.underlying_route().publication()
        else {
            return Ok(false);
        };

        let mut matched_payload_source = None;
        for candidate in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id: state_site,
                contract,
                ..
            } = candidate.terminator()
            else {
                continue;
            };
            if state_site != site_id {
                continue;
            }
            match contract.state_region(state.state_id()) {
                LateLoweredHandleStateRegion::Body if target == contract.body_complete_target() => {
                    let source = contract.body_completion_payload_source().ok_or_else(|| {
                        frontend_error(format!(
                            "wrapper completion projection 找不到 site{} 的 published body completion payload source",
                            site_id.as_u32()
                        ))
                    })?;
                    matched_payload_source = Some(source);
                    break;
                }
                LateLoweredHandleStateRegion::Arm {
                    handled_case: region_case,
                    arm_ordinal: region_ordinal,
                } if target == contract.arm_complete_target() => {
                    let LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(
                        payload_source,
                    ) = projection.complete().payload_source()
                    else {
                        return Ok(false);
                    };
                    let arm = contract
                        .handled_arms()
                        .iter()
                        .find(|arm| {
                            arm.arm_ordinal() == region_ordinal
                                && arm.handled_case() == region_case
                        })
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "wrapper completion projection 找不到 site{} arm#{} case c{} 的 published arm contract",
                                site_id.as_u32(),
                                region_ordinal,
                                region_case.as_u32()
                            ))
                        })?;
                    if !same_completion_payload_source_ignoring_span(
                        arm.completion_payload_source(),
                        payload_source,
                    ) {
                        return Err(frontend_error(format!(
                            "wrapper completion projection payload source drift: published={payload_source:?}, arm={:?}",
                            arm.completion_payload_source()
                        )));
                    }
                    matched_payload_source = Some(arm.completion_payload_source());
                    break;
                }
                _ => continue,
            }
        }

        let Some(payload_source) = matched_payload_source else {
            return Ok(false);
        };
        let wrapper_layout = self
            .abi
            .step_layout(projection.wrapper_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "wrapper projection 缺少 wrapper step schema s{} layout",
                    projection.wrapper_step_schema().as_u32()
                ))
            })?;
        let payload = self.lower_completion_payload(payload_source)?;
        let projected = self.codegen.build_step_complete(wrapper_layout, payload)?;
        self.sync_frame_slots_from_locals()?;
        self.codegen.builder.build_return(Some(&projected))?;
        Ok(true)
    }

    pub(super) fn try_return_handle_completion_from_resume_entry(
        &mut self,
        state: &LateLoweredState,
        target: StateId,
    ) -> Result<bool, LlvmEmitError> {
        if self.handle_completion_mode != HandleCompletionMode::ReturnFromFunction {
            return Ok(false);
        }
        let mut matched_payload_source = None;
        for candidate in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = candidate.terminator()
            else {
                continue;
            };
            let is_surface_resume_handle = self
                .surface_resume_handle_sites
                .as_ref()
                .is_some_and(|sites| sites.contains(site_id));
            if let Some(surface_handle_sites) = &self.surface_resume_handle_sites
                && !surface_handle_sites.contains(site_id)
            {
                continue;
            }
            if contract.needs_completion_state() && !is_surface_resume_handle {
                continue;
            }
            let payload_source = match contract.state_region(state.state_id()) {
                LateLoweredHandleStateRegion::Body if target == contract.body_complete_target() => {
                    contract.body_completion_payload_source().ok_or_else(|| {
                        frontend_error(format!(
                            "resume entry handle body st{} 缺少 body completion payload source",
                            state.state_id().as_u32()
                        ))
                    })?
                }
                LateLoweredHandleStateRegion::Arm {
                    handled_case,
                    arm_ordinal,
                } if target == contract.arm_complete_target() => contract
                    .handled_arms()
                    .iter()
                    .find(|arm| {
                        arm.arm_ordinal() == arm_ordinal && arm.handled_case() == handled_case
                    })
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "resume entry handle arm st{} 缺少 arm#{} case c{} completion payload source",
                            state.state_id().as_u32(),
                            arm_ordinal,
                            handled_case.as_u32()
                        ))
                    })?
                    .completion_payload_source(),
                _ => continue,
            };
            if matched_payload_source
                .replace(payload_source.clone())
                .is_some()
            {
                return Err(frontend_error(format!(
                    "resume entry state st{} -> st{} 命中多个 handle completion return contract",
                    state.state_id().as_u32(),
                    target.as_u32(),
                )));
            }
        }
        let Some(payload_source) = matched_payload_source else {
            return Ok(false);
        };
        self.return_handle_completion_payload(payload_source)
    }

    pub(super) fn cleanup_handle_contexts_before_function_return(
        &mut self,
        state_id: StateId,
        complete_state: StateId,
    ) -> Result<(), LlvmEmitError> {
        let mut handles = Vec::<(usize, SiteId)>::new();
        for handle_state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = handle_state.terminator()
            else {
                continue;
            };
            let returns_past_handle = match contract.state_region(state_id) {
                LateLoweredHandleStateRegion::Body => {
                    complete_state != contract.body_complete_target()
                }
                LateLoweredHandleStateRegion::Arm { .. } => {
                    complete_state != contract.arm_complete_target()
                }
                LateLoweredHandleStateRegion::Finally => contract
                    .finally_complete_target()
                    .is_some_and(|finally_complete| complete_state != finally_complete),
                LateLoweredHandleStateRegion::OutsideHandle
                | LateLoweredHandleStateRegion::Dispatch
                | LateLoweredHandleStateRegion::Exit => false,
            };
            if !returns_past_handle {
                continue;
            }
            handles.push((
                self.handle_dispatch_nesting_depth(handle_state.state_id()),
                *site_id,
            ));
        }

        handles.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.as_u32().cmp(&right.1.as_u32()))
        });
        for pair in handles.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(frontend_error(format!(
                    "return state st{} 命中多个同层 HandleDispatch return cleanup contract",
                    state_id.as_u32()
                )));
            }
        }
        for (_, site_id) in handles {
            self.restore_and_clear_handle_effect_ctx_slots(
                site_id,
                "handle_return_restore_ctx",
                "handle_return_clear_ctx",
            )?;
        }
        Ok(())
    }

    pub(super) fn return_handle_completion_payload(
        &mut self,
        owner_payload_source: LateLoweredCompletionPayloadSource,
    ) -> Result<bool, LlvmEmitError> {
        if let Some(projection) = self.return_projection {
            let wrapper_layout = self
                .abi
                .step_layout(projection.wrapper_step_schema())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "wrapper projection 缺少 wrapper step schema s{} layout",
                        projection.wrapper_step_schema().as_u32()
                    ))
                })?;
            let payload = match projection.complete().payload_source() {
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { .. } => self
                    .lower_completion_payload_as(
                        &owner_payload_source,
                        wrapper_layout.complete_variant().payload_source_ty(),
                    )?,
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
                    self.lower_completion_payload_as(
                        source,
                        wrapper_layout.complete_variant().payload_source_ty(),
                    )?
                }
            };
            let payload = self.complete_payload_or_default(wrapper_layout, payload)?;
            let projected = self.codegen.build_step_complete(wrapper_layout, payload)?;
            self.sync_frame_slots_from_locals()?;
            self.codegen.builder.build_return(Some(&projected))?;
        } else {
            match self.return_mode {
                CallableReturnMode::EffectOutcome => {
                    let outcome = self
                        .build_complete_effect_outcome_from_payload_source(&owner_payload_source)?;
                    self.emit_effect_outcome_return(outcome)?;
                }
                _ => {
                    let payload = self.lower_completion_payload_as(
                        &owner_payload_source,
                        self.step_layout.complete_variant().payload_source_ty(),
                    )?;
                    let payload = self.complete_payload_or_default(self.step_layout, payload)?;
                    let step = self
                        .codegen
                        .build_step_complete(self.step_layout, payload)?;
                    self.return_step(step)?;
                }
            }
        }
        Ok(true)
    }

    pub(super) fn complete_payload_or_default(
        &mut self,
        step_layout: &StepLayout<'ctx>,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if payload.is_some() || step_layout.complete_variant().payload_is_elided() {
            return Ok(payload);
        }
        let payload_ty = step_layout.complete_variant().payload_source_ty();
        let payload_cg =
            self.codegen
                .cg_ty_of(payload_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "default Complete payload type",
                    at: self.mir_fun.span.into(),
                })?;
        Ok(self
            .codegen
            .default_value(self.mir_fun.span, payload_cg)?
            .value)
    }

    pub(super) fn try_route_handle_completion_goto(
        &mut self,
        state: &LateLoweredState,
        target: StateId,
    ) -> Result<bool, LlvmEmitError> {
        let Some(action) = self.handle_goto_action(state.state_id(), target)? else {
            return Ok(false);
        };
        match action {
            HandleGotoRuntimeAction::RestoreSavedCtxAndGoto {
                clear_slots,
                site_id,
                target,
            } => {
                if clear_slots {
                    self.restore_and_clear_handle_effect_ctx_slots(
                        site_id,
                        "handle_direct_exit_ctx",
                        "handle_direct_exit_ctx_clear",
                    )?;
                } else {
                    self.restore_handle_saved_effect_ctx(site_id, "handle_direct_exit_ctx")?;
                }
                self.branch_to_state(target)?;
            }
            HandleGotoRuntimeAction::BeginCompletion(action) => {
                self.begin_handle_pending_completion(action, None)?;
            }
            HandleGotoRuntimeAction::FinishFinally(finally) => {
                self.finish_handle_finally_completion(finally)?;
            }
        }
        Ok(true)
    }

    pub(super) fn handle_goto_action(
        &self,
        state_id: StateId,
        target: StateId,
    ) -> Result<Option<HandleGotoRuntimeAction>, LlvmEmitError> {
        let mut matched = None;
        for candidate in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = candidate.terminator()
            else {
                continue;
            };
            let layout =
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
            let has_continuation_binder = contract
                .handled_arms()
                .iter()
                .any(|arm| arm.continuation_binder().is_some());
            let action = match contract.state_region(state_id) {
                LateLoweredHandleStateRegion::Body if target == contract.body_complete_target() => {
                    if contract.needs_completion_state() {
                        self.handle_begin_completion_action(layout, *site_id)?
                            .map(HandleGotoRuntimeAction::BeginCompletion)
                    } else if has_continuation_binder {
                        Some(HandleGotoRuntimeAction::RestoreSavedCtxAndGoto {
                            clear_slots: false,
                            site_id: *site_id,
                            target,
                        })
                    } else {
                        Some(HandleGotoRuntimeAction::RestoreSavedCtxAndGoto {
                            clear_slots: true,
                            site_id: *site_id,
                            target,
                        })
                    }
                }
                LateLoweredHandleStateRegion::Arm { .. }
                    if target == contract.arm_complete_target() =>
                {
                    if contract.needs_completion_state() {
                        self.handle_begin_completion_action(layout, *site_id)?
                            .map(HandleGotoRuntimeAction::BeginCompletion)
                    } else if has_continuation_binder {
                        Some(HandleGotoRuntimeAction::RestoreSavedCtxAndGoto {
                            clear_slots: false,
                            site_id: *site_id,
                            target,
                        })
                    } else {
                        Some(HandleGotoRuntimeAction::RestoreSavedCtxAndGoto {
                            clear_slots: true,
                            site_id: *site_id,
                            target,
                        })
                    }
                }
                LateLoweredHandleStateRegion::Finally
                    if Some(target) == contract.finally_complete_target() =>
                {
                    Some(HandleGotoRuntimeAction::FinishFinally(
                        self.handle_finally_runtime(layout, *site_id)?,
                    ))
                }
                _ => None,
            };
            let Some(action) = action else {
                continue;
            };
            if matched.replace(action).is_some() {
                return Err(frontend_error(format!(
                    "state st{} -> st{} 命中多个 HandleDispatch completion contract",
                    state_id.as_u32(),
                    target.as_u32()
                )));
            }
        }
        Ok(matched)
    }

    pub(super) fn handle_begin_completion_action(
        &self,
        layout: &super::super::types::HandleDispatchLayout,
        site_id: SiteId,
    ) -> Result<Option<HandlePendingCompletionRuntime>, LlvmEmitError> {
        let contract = layout.lowered_contract();
        if !contract.needs_completion_state() {
            return Ok(None);
        }
        let completion = self.handle_completion_mode.pending_completion();
        let completion_tag_value = layout.completion_tag_value(completion).ok_or_else(|| {
            frontend_error(format!(
                "HandleDispatch site{} 缺少 completion tag {:?}",
                site_id.as_u32(),
                completion
            ))
        })?;
        let finally_state = handle_finally_state(contract).ok_or_else(|| {
            frontend_error(format!(
                "HandleDispatch site{} 需要 completion state 但缺少 finally region",
                site_id.as_u32()
            ))
        })?;
        Ok(Some(HandlePendingCompletionRuntime {
            site_id,
            completion,
            completion_tag_value,
            completion_tag_field_index: layout.completion_tag_field_index(),
            finally_state,
            payload_transport: None,
        }))
    }

    pub(super) fn handle_boundary_action(
        &self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
    ) -> Result<Option<HandleBoundaryRuntimeAction>, LlvmEmitError> {
        self.handle_boundary_action_excluding(boundary_id, case_tag, None)
    }

    pub(super) fn handle_boundary_action_excluding(
        &self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
        excluded_site: Option<SiteId>,
    ) -> Result<Option<HandleBoundaryRuntimeAction>, LlvmEmitError> {
        let mut matched = None::<(usize, HandleBoundaryRuntimeAction)>;
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = state.terminator()
            else {
                continue;
            };
            if excluded_site.is_some_and(|excluded| excluded == *site_id) {
                continue;
            }
            let Some(routing) = contract.boundary_routing(boundary_id) else {
                continue;
            };
            let Some(case) = routing.case_routing(case_tag) else {
                continue;
            };
            let layout =
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
            let action = match case.action() {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    arm_ordinal,
                    ..
                } => {
                    let arm = layout
                        .handled_arms()
                        .iter()
                        .find(|arm| arm.arm_ordinal() == arm_ordinal)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "HandleDispatch site{} boundary bd{} case c{} 缺少 arm#{} layout",
                                site_id.as_u32(),
                                boundary_id.as_u32(),
                                case_tag.as_u32(),
                                arm_ordinal
                            ))
                        })?;
                    if arm.arm_state() != arm_state {
                        return Err(frontend_error(format!(
                            "HandleDispatch site{} boundary bd{} case c{} arm state 漂移：routing=st{} layout=st{}",
                            site_id.as_u32(),
                            boundary_id.as_u32(),
                            case_tag.as_u32(),
                            arm_state.as_u32(),
                            arm.arm_state().as_u32()
                        )));
                    }
                    HandleBoundaryRuntimeAction::ConsumeToArm(HandleConsumeArmRuntime {
                        site_id: *site_id,
                        arm_ordinal,
                        arm_state,
                        payload_binders: arm.payload_binders().to_vec(),
                        continuation_binder: arm.continuation_binder(),
                    })
                }
                LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion } => {
                    let origin = LateLoweredHandlePendingCompletionOrigin::new(
                        completion,
                        routing.boundary_id(),
                        routing.owner_state(),
                        routing.resume_state(),
                    );
                    let completion_tag_value = layout.pending_completion_origin_tag_value(origin).ok_or_else(|| {
                        frontend_error(format!(
                            "HandleDispatch site{} boundary bd{} case c{} 缺少 pending completion origin tag {:?}",
                            site_id.as_u32(),
                            boundary_id.as_u32(),
                            case_tag.as_u32(),
                            origin
                        ))
                    })?;
                    let finally_state = handle_finally_state(contract).ok_or_else(|| {
                        frontend_error(format!(
                            "HandleDispatch site{} boundary bd{} pending completion 缺少 finally region",
                            site_id.as_u32(),
                            boundary_id.as_u32()
                        ))
                    })?;
                    HandleBoundaryRuntimeAction::PendingCompletion(HandlePendingCompletionRuntime {
                        site_id: *site_id,
                        completion,
                        completion_tag_value,
                        completion_tag_field_index: layout.completion_tag_field_index(),
                        finally_state,
                        payload_transport: layout.pending_payload_transport_layout(completion).map(
                            |transport| HandlePendingPayloadRuntime {
                                completion: transport.completion(),
                                payload_tuple_ty: transport.payload_tuple_ty(),
                                frame_field_index: transport.frame_field_index(),
                            },
                        ),
                    })
                }
                LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => {
                    HandleBoundaryRuntimeAction::EmitOutward
                }
            };
            let action = if self.surface_resume_handle_sites.is_some()
                && !self.surface_resume_allows_handle_dispatch(*site_id, state.state_id())
                && !matches!(action, HandleBoundaryRuntimeAction::EmitOutward)
            {
                HandleBoundaryRuntimeAction::EmitOutward
            } else {
                action
            };
            let depth = self.handle_dispatch_nesting_depth(state.state_id());
            match (&matched, &action) {
                (None, _) => matched = Some((depth, action)),
                (Some((_, HandleBoundaryRuntimeAction::EmitOutward)), _)
                    if !matches!(action, HandleBoundaryRuntimeAction::EmitOutward) =>
                {
                    matched = Some((depth, action))
                }
                (_, HandleBoundaryRuntimeAction::EmitOutward) => {}
                (Some((matched_depth, _)), _) if depth > *matched_depth => {
                    matched = Some((depth, action))
                }
                (Some((matched_depth, _)), _) if depth < *matched_depth => {}
                (Some(_), _) => {
                    return Err(frontend_error(format!(
                        "boundary bd{} case c{} 命中多个 HandleDispatch routing contract",
                        boundary_id.as_u32(),
                        case_tag.as_u32()
                    )));
                }
            }
        }
        Ok(matched.map(|(_, action)| action))
    }

    pub(super) fn handle_boundary_dispatch_candidates_excluding(
        &self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
        excluded_site: Option<SiteId>,
    ) -> Result<Vec<HandleBoundaryDispatchCandidate>, LlvmEmitError> {
        let mut candidates = Vec::new();
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = state.terminator()
            else {
                continue;
            };
            if excluded_site.is_some_and(|excluded| excluded == *site_id) {
                continue;
            }
            let Some(routing) = contract.boundary_routing(boundary_id) else {
                continue;
            };
            let Some(case) = routing.case_routing(case_tag) else {
                continue;
            };
            let layout =
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
            let LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                arm_state,
                arm_ordinal,
                ..
            } = case.action()
            else {
                continue;
            };
            let handled_arm = layout.handled_arm_by_ordinal(arm_ordinal).ok_or_else(|| {
                frontend_error(format!(
                    "HandleDispatch site{} boundary bd{} case c{} 缺少 arm ordinal #{} 的 handled arm layout",
                    site_id.as_u32(),
                    boundary_id.as_u32(),
                    case_tag.as_u32(),
                    arm_ordinal,
                ))
            })?;
            if handled_arm.arm_ordinal() != arm_ordinal {
                return Err(frontend_error(format!(
                    "HandleDispatch site{} boundary bd{} case c{} arm ordinal 漂移：routing=#{} layout=#{}",
                    site_id.as_u32(),
                    boundary_id.as_u32(),
                    case_tag.as_u32(),
                    arm_ordinal,
                    handled_arm.arm_ordinal(),
                )));
            }
            if handled_arm.arm_state() != arm_state {
                return Err(frontend_error(format!(
                    "HandleDispatch site{} boundary bd{} case c{} arm state 漂移：routing=st{} layout=st{}",
                    site_id.as_u32(),
                    boundary_id.as_u32(),
                    case_tag.as_u32(),
                    arm_state.as_u32(),
                    handled_arm.arm_state().as_u32(),
                )));
            }
            let action = HandleBoundaryRuntimeAction::ConsumeToArm(HandleConsumeArmRuntime {
                site_id: *site_id,
                arm_ordinal,
                arm_state,
                payload_binders: handled_arm.payload_binders().to_vec(),
                continuation_binder: handled_arm.continuation_binder(),
            });
            let action = if self.surface_resume_handle_sites.is_some()
                && !self.surface_resume_allows_handle_dispatch(*site_id, state.state_id())
                && !matches!(action, HandleBoundaryRuntimeAction::EmitOutward)
            {
                HandleBoundaryRuntimeAction::EmitOutward
            } else {
                action
            };
            if matches!(action, HandleBoundaryRuntimeAction::EmitOutward) {
                continue;
            }
            candidates.push(HandleBoundaryDispatchCandidate {
                dispatch_identity: self
                    .codegen
                    .effect_handler_dispatch_identity(*site_id, arm_ordinal),
                action,
            });
        }
        Ok(candidates)
    }

    pub(super) fn surface_resume_allows_handle_dispatch(
        &self,
        site_id: SiteId,
        dispatch_state: StateId,
    ) -> bool {
        let Some(surface_sites) = self.surface_resume_handle_sites.as_ref() else {
            return true;
        };
        if surface_sites.contains(&site_id) {
            return true;
        }

        self.callable.state_graph().states().iter().any(|state| {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id: parent_site,
                contract,
                ..
            } = state.terminator()
            else {
                return false;
            };
            surface_sites.contains(parent_site)
                && handle_dispatch_region_implies_runtime_nesting(
                    contract.state_region(dispatch_state),
                )
        })
    }
}
