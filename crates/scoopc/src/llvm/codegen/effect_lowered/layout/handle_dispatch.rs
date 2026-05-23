//! Handle-dispatch ABI: arm layouts, pending-payload transports, region
//! routings, and runtime-error contracts.
//!
//! Handle dispatch routes effect operations to the most-recently-installed
//! handler arm at runtime. This module materializes the per-region arm
//! layouts, the pending payload transports that carry suspend payloads
//! between regions, the per-handle pending completion origins, and the
//! runtime-error terminal actions used when a handler aborts.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_local_runtime_error_contracts(
        &mut self,
    ) -> Result<BTreeMap<(StepSchemaId, SiteId), LocalRuntimeErrorContract<'ctx>>, LlvmEmitError>
    {
        let mut contracts = BTreeMap::new();
        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            for boundary in callable.boundary_map().entries() {
                let (
                    LateLoweredBoundarySource::Site {
                        site_id,
                        kind: BoundarySiteKind::Call,
                    },
                    Some(LateLoweredBoundaryLowering::Call(lowering)),
                ) = (boundary.source(), boundary.lowering())
                else {
                    continue;
                };
                let Some(contract) = lowering.consumed_runtime_error_case() else {
                    continue;
                };
                let Some(target_state) = callable
                    .state_graph()
                    .states()
                    .iter()
                    .find(|state| state.state_id() == contract.target_state())
                else {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 缺少 callable `{}` call site {} local runtime-error target state st{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.target_state().as_u32(),
                    )));
                };
                let terminal_action = match target_state.terminator() {
                    LateLoweredStateTerminator::LocalRuntimeError {
                        payload_tuple_ty,
                        terminal_action,
                    } if *payload_tuple_ty == contract.payload_tuple_ty()
                        && *terminal_action == contract.terminal_action() =>
                    {
                        *terminal_action
                    }
                    LateLoweredStateTerminator::LocalRuntimeError {
                        payload_tuple_ty,
                        terminal_action,
                    } => {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` call site {} 的 local runtime-error target state st{} contract 漂移：state_graph=(payload_tuple_ty=t{}, terminal_action={terminal_action:?})，boundary lowering=(payload_tuple_ty=t{}, terminal_action={:?})",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            contract.target_state().as_u32(),
                            payload_tuple_ty.as_u32(),
                            contract.payload_tuple_ty().as_u32(),
                            contract.terminal_action(),
                        )));
                    }
                    other => {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` call site {} 的 local runtime-error target state st{} 不是 LocalRuntimeError terminator，而是 {other:?}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            contract.target_state().as_u32(),
                        )));
                    }
                };
                let key = (callable.step_schema(), site_id);
                if contracts.contains_key(&key) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 owner step schema {} call site {} 的 local runtime-error contract 重复发布",
                        callable.step_schema().as_u32(),
                        site_id.as_u32(),
                    )));
                }
                let payload_layout = self.source_value_layout(contract.payload_tuple_ty())?;
                let payload_abi = *payload_layout.abi();
                let terminal_action = self.materialize_local_runtime_error_terminal_action(
                    terminal_action,
                    payload_abi,
                )?;
                contracts.insert(
                    key,
                    LocalRuntimeErrorContract::new(
                        callable.step_schema(),
                        site_id,
                        contract.input_case_tag(),
                        contract.payload_tuple_ty(),
                        payload_abi,
                        terminal_action,
                        contract.target_state(),
                    ),
                );
            }
        }
        Ok(contracts)
    }

    pub(super) fn materialize_handle_dispatch_layouts(
        &mut self,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
        continuation_layouts: &BTreeMap<
            crate::effect_lowered::ir::ContinuationObjectId,
            ContinuationObjectLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<BTreeMap<(StepSchemaId, SiteId), HandleDispatchLayout>, LlvmEmitError> {
        let mut layouts = BTreeMap::new();
        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            let handle_states = callable
                .state_graph()
                .states()
                .iter()
                .filter(|state| {
                    matches!(
                        state.terminator(),
                        LateLoweredStateTerminator::HandleDispatch { .. }
                    )
                })
                .collect::<Vec<_>>();
            if handle_states.is_empty() {
                continue;
            }
            let frame_layout = frame_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` 的 frame layout，无法发布 HandleDispatch contract",
                    callable.root_fqn(),
                ))
            })?;
            let state_tag_field_index = frame_layout
                .field_index_for_system(SystemSlotKind::StateTag)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` 的 frame layout 缺少 StateTag system field，无法发布 HandleDispatch contract",
                        callable.root_fqn(),
                    ))
                })?;
            let completion_tag_field_index = frame_layout
                .field_index_for_system(SystemSlotKind::CompletionTag)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` 的 frame layout 缺少 CompletionTag system field，无法发布 HandleDispatch contract",
                        callable.root_fqn(),
                    ))
                })?;
            let payload_carrier_field_index = frame_layout
                .field_index_for_system(SystemSlotKind::ResumePayloadCarrier)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` 的 frame layout 缺少 ResumePayloadCarrier system field，无法发布 HandleDispatch contract",
                        callable.root_fqn(),
                    ))
                })?;

            for state in handle_states {
                let LateLoweredStateTerminator::HandleDispatch {
                    site_id,
                    body_state,
                    arm_states,
                    finally_state,
                    exit_state,
                    contract,
                    drop_state,
                    ..
                } = state.terminator()
                else {
                    continue;
                };

                let expected_complete_target = finally_state.unwrap_or(*exit_state);
                if contract.body_complete_target() != expected_complete_target {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 body complete target 漂移：contract=st{}，state_graph=st{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.body_complete_target().as_u32(),
                        expected_complete_target.as_u32(),
                    )));
                }
                if contract.arm_complete_target() != expected_complete_target {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 arm complete target 漂移：contract=st{}，state_graph=st{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.arm_complete_target().as_u32(),
                        expected_complete_target.as_u32(),
                    )));
                }
                if contract.finally_complete_target() != finally_state.map(|_| *exit_state) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 finally complete target 漂移：contract={:?}，state_graph={:?}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.finally_complete_target(),
                        finally_state.map(|_| *exit_state),
                    )));
                }
                if contract.abandon_target() != *drop_state {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 abandon target 漂移：contract={:?}，state_graph={:?}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.abandon_target(),
                        drop_state,
                    )));
                }
                if contract.handled_arms().len() != arm_states.len() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled-arm 数量({}) 与 state_graph arm 数量({}) 不一致",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.handled_arms().len(),
                        arm_states.len(),
                    )));
                }
                let mut published_arm_ordinals = BTreeSet::new();
                for arm in contract.handled_arms() {
                    let arm_ordinal = arm.arm_ordinal() as usize;
                    let Some(expected_state) = arm_states.get(arm_ordinal) else {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} 引用了越界 arm ordinal {}（state_graph arm 数量={})",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            arm.arm_ordinal(),
                            arm_states.len(),
                        )));
                    };
                    if !published_arm_ordinals.insert(arm.arm_ordinal()) {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 arm ordinal {}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.arm_ordinal(),
                        )));
                    }
                    if arm.arm_state() != *expected_state {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} arm state 漂移：contract=st{}，state_graph=st{}（ordinal={})",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            arm.arm_state().as_u32(),
                            expected_state.as_u32(),
                            arm.arm_ordinal(),
                        )));
                    }
                }
                let published_handled_arms = self.materialize_published_handle_arm_layouts(
                    callable,
                    *site_id,
                    contract,
                    frame_layout,
                    continuation_layouts,
                    surface_resume_layouts,
                )?;

                let expected_outward_cases = collect_handle_contract_total_outward_cases(contract);
                let published_outward_cases = contract
                    .outward_emissions()
                    .iter()
                    .map(|emission| emission.case_tag())
                    .collect::<BTreeSet<_>>();
                if !published_outward_cases.is_subset(&expected_outward_cases) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 outward emission 包含未在 HandleDispatch contract 中声明的 case：contract={}，emissions={}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        render_case_tags(&expected_outward_cases),
                        render_case_tags(&published_outward_cases),
                    )));
                }

                let expected_pending_outward = if finally_state.is_some() {
                    collect_handle_contract_pending_outward_cases(contract)
                } else {
                    BTreeSet::new()
                };
                let published_pending_outward = contract
                    .pending_completions()
                    .iter()
                    .filter_map(|pending| match pending {
                        LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => {
                            Some(*case_tag)
                        }
                        LateLoweredHandlePendingCompletion::ContinueToExit
                        | LateLoweredHandlePendingCompletion::ReturnFromFunction => None,
                    })
                    .collect::<BTreeSet<_>>();
                if expected_pending_outward != published_pending_outward {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending outward completion 集合漂移：contract={}，pending={}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        render_case_tags(&expected_pending_outward),
                        render_case_tags(&published_pending_outward),
                    )));
                }

                if finally_state.is_some() {
                    for required in [
                        LateLoweredHandlePendingCompletion::ContinueToExit,
                        LateLoweredHandlePendingCompletion::ReturnFromFunction,
                    ] {
                        if !contract.pending_completions().contains(&required) {
                            return Err(frontend_error(format!(
                                "LLVM ABI materialization 发现 callable `{}` handle site {} 缺少 required pending completion {:?}",
                                callable.root_fqn(),
                                site_id.as_u32(),
                                required,
                            )));
                        }
                    }
                } else if !contract.pending_completions().is_empty() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 没有 finally state，却发布了 pending completion {:?}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.pending_completions(),
                    )));
                }

                let expected_state_regions = build_expected_handle_state_regions(
                    callable.root_fqn(),
                    *site_id,
                    callable.state_graph(),
                    state.state_id(),
                    *body_state,
                    contract,
                    *finally_state,
                    *exit_state,
                )?;
                validate_published_handle_state_regions(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                    &expected_state_regions,
                )?;
                let expected_boundary_routings = build_expected_handle_boundary_routings(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                    &expected_state_regions,
                    callable.boundary_map(),
                )?;
                validate_published_handle_boundary_routings(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                    &expected_boundary_routings,
                )?;
                validate_published_handle_pending_completion_origins(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                )?;

                let mut completion_tags = BTreeMap::new();
                let mut pending_completion_origin_tags = BTreeMap::new();
                let mut next_completion_tag = 1u32;
                for pending in contract.pending_completions() {
                    if matches!(
                        pending,
                        LateLoweredHandlePendingCompletion::PropagateOutward(_)
                    ) {
                        continue;
                    }
                    if completion_tags
                        .insert(*pending, next_completion_tag)
                        .is_some()
                    {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 pending completion {:?}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            pending,
                        )));
                    }
                    next_completion_tag = next_completion_tag.saturating_add(1);
                }
                for origin in contract.pending_completion_origins() {
                    let LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) =
                        origin.completion()
                    else {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending origin {:?} 不是 outward completion",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            origin,
                        )));
                    };
                    if contract.outward_emission(case_tag).is_none() {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending completion origin {:?} 缺少 outward emission",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            origin,
                        )));
                    }
                    if pending_completion_origin_tags
                        .insert(*origin, next_completion_tag)
                        .is_some()
                    {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 pending completion origin {:?}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            origin,
                        )));
                    }
                    next_completion_tag = next_completion_tag.saturating_add(1);
                }
                let pending_payload_transports = self
                    .materialize_published_handle_pending_payload_transports(
                        callable,
                        *site_id,
                        contract,
                        frame_layout,
                    )?;

                let key = (callable.step_schema(), *site_id);
                if layouts.contains_key(&key) {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 owner step schema s{} handle site {} 的 HandleDispatch contract 重复发布",
                        callable.step_schema().as_u32(),
                        site_id.as_u32(),
                    )));
                }
                layouts.insert(
                    key,
                    HandleDispatchLayout::new(
                        callable.step_schema(),
                        *site_id,
                        contract.clone(),
                        state_tag_field_index,
                        completion_tag_field_index,
                        payload_carrier_field_index,
                        completion_tags,
                        pending_completion_origin_tags,
                        pending_payload_transports,
                        published_handled_arms,
                    ),
                );
            }
        }
        Ok(layouts)
    }

    pub(super) fn materialize_published_handle_arm_layouts(
        &self,
        callable: &LateLoweredCallable,
        site_id: SiteId,
        contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
        frame_layout: &FrameLayout<'ctx>,
        continuation_layouts: &BTreeMap<
            crate::effect_lowered::ir::ContinuationObjectId,
            ContinuationObjectLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            ContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<Vec<HandleArmLayout>, LlvmEmitError> {
        let mut layouts = Vec::with_capacity(contract.handled_arms().len());
        for arm in contract.handled_arms() {
            let mut payload_binders = Vec::with_capacity(arm.payload_binders().len());
            for (expected_ordinal, binder) in arm.payload_binders().iter().enumerate() {
                if binder.ordinal() != expected_ordinal as u32 {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload binder ordinal 漂移：contract=#{}，expected=#{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        arm.handled_case().as_u32(),
                        binder.ordinal(),
                        expected_ordinal,
                    )));
                }
                let frame_field_index = match binder.frame_slot() {
                    Some(frame_slot) => Some(frame_layout.field_index_for_slot(frame_slot).ok_or_else(
                        || frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload binder #{} 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            expected_ordinal,
                            frame_slot.as_u32(),
                        )),
                    )?),
                    None => None,
                };
                payload_binders.push(HandlePayloadBinderLayout::new(
                    binder.ordinal(),
                    binder.local(),
                    binder.frame_slot(),
                    frame_field_index,
                ));
            }

            let continuation_binder = match arm.continuation_binder() {
                Some(binder) => {
                    if binder.continuation_object() != callable.continuation_object() {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation object 漂移：contract=ko{}，owner=ko{}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_object().as_u32(),
                            callable.continuation_object().as_u32(),
                        )));
                    }
                    let continuation_layout = continuation_layouts.get(&binder.continuation_object()).ok_or_else(
                        || frontend_error(format!(
                            "LLVM ABI materialization 缺少 callable `{}` handle site {} 的 handled case c{} continuation object ko{} layout",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_object().as_u32(),
                        )),
                    )?;
                    let surface_layout = surface_resume_layouts
                        .get(&binder.continuation_schema())
                        .ok_or_else(|| frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation binder 缺少 continuation schema k{} 的 authoritative surface-resume dispatch inventory",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_schema().as_u32(),
                        )))?;
                    if matches!(
                        surface_layout.dispatch_source_kind(),
                        crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
                            | crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable
                    ) {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation schema k{} dispatch source kind 为 {:?}，无法作为 authoritative handle-binder surface source",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_schema().as_u32(),
                            surface_layout.dispatch_source_kind(),
                        )));
                    }
                    let _ = continuation_layout;
                    let frame_field_index = match binder.frame_slot() {
                        Some(frame_slot) => Some(frame_layout.field_index_for_slot(frame_slot).ok_or_else(
                            || frontend_error(format!(
                                "LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation binder 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                                callable.root_fqn(),
                                site_id.as_u32(),
                                arm.handled_case().as_u32(),
                                frame_slot.as_u32(),
                            )),
                        )?),
                        None => None,
                    };
                    Some(HandleContinuationBinderLayout::new(
                        binder.local(),
                        binder.frame_slot(),
                        frame_field_index,
                        binder.continuation_schema(),
                        binder.continuation_object(),
                        surface_layout.dispatch_source_kind(),
                        surface_layout.return_step_schema(),
                    ))
                }
                None => None,
            };

            layouts.push(HandleArmLayout::new(
                arm.handled_case(),
                arm.arm_state(),
                arm.arm_ordinal(),
                arm.payload_tuple_ty(),
                payload_binders,
                continuation_binder,
                arm.arm_outward_cases().to_vec(),
            ));
        }

        Ok(layouts)
    }

    pub(super) fn materialize_published_handle_pending_payload_transports(
        &self,
        callable: &LateLoweredCallable,
        site_id: SiteId,
        contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
        frame_layout: &FrameLayout<'ctx>,
    ) -> Result<
        BTreeMap<LateLoweredHandlePendingCompletion, HandlePendingPayloadTransportLayout>,
        LlvmEmitError,
    > {
        let expected_pending_cases = contract
            .pending_completions()
            .iter()
            .filter_map(|pending| match pending {
                LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => Some(*case_tag),
                LateLoweredHandlePendingCompletion::ContinueToExit
                | LateLoweredHandlePendingCompletion::ReturnFromFunction => None,
            })
            .collect::<BTreeSet<_>>();
        let mut published_pending_cases = BTreeSet::new();
        let mut layouts = BTreeMap::new();

        for transport in contract.pending_payload_transports() {
            let completion = transport.completion();
            let case_tag = match completion {
                LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => case_tag,
                LateLoweredHandlePendingCompletion::ContinueToExit
                | LateLoweredHandlePendingCompletion::ReturnFromFunction => {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 为 {:?} 发布了 pending payload transport；只有 PropagateOutward(case) 才允许发布 typed payload transport",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        completion,
                    )));
                }
            };
            if !contract.pending_completions().contains(&completion) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport {:?} 没有对应的 pending completion",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    completion,
                )));
            }
            let emission = contract.outward_emission(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 缺少 outward emission",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                ))
            })?;
            if emission.payload_tuple_ty() != transport.payload_tuple_ty() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} payload tuple ty 漂移：transport=t{}，outward emission=t{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                    transport.payload_tuple_ty().as_u32(),
                    emission.payload_tuple_ty().as_u32(),
                )));
            }
            let slot = callable
                .frame_schema()
                .slots()
                .iter()
                .find(|slot| slot.slot_id() == transport.frame_slot())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 引用了不存在的 frame slot fs{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        case_tag.as_u32(),
                        transport.frame_slot().as_u32(),
                    ))
            })?;
            if slot.kind() != (LateLoweredFrameSlotKind::HandlePendingPayload { site_id, case_tag })
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 引用的 frame slot fs{} kind 漂移：published={:?}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                    transport.frame_slot().as_u32(),
                    slot.kind(),
                )));
            }
            if slot.ty() != transport.payload_tuple_ty() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} frame slot fs{} 类型漂移：slot=t{}，transport=t{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                    transport.frame_slot().as_u32(),
                    slot.ty().as_u32(),
                    transport.payload_tuple_ty().as_u32(),
                )));
            }
            let frame_field_index = frame_layout
                .field_index_for_slot(transport.frame_slot())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        case_tag.as_u32(),
                        transport.frame_slot().as_u32(),
                    ))
                })?;
            if layouts
                .insert(
                    completion,
                    HandlePendingPayloadTransportLayout::new(*transport, frame_field_index),
                )
                .is_some()
            {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 pending payload transport {:?}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    completion,
                )));
            }
            published_pending_cases.insert(case_tag);
        }

        if published_pending_cases != expected_pending_cases {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport 集合漂移：published={}，expected={}",
                callable.root_fqn(),
                site_id.as_u32(),
                render_case_tags(&published_pending_cases),
                render_case_tags(&expected_pending_cases),
            )));
        }

        Ok(layouts)
    }

    pub(super) fn materialize_local_runtime_error_terminal_action(
        &mut self,
        action: LateLoweredLocalRuntimeErrorTerminalAction,
        payload_abi: AbiValue<'ctx>,
    ) -> Result<LocalRuntimeErrorTerminalAction<'ctx>, LlvmEmitError> {
        match action {
            LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal { runtime_entry } => {
                Ok(LocalRuntimeErrorTerminalAction::RuntimeFatal {
                    runtime_entry: self
                        .materialize_published_runtime_entry(runtime_entry, payload_abi)?,
                })
            }
        }
    }

    pub(super) fn materialize_published_runtime_entry(
        &mut self,
        runtime_entry: LateLoweredPublishedRuntimeEntry,
        payload_abi: AbiValue<'ctx>,
    ) -> Result<PublishedRuntimeEntryLayout<'ctx>, LlvmEmitError> {
        match runtime_entry {
            LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal => {
                if payload_abi.is_elided() {
                    return Err(frontend_error(
                        "LLVM ABI materialization 不允许把 local runtime-error payload 退化成零载荷 runtime fatal contract"
                            .to_string(),
                    ));
                }
                self.codegen.declare_runtime_error_fatal();
                let params: [BasicMetadataTypeEnum<'ctx>; 1] = [payload_abi.llvm_ty().into()];
                let llvm_ty = self.codegen.context.void_type().fn_type(&params, false);
                Ok(PublishedRuntimeEntryLayout::new(
                    runtime_entry,
                    runtime_entry.symbol_name().to_string(),
                    llvm_ty,
                    params.len(),
                ))
            }
        }
    }
}

