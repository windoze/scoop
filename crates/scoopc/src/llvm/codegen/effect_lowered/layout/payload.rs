//! Resume- and completion-payload binding layouts plus frame-slot helpers.
//!
//! Each suspend site publishes an inbound resume payload binding and an
//! outbound completion payload binding; this module materializes both and
//! validates that the binding's published frame slot matches the late-
//! lowered contract. Also exposes the common frame-slot lookup utilities
//! used across the boundary / payload validators.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn materialize_resume_payload_binding_layouts(
        &mut self,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
    ) -> Result<
        (
            ResumePayloadBindingLayouts,
            ResumePayloadBindingLayoutsByState,
        ),
        LlvmEmitError,
    > {
        let mut bindings_by_boundary = ResumePayloadBindingLayouts::new();
        let mut bindings_by_state = ResumePayloadBindingLayoutsByState::new();

        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            let frame_layout = frame_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 frame layout，无法发布 resumed local/home contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;

            for boundary in callable.boundary_map().entries() {
                let requires_binding = matches!(
                    boundary.lowering(),
                    Some(
                        LateLoweredBoundaryLowering::Call(_)
                            | LateLoweredBoundaryLowering::Perform(_)
                            | LateLoweredBoundaryLowering::Resume(_)
                            | LateLoweredBoundaryLowering::RuntimeError(_)
                    )
                );
                if requires_binding
                    && callable
                        .frame_schema()
                        .resume_payload_binding(boundary.boundary_id())
                        .is_none()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` boundary bd{} 缺少 resumed local/home contract",
                        callable.root_fqn(),
                        boundary.boundary_id().as_u32(),
                    )));
                }
            }

            for binding in callable.frame_schema().resume_payload_bindings() {
                let frame_field_index =
                    self.validate_resume_payload_binding(callable, frame_layout, binding)?;
                let layout = ResumePayloadBindingLayout::new(
                    callable.step_schema(),
                    *binding,
                    frame_field_index,
                );
                let boundary_key = (callable.step_schema(), binding.boundary_id());
                if bindings_by_boundary.insert(boundary_key, layout).is_some() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 owner step schema s{} boundary bd{} 的 resumed local/home contract 重复发布",
                        callable.step_schema().as_u32(),
                        binding.boundary_id().as_u32(),
                    )));
                }
                let state_key = (callable.step_schema(), binding.resume_state());
                match bindings_by_state.get(&state_key) {
                    Some(existing)
                        if existing.consumer_local() == layout.consumer_local()
                            && existing.consumer_frame_slot() == layout.consumer_frame_slot()
                            && existing.frame_field_index() == layout.frame_field_index() => {}
                    Some(existing) => {
                        return Err(frontend_error(format!(
                            "LLVM ABI materialization 发现 owner step schema s{} resume state st{} 的 resumed local/home contract 冲突：已发布 boundary bd{} -> local{} home={:?}，当前 boundary bd{} -> local{} home={:?}",
                            callable.step_schema().as_u32(),
                            binding.resume_state().as_u32(),
                            existing.boundary_id().as_u32(),
                            existing.consumer_local().as_u32(),
                            existing.consumer_frame_slot(),
                            binding.boundary_id().as_u32(),
                            binding.consumer_local().as_u32(),
                            binding.consumer_frame_slot(),
                        )));
                    }
                    None => {
                        bindings_by_state.insert(state_key, layout);
                    }
                }
            }
        }

        Ok((bindings_by_boundary, bindings_by_state))
    }

    pub(super) fn validate_resume_payload_binding(
        &mut self,
        callable: &LateLoweredCallable,
        frame_layout: &FrameLayout<'ctx>,
        binding: &LateLoweredResumePayloadBinding,
    ) -> Result<Option<u32>, LlvmEmitError> {
        let boundary = callable
            .boundary_map()
            .boundary(binding.boundary_id())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` 的 resumed local/home contract 引用了不存在的 boundary bd{}",
                    callable.root_fqn(),
                    binding.boundary_id().as_u32(),
                ))
            })?;
        if binding.resume_state() != boundary.resume_state() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract resume_state 漂移：published=st{}，boundary=st{}",
                callable.root_fqn(),
                binding.boundary_id().as_u32(),
                binding.resume_state().as_u32(),
                boundary.resume_state().as_u32(),
            )));
        }

        let (expected_local, expected_home_boundary) = match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                (lowering.result_local(), binding.boundary_id())
            }
            Some(LateLoweredBoundaryLowering::Perform(_)) => {
                let (local, _) = Self::published_boundary_result_slot(
                    callable.frame_schema(),
                    binding.boundary_id(),
                )
                .ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` perform boundary bd{} 缺少 BoundaryResult slot，无法校验 resumed local/home contract",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                    ))
                })?;
                (local, binding.boundary_id())
            }
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                (lowering.result_local(), binding.boundary_id())
            }
            Some(LateLoweredBoundaryLowering::RuntimeError(lowering)) => {
                let paired_binding = callable
                    .frame_schema()
                    .resume_payload_binding(lowering.resume_boundary())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` runtime-error boundary bd{} 的 paired resume boundary bd{} 缺少 resumed local/home contract",
                            callable.root_fqn(),
                            binding.boundary_id().as_u32(),
                            lowering.resume_boundary().as_u32(),
                        ))
                    })?;
                if paired_binding.consumer_local() != binding.consumer_local()
                    || paired_binding.consumer_frame_slot() != binding.consumer_frame_slot()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` runtime-error boundary bd{} 的 resumed local/home contract 与 paired resume boundary bd{} 漂移",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        lowering.resume_boundary().as_u32(),
                    )));
                }
                (paired_binding.consumer_local(), lowering.resume_boundary())
            }
            Some(LateLoweredBoundaryLowering::ClassCtor(_))
            | Some(LateLoweredBoundaryLowering::Handle(_))
            | None => {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` boundary bd{} 不应发布 resumed local/home contract",
                    callable.root_fqn(),
                    binding.boundary_id().as_u32(),
                )));
            }
        };

        if binding.consumer_local() != expected_local {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract local 漂移：published=local{}，expected=local{}",
                callable.root_fqn(),
                binding.boundary_id().as_u32(),
                binding.consumer_local().as_u32(),
                expected_local.as_u32(),
            )));
        }

        let boundary_result_slot =
            Self::published_boundary_result_slot(callable.frame_schema(), expected_home_boundary);
        match (boundary_result_slot, binding.consumer_frame_slot()) {
            (Some((slot_local, slot_id)), Some(binding_slot)) => {
                if slot_local != binding.consumer_local() || slot_id != binding_slot {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home slot 漂移：published=slot{}，expected BoundaryResult home=slot{}",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        binding_slot.as_u32(),
                        slot_id.as_u32(),
                    )));
                }
            }
            (Some((_slot_local, slot_id)), None) => {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` boundary bd{} 已有 BoundaryResult home slot{}，但 resumed local/home contract 未发布 frame home",
                    callable.root_fqn(),
                    binding.boundary_id().as_u32(),
                    slot_id.as_u32(),
                )));
            }
            (None, Some(binding_slot)) => {
                let slot = callable
                    .frame_schema()
                    .slots()
                    .iter()
                    .find(|slot| slot.slot_id() == binding_slot)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract 引用了不存在的 frame slot fs{}",
                            callable.root_fqn(),
                            binding.boundary_id().as_u32(),
                            binding_slot.as_u32(),
                        ))
                    })?;
                let Some(slot_local) = Self::frame_slot_local(slot.kind()) else {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract 引用了不能承载 local 的 frame slot fs{} kind={:?}",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        binding_slot.as_u32(),
                        slot.kind(),
                    )));
                };
                if slot_local != binding.consumer_local() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract frame slot fs{} 绑定到了 local{}，但 published local 为 local{}",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        binding_slot.as_u32(),
                        slot_local.as_u32(),
                        binding.consumer_local().as_u32(),
                    )));
                }
            }
            (None, None) => {}
        }

        binding
            .consumer_frame_slot()
            .map(|slot_id| {
                frame_layout
                    .field_index_for_slot(slot_id)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                            callable.root_fqn(),
                            binding.boundary_id().as_u32(),
                            slot_id.as_u32(),
                        ))
                    })
            })
            .transpose()
    }

    pub(super) fn materialize_completion_payload_binding_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, FrameLayout<'ctx>>,
    ) -> Result<CompletionPayloadBindingLayouts<'ctx>, LlvmEmitError> {
        let mut layouts = CompletionPayloadBindingLayouts::new();

        for callable in self.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            let step_type = self.program.step_type(callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 Step shell，无法发布 completion payload contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;
            let step_layout = step_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 Step layout，无法发布 completion payload contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;
            if step_layout.complete_variant().payload_source_ty() != step_type.complete_ty() {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` 的 Step complete variant 类型漂移：layout=t{}，step=t{}",
                    callable.root_fqn(),
                    step_layout.complete_variant().payload_source_ty().as_u32(),
                    step_type.complete_ty().as_u32(),
                )));
            }
            let frame_layout = frame_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 frame layout，无法发布 completion payload contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;

            for state in callable.state_graph().states() {
                if !matches!(
                    state.terminator(),
                    LateLoweredStateTerminator::Return { .. }
                ) {
                    continue;
                }
                if callable
                    .frame_schema()
                    .completion_payload_binding_for_state(state.state_id())
                    .is_none()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` return state st{} 缺少 completion payload contract",
                        callable.root_fqn(),
                        state.state_id().as_u32(),
                    )));
                }
            }

            for binding in callable.frame_schema().completion_payload_bindings() {
                let (payload_abi, frame_field_index) = self.validate_completion_payload_binding(
                    callable,
                    step_type,
                    frame_layout,
                    binding,
                )?;
                let layout = CompletionPayloadBindingLayout::new(
                    callable.step_schema(),
                    binding.clone(),
                    payload_abi,
                    frame_field_index,
                );
                let key = (callable.step_schema(), binding.return_state());
                if layouts.insert(key, layout).is_some() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 owner step schema s{} return state st{} 的 completion payload contract 重复发布",
                        callable.step_schema().as_u32(),
                        binding.return_state().as_u32(),
                    )));
                }
            }
        }

        Ok(layouts)
    }

    pub(super) fn validate_completion_payload_binding(
        &mut self,
        callable: &LateLoweredCallable,
        step_type: &LateLoweredStepType,
        frame_layout: &FrameLayout<'ctx>,
        binding: &LateLoweredCompletionPayloadBinding,
    ) -> Result<(AbiValue<'ctx>, Option<u32>), LlvmEmitError> {
        let state = callable
            .state_graph()
            .states()
            .iter()
            .find(|state| state.state_id() == binding.return_state())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 发现 callable `{}` 的 completion payload contract 引用了不存在的 return state st{}",
                    callable.root_fqn(),
                    binding.return_state().as_u32(),
                ))
            })?;
        let LateLoweredStateTerminator::Return {
            payload_source,
            complete_state,
        } = state.terminator()
        else {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` state st{} 不是 Return，却发布了 completion payload contract",
                callable.root_fqn(),
                binding.return_state().as_u32(),
            )));
        };
        if binding.complete_state() != *complete_state
            || binding.complete_state() != callable.state_graph().complete_state()
        {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` return state st{} 的 complete_state 漂移：binding=st{}，state_graph_return=st{}，callable_complete=st{}",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.complete_state().as_u32(),
                complete_state.as_u32(),
                callable.state_graph().complete_state().as_u32(),
            )));
        }
        if binding.payload_source() != payload_source {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload source 漂移：binding={:?}，state_graph={:?}",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.payload_source(),
                payload_source,
            )));
        }
        if binding.payload_source().source_ty() != step_type.complete_ty() {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload source type t{} 与 StepSchema s{} complete_ty t{} 不一致",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.payload_source().source_ty().as_u32(),
                step_type.step_schema().as_u32(),
                step_type.complete_ty().as_u32(),
            )));
        }
        if matches!(
            binding.payload_source(),
            LateLoweredCompletionPayloadSource::Unit { .. }
        ) && !matches!(
            self.source_types.kind(step_type.complete_ty()),
            TypeKind::Value(ValueTypeKind::Unit)
        ) {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` return state st{} 对 non-Unit complete_ty t{} 发布了 Unit completion payload source",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                step_type.complete_ty().as_u32(),
            )));
        }

        let expected_frame_slot = match binding.payload_source() {
            LateLoweredCompletionPayloadSource::Operand(source) => match source.value() {
                LateLoweredOperandValueSource::Local(local) => {
                    Self::published_frame_slot_for_local(callable.frame_schema(), *local)
                }
                LateLoweredOperandValueSource::Const(_) => None,
            },
            LateLoweredCompletionPayloadSource::Unit { .. } => None,
        };
        if binding.payload_frame_slot() != expected_frame_slot {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload frame home 漂移：binding={:?}，expected={:?}",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.payload_frame_slot(),
                expected_frame_slot,
            )));
        }

        let frame_field_index = binding
            .payload_frame_slot()
            .map(|slot_id| {
                let slot = callable
                    .frame_schema()
                    .slots()
                    .iter()
                    .find(|slot| slot.slot_id() == slot_id)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload contract 引用了不存在的 frame slot fs{}",
                            callable.root_fqn(),
                            binding.return_state().as_u32(),
                            slot_id.as_u32(),
                        ))
                    })?;
                if slot.ty() != binding.payload_source().source_ty() {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload home slot fs{} 类型 t{} 与 payload source type t{} 不一致",
                        callable.root_fqn(),
                        binding.return_state().as_u32(),
                        slot_id.as_u32(),
                        slot.ty().as_u32(),
                        binding.payload_source().source_ty().as_u32(),
                    )));
                }
                frame_layout.field_index_for_slot(slot_id).ok_or_else(|| {
                    frontend_error(format!(
                        "LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload frame slot fs{} 在 frame layout 中缺少对应 field",
                        callable.root_fqn(),
                        binding.return_state().as_u32(),
                        slot_id.as_u32(),
                    ))
                })
            })
            .transpose()?;
        let payload_layout = self.source_value_layout(binding.payload_source().source_ty())?;
        Ok((*payload_layout.abi(), frame_field_index))
    }

    pub(super) fn published_boundary_result_slot(
        frame_schema: &crate::effect_lowered::ir::LateLoweredFrameSchema,
        boundary_id: BoundaryId,
    ) -> Option<(
        crate::effect_lowered::mir_source::LocalId,
        crate::effect_lowered::ir::FrameSlotId,
    )> {
        frame_schema
            .slots()
            .iter()
            .find_map(|slot| match slot.kind() {
                LateLoweredFrameSlotKind::BoundaryResult { boundary, local }
                    if boundary == boundary_id =>
                {
                    Some((local, slot.slot_id()))
                }
                _ => None,
            })
    }

    pub(super) fn published_frame_slot_for_local(
        frame_schema: &crate::effect_lowered::ir::LateLoweredFrameSchema,
        local: crate::effect_lowered::mir_source::LocalId,
    ) -> Option<crate::effect_lowered::ir::FrameSlotId> {
        frame_schema.slots().iter().find_map(|slot| {
            (Self::frame_slot_local(slot.kind()) == Some(local)).then_some(slot.slot_id())
        })
    }

    pub(super) fn frame_slot_local(
        kind: LateLoweredFrameSlotKind,
    ) -> Option<crate::effect_lowered::mir_source::LocalId> {
        match kind {
            LateLoweredFrameSlotKind::SourceLocal(local)
            | LateLoweredFrameSlotKind::CompilerTemporary(local)
            | LateLoweredFrameSlotKind::JoinValue { local, .. }
            | LateLoweredFrameSlotKind::BoundaryResult { local, .. }
            | LateLoweredFrameSlotKind::HandleBinder { local, .. } => Some(local),
            LateLoweredFrameSlotKind::HandleSavedEffectCtx { .. }
            | LateLoweredFrameSlotKind::HandleArmEffectCtx { .. }
            | LateLoweredFrameSlotKind::HandlePendingPayload { .. }
            | LateLoweredFrameSlotKind::ResumePayload { .. }
            | LateLoweredFrameSlotKind::System(_) => None,
        }
    }
}

pub(super) fn same_completion_payload_source_ignoring_span(
    left: &LateLoweredCompletionPayloadSource,
    right: &LateLoweredCompletionPayloadSource,
) -> bool {
    match (left, right) {
        (
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: left_ty,
            },
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: right_ty,
            },
        ) => left_ty == right_ty,
        (
            LateLoweredCompletionPayloadSource::Operand(left),
            LateLoweredCompletionPayloadSource::Operand(right),
        ) => left.source_ty() == right.source_ty() && left.value() == right.value(),
        _ => false,
    }
}