fn collect_handle_contract_total_outward_cases(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> BTreeSet<crate::effect_facts::CaseTag> {
    let mut tags = contract
        .body_outward_cases()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for arm in contract.handled_arms() {
        tags.extend(arm.arm_outward_cases().iter().copied());
    }
    tags.extend(contract.finally_outward_cases().iter().copied());
    tags
}

fn collect_handle_contract_pending_outward_cases(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> BTreeSet<crate::effect_facts::CaseTag> {
    let emitted_cases = contract
        .outward_emissions()
        .iter()
        .map(|emission| emission.case_tag())
        .collect::<BTreeSet<_>>();
    let mut tags = contract
        .body_outward_cases()
        .iter()
        .copied()
        .filter(|case_tag| emitted_cases.contains(case_tag))
        .collect::<BTreeSet<_>>();
    for arm in contract.handled_arms() {
        tags.extend(
            arm.arm_outward_cases()
                .iter()
                .copied()
                .filter(|case_tag| emitted_cases.contains(case_tag)),
        );
    }
    tags
}

#[allow(clippy::too_many_arguments)]
fn build_expected_handle_state_regions(
    owner_root_fqn: &str,
    site_id: SiteId,
    state_graph: &crate::effect_lowered::ir::LateLoweredStateGraph,
    dispatch_state: crate::effect_lowered::ir::StateId,
    body_state: crate::effect_lowered::ir::StateId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    finally_state: Option<crate::effect_lowered::ir::StateId>,
    exit_state: crate::effect_lowered::ir::StateId,
) -> Result<
    BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
    LlvmEmitError,
> {
    let mut regions = BTreeMap::new();
    insert_expected_handle_state_region(
        owner_root_fqn,
        site_id,
        &mut regions,
        dispatch_state,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Dispatch,
    )?;
    insert_expected_handle_state_region(
        owner_root_fqn,
        site_id,
        &mut regions,
        exit_state,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Exit,
    )?;

    let mut stop_states = BTreeSet::from([dispatch_state, exit_state]);
    stop_states.extend(
        contract
            .handled_arms()
            .iter()
            .map(crate::effect_lowered::ir::LateLoweredHandleArmDispatch::arm_state),
    );
    if let Some(finally_state) = finally_state {
        stop_states.insert(finally_state);
    }

    for state_id in collect_expected_handle_region_states(
        owner_root_fqn,
        site_id,
        state_graph,
        body_state,
        &stop_states,
    )? {
        insert_expected_handle_state_region(
            owner_root_fqn,
            site_id,
            &mut regions,
            state_id,
            crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body,
        )?;
    }

    for arm in contract.handled_arms() {
        let mut arm_stops = stop_states.clone();
        arm_stops.remove(&arm.arm_state());
        let region = crate::effect_lowered::ir::LateLoweredHandleStateRegion::Arm {
            handled_case: arm.handled_case(),
            arm_ordinal: arm.arm_ordinal(),
        };
        for state_id in collect_expected_handle_region_states(
            owner_root_fqn,
            site_id,
            state_graph,
            arm.arm_state(),
            &arm_stops,
        )? {
            insert_expected_handle_state_region(
                owner_root_fqn,
                site_id,
                &mut regions,
                state_id,
                region,
            )?;
        }
    }

    if let Some(finally_state) = finally_state {
        let mut finally_stops = stop_states;
        finally_stops.remove(&finally_state);
        for state_id in collect_expected_handle_region_states(
            owner_root_fqn,
            site_id,
            state_graph,
            finally_state,
            &finally_stops,
        )? {
            insert_expected_handle_state_region(
                owner_root_fqn,
                site_id,
                &mut regions,
                state_id,
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Finally,
            )?;
        }
    }

    Ok(regions)
}

fn collect_expected_handle_region_states(
    owner_root_fqn: &str,
    site_id: SiteId,
    state_graph: &crate::effect_lowered::ir::LateLoweredStateGraph,
    entry_state: crate::effect_lowered::ir::StateId,
    stop_states: &BTreeSet<crate::effect_lowered::ir::StateId>,
) -> Result<BTreeSet<crate::effect_lowered::ir::StateId>, LlvmEmitError> {
    if state_graph.state(entry_state).is_none() {
        return Err(frontend_error(format!(
            "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 region root st{} 不存在于 state graph 中",
            site_id.as_u32(),
            entry_state.as_u32(),
        )));
    }

    let mut visited = BTreeSet::new();
    let mut worklist = vec![entry_state];
    while let Some(state_id) = worklist.pop() {
        if stop_states.contains(&state_id) || !visited.insert(state_id) {
            continue;
        }
        let state = state_graph.state(state_id).ok_or_else(|| {
            frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 region traversal 命中了不存在的 state st{}",
                site_id.as_u32(),
                state_id.as_u32(),
            ))
        })?;
        worklist.extend(state.successors().iter().rev().copied());
    }
    Ok(visited)
}

fn insert_expected_handle_state_region(
    owner_root_fqn: &str,
    site_id: SiteId,
    regions: &mut BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
    state_id: crate::effect_lowered::ir::StateId,
    region: crate::effect_lowered::ir::LateLoweredHandleStateRegion,
) -> Result<(), LlvmEmitError> {
    match regions.insert(state_id, region) {
        Some(existing) if existing != region => Err(frontend_error(format!(
            "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 state st{} 同时归属于 {:?} 和 {:?}",
            site_id.as_u32(),
            state_id.as_u32(),
            existing,
            region,
        ))),
        Some(_) | None => Ok(()),
    }
}

fn validate_published_handle_state_regions(
    owner_root_fqn: &str,
    site_id: SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    expected_regions: &BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
) -> Result<(), LlvmEmitError> {
    let mut published = BTreeMap::new();
    for entry in contract.state_regions() {
        if published.insert(entry.state_id(), entry.region()).is_some() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 published state region 重复声明 st{}",
                site_id.as_u32(),
                entry.state_id().as_u32(),
            )));
        }
    }
    if &published != expected_regions {
        return Err(frontend_error(format!(
            "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 state-region contract 漂移：published={published:?}，state_graph={expected_regions:?}",
            site_id.as_u32(),
        )));
    }
    Ok(())
}

fn build_expected_handle_boundary_routings(
    owner_root_fqn: &str,
    site_id: SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    expected_regions: &BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
    boundary_map: &crate::effect_lowered::ir::LateLoweredBoundaryMap,
) -> Result<
    BTreeMap<
        crate::effect_lowered::ir::BoundaryId,
        crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting,
    >,
    LlvmEmitError,
> {
    let handled_arms = contract
        .handled_arms()
        .iter()
        .map(|arm| (arm.handled_case(), arm))
        .collect::<BTreeMap<_, _>>();
    let body_outward_cases = contract
        .body_outward_cases()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let finally_outward_cases = contract
        .finally_outward_cases()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let outward_emission_cases = contract
        .outward_emissions()
        .iter()
        .map(|emission| emission.case_tag())
        .collect::<BTreeSet<_>>();
    let pending_outward_cases = contract
        .pending_completions()
        .iter()
        .filter_map(|pending| match pending {
            crate::effect_lowered::ir::LateLoweredHandlePendingCompletion::PropagateOutward(
                case_tag,
            ) => Some((*case_tag, *pending)),
            crate::effect_lowered::ir::LateLoweredHandlePendingCompletion::ContinueToExit
            | crate::effect_lowered::ir::LateLoweredHandlePendingCompletion::ReturnFromFunction => {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    let mut routes = BTreeMap::new();

    for boundary in boundary_map.entries() {
        let owner_region = expected_regions
            .get(&boundary.owner_state())
            .copied()
            .unwrap_or(crate::effect_lowered::ir::LateLoweredHandleStateRegion::OutsideHandle);
        if matches!(
            owner_region,
            crate::effect_lowered::ir::LateLoweredHandleStateRegion::OutsideHandle
                | crate::effect_lowered::ir::LateLoweredHandleStateRegion::Exit
        ) {
            continue;
        }
        if matches!(
            owner_region,
            crate::effect_lowered::ir::LateLoweredHandleStateRegion::Dispatch
        ) && !matches!(
            boundary.source(),
            crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                site_id: boundary_site,
                kind: crate::effect_lowered::ir::BoundarySiteKind::Handle,
            } if boundary_site == site_id
        ) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 dispatch boundary bd{} source 漂移：{:?}",
                site_id.as_u32(),
                boundary.boundary_id().as_u32(),
                boundary.source(),
            )));
        }
        let case_tags =
            collect_expected_handle_boundary_case_tags(owner_root_fqn, site_id, boundary)?;
        let case_routings = case_tags
            .into_iter()
            .map(|case_tag| {
                build_expected_handle_boundary_case_routing(
                    owner_root_fqn,
                    site_id,
                    boundary,
                    owner_region,
                    case_tag,
                    &handled_arms,
                    &body_outward_cases,
                    &finally_outward_cases,
                    &outward_emission_cases,
                    &pending_outward_cases,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        routes.insert(
            boundary.boundary_id(),
            crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                boundary.boundary_id(),
                boundary.owner_state(),
                owner_region,
                boundary.resume_state(),
                case_routings,
            ),
        );
    }
    Ok(routes)
}

fn collect_expected_handle_boundary_case_tags(
    owner_root_fqn: &str,
    site_id: SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
) -> Result<Vec<crate::effect_facts::CaseTag>, LlvmEmitError> {
    let lowering = boundary.lowering().ok_or_else(|| {
        frontend_error(format!(
            "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary bd{} 缺少 lowering，无法校验 routing contract",
            site_id.as_u32(),
            boundary.boundary_id().as_u32(),
        ))
    })?;
    let mut tags = BTreeSet::new();
    let raw_tags: Vec<crate::effect_facts::CaseTag> = match lowering {
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Call(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::ClassCtor(lowering) => lowering
            .emitted_steps()
            .iter()
            .map(|emission| emission.case_tag())
            .collect(),
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Perform(lowering) => {
            vec![lowering.emitted_step().case_tag()]
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Resume(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            vec![lowering.emitted_step().case_tag()]
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Handle(lowering) => lowering
            .outward_emissions()
            .iter()
            .map(|emission| emission.case_tag())
            .collect(),
    };
    for case_tag in raw_tags {
        if !tags.insert(case_tag) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary bd{} 重复发布 case c{}，无法校验稳定 routing",
                site_id.as_u32(),
                boundary.boundary_id().as_u32(),
                case_tag.as_u32(),
            )));
        }
    }
    Ok(tags.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn build_expected_handle_boundary_case_routing(
    owner_root_fqn: &str,
    site_id: SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    owner_region: crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    case_tag: crate::effect_facts::CaseTag,
    handled_arms: &BTreeMap<
        crate::effect_facts::CaseTag,
        &crate::effect_lowered::ir::LateLoweredHandleArmDispatch,
    >,
    body_outward_cases: &BTreeSet<crate::effect_facts::CaseTag>,
    finally_outward_cases: &BTreeSet<crate::effect_facts::CaseTag>,
    outward_emission_cases: &BTreeSet<crate::effect_facts::CaseTag>,
    pending_outward_cases: &BTreeMap<
        crate::effect_facts::CaseTag,
        crate::effect_lowered::ir::LateLoweredHandlePendingCompletion,
    >,
) -> Result<crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting, LlvmEmitError> {
    let action = match owner_region {
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body => {
            if let Some(arm) = handled_arms.get(&case_tag) {
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state: arm.arm_state(),
                    arm_ordinal: arm.arm_ordinal(),
                    continuation_resume_state: boundary.resume_state(),
                }
            } else if body_outward_cases.contains(&case_tag) {
                pending_outward_cases.get(&case_tag).copied().map_or(
                    crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                    |completion| crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion },
                )
            } else if finally_outward_cases.contains(&case_tag) {
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
            } else {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 body boundary bd{} 发布了未声明的 case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Arm {
            handled_case,
            arm_ordinal,
        } => {
            let arm = handled_arms.get(&handled_case).ok_or_else(|| frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 arm region(c{}, ordinal={}) 缺少 handled-arm contract",
                site_id.as_u32(),
                handled_case.as_u32(),
                arm_ordinal,
            )))?;
            if arm.arm_ordinal() != arm_ordinal {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 arm region(c{}, ordinal={}) 与 handled-arm ordinal {} 不一致",
                    site_id.as_u32(),
                    handled_case.as_u32(),
                    arm_ordinal,
                    arm.arm_ordinal(),
                )));
            }
            if !arm.arm_outward_cases().contains(&case_tag) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 arm boundary bd{} 发布了未声明的 case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            pending_outward_cases.get(&case_tag).copied().map_or(
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                |completion| crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion },
            )
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Finally => {
            if !finally_outward_cases.contains(&case_tag) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 finally boundary bd{} 发布了未声明的 case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Dispatch => {
            if !outward_emission_cases.contains(&case_tag) {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 dispatch boundary bd{} 发布了未声明的 outward emission case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Exit
        | crate::effect_lowered::ir::LateLoweredHandleStateRegion::OutsideHandle => {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary bd{} owner state st{} 不属于当前 handle region",
                site_id.as_u32(),
                boundary.boundary_id().as_u32(),
                boundary.owner_state().as_u32(),
            )));
        }
    };
    Ok(crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(case_tag, action))
}

fn validate_published_handle_boundary_routings(
    owner_root_fqn: &str,
    site_id: SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    expected_routes: &BTreeMap<
        crate::effect_lowered::ir::BoundaryId,
        crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting,
    >,
) -> Result<(), LlvmEmitError> {
    let mut published = BTreeMap::new();
    for routing in contract.boundary_routings() {
        if published
            .insert(routing.boundary_id(), routing.clone())
            .is_some()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 published boundary routing 重复声明 bd{}",
                site_id.as_u32(),
                routing.boundary_id().as_u32(),
            )));
        }
    }
    if &published != expected_routes {
        return Err(frontend_error(format!(
            "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary-routing contract 漂移：published={published:?}，expected={expected_routes:?}",
            site_id.as_u32(),
        )));
    }
    Ok(())
}

fn validate_published_handle_pending_completion_origins(
    owner_root_fqn: &str,
    site_id: SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> Result<(), LlvmEmitError> {
    let mut expected = BTreeSet::new();
    for routing in contract.boundary_routings() {
        for case in routing.case_routings() {
            let crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                completion,
            } = case.action()
            else {
                continue;
            };
            expected.insert(
                crate::effect_lowered::ir::LateLoweredHandlePendingCompletionOrigin::new(
                    completion,
                    routing.boundary_id(),
                    routing.owner_state(),
                    routing.resume_state(),
                ),
            );
        }
    }
    let published = contract
        .pending_completion_origins()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if published != expected {
        return Err(frontend_error(format!(
            "LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 pending completion origin contract 漂移：published={published:?}，expected={expected:?}",
            site_id.as_u32(),
        )));
    }
    Ok(())
}
