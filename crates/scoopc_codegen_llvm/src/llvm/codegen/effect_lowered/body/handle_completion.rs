//! Handle pending-completion lifecycle: handle finally runtime, the begin/finish steps for pending completions, payload-binder stores, and the handle pending-payload / completion-tag transport slots.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn handle_dispatch_nesting_depth(&self, dispatch_state: StateId) -> usize {
        self.callable
            .state_graph()
            .states()
            .iter()
            .filter(|state| state.state_id() != dispatch_state)
            .filter_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch { contract, .. } => Some(contract),
                _ => None,
            })
            .filter(|contract| {
                handle_dispatch_region_implies_runtime_nesting(
                    contract.state_region(dispatch_state),
                )
            })
            .count()
    }

    pub(super) fn handle_finally_runtime(
        &self,
        layout: &super::super::types::HandleDispatchLayout,
        site_id: SiteId,
    ) -> Result<HandleFinallyRuntime, LlvmEmitError> {
        let contract = layout.lowered_contract();
        let exit_state = contract.finally_complete_target().ok_or_else(|| {
            frontend_error(format!(
                "HandleDispatch site{} finally region 缺少 complete target",
                site_id.as_u32()
            ))
        })?;
        let continue_to_exit_tag = layout
            .completion_tag_value(LateLoweredHandlePendingCompletion::ContinueToExit)
            .ok_or_else(|| {
                frontend_error(format!(
                    "HandleDispatch site{} 缺少 ContinueToExit completion tag",
                    site_id.as_u32()
                ))
            })?;
        let return_from_function_tag = layout
            .completion_tag_value(LateLoweredHandlePendingCompletion::ReturnFromFunction)
            .ok_or_else(|| {
                frontend_error(format!(
                    "HandleDispatch site{} 缺少 ReturnFromFunction completion tag",
                    site_id.as_u32()
                ))
            })?;
        let return_payload_source = handle_finally_return_payload_source(contract)?;
        let mut propagate_outward = Vec::new();
        for origin in contract.pending_completion_origins() {
            let LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) =
                origin.completion()
            else {
                continue;
            };
            let emission = contract.outward_emission(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "HandleDispatch site{} pending outward c{} 缺少 outward emission",
                    site_id.as_u32(),
                    case_tag.as_u32()
                ))
            })?;
            let completion_tag_value = layout
                .pending_completion_origin_tag_value(*origin)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "HandleDispatch site{} 缺少 pending outward origin tag {:?}",
                        site_id.as_u32(),
                        origin
                    ))
                })?;
            let boundary_id = self.handle_boundary_for_site(site_id)?.boundary_id();
            propagate_outward.push(HandleOutwardCompletionRuntime {
                boundary_id,
                completion_tag_value,
                case_tag,
                payload_tuple_ty: emission.payload_tuple_ty(),
                resume_state: origin.resume_state(),
                payload_transport: layout
                    .pending_payload_transport_layout(origin.completion())
                    .map(|transport| HandlePendingPayloadRuntime {
                        completion: transport.completion(),
                        payload_tuple_ty: transport.payload_tuple_ty(),
                        frame_field_index: transport.frame_field_index(),
                    }),
            });
        }
        Ok(HandleFinallyRuntime {
            site_id,
            completion_tag_field_index: layout.completion_tag_field_index(),
            exit_state,
            continue_to_exit_tag,
            return_from_function_tag,
            return_payload_source,
            propagate_outward,
        })
    }

    pub(super) fn begin_handle_pending_completion(
        &mut self,
        action: HandlePendingCompletionRuntime,
        payload: Option<(Option<BasicValueEnum<'ctx>>, TypeId)>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(transport) = action.payload_transport {
            let Some((payload, payload_ty)) = payload else {
                return Err(frontend_error(format!(
                    "HandleDispatch pending completion {:?} 需要 payload transport，但当前 completion 没有 payload",
                    action.completion
                )));
            };
            self.store_handle_pending_payload(transport, payload, payload_ty)?;
        } else if let Some((payload, payload_ty)) = payload {
            let payload_layout = self.abi.source_value_layout(payload_ty)?;
            if payload.is_some() || !payload_layout.abi().is_elided() {
                return Err(frontend_error(format!(
                    "HandleDispatch pending completion {:?} 缺少 published payload transport for t{}",
                    action.completion,
                    payload_ty.as_u32()
                )));
            }
        }
        self.store_handle_completion_tag(
            action.completion_tag_field_index,
            action.completion_tag_value,
        )?;
        self.branch_to_state(action.finally_state)
    }

    pub(super) fn finish_handle_finally_completion(
        &mut self,
        finally: HandleFinallyRuntime,
    ) -> Result<(), LlvmEmitError> {
        let tag = self.load_handle_completion_tag(finally.completion_tag_field_index)?;
        let function = self.function;
        let invalid_bb = self.codegen.context.append_basic_block(
            function,
            &format!("handle{}_invalid_completion", finally.site_id.as_u32()),
        );
        let (normal_tag, normal_bb) = match self.handle_completion_mode {
            HandleCompletionMode::ContinueToExit => (
                finally.continue_to_exit_tag,
                self.codegen.context.append_basic_block(
                    function,
                    &format!("handle{}_continue_exit", finally.site_id.as_u32()),
                ),
            ),
            HandleCompletionMode::ReturnFromFunction => (
                finally.return_from_function_tag,
                self.codegen.context.append_basic_block(
                    function,
                    &format!("handle{}_return_function", finally.site_id.as_u32()),
                ),
            ),
        };
        let mut cases = vec![(
            tag.get_type().const_int(u64::from(normal_tag), false),
            normal_bb,
        )];
        let mut outward_blocks = Vec::new();
        for outward in &finally.propagate_outward {
            let bb = self.codegen.context.append_basic_block(
                function,
                &format!(
                    "handle{}_propagate_c{}_st{}",
                    finally.site_id.as_u32(),
                    outward.case_tag.as_u32(),
                    outward.resume_state.as_u32()
                ),
            );
            cases.push((
                tag.get_type()
                    .const_int(u64::from(outward.completion_tag_value), false),
                bb,
            ));
            outward_blocks.push((*outward, bb));
        }
        self.codegen.builder.build_switch(tag, invalid_bb, &cases)?;

        self.codegen.builder.position_at_end(normal_bb);
        self.restore_and_clear_handle_effect_ctx_slots(
            finally.site_id,
            "handle_finally_normal_ctx",
            "handle_finally_normal_ctx_clear",
        )?;
        match self.handle_completion_mode {
            HandleCompletionMode::ContinueToExit => {
                self.branch_to_state(finally.exit_state)?;
            }
            HandleCompletionMode::ReturnFromFunction => {
                let payload_source = finally.return_payload_source.as_ref().ok_or_else(|| {
                    frontend_error(format!(
                        "HandleDispatch site{} ReturnFromFunction 缺少 finally completion payload source",
                        finally.site_id.as_u32(),
                        ))
                })?;
                match self.return_mode {
                    CallableReturnMode::EffectOutcome => {
                        let outcome =
                            self.build_complete_effect_outcome_from_payload_source(payload_source)?;
                        self.emit_effect_outcome_return(outcome)?;
                    }
                    _ => {
                        let step = self.build_complete_step_from_payload_source(payload_source)?;
                        self.return_step(step)?;
                    }
                }
            }
        }

        for (outward, bb) in outward_blocks {
            self.codegen.builder.position_at_end(bb);
            self.restore_and_clear_handle_effect_ctx_slots(
                finally.site_id,
                "handle_finally_outward_ctx",
                "handle_finally_outward_ctx_clear",
            )?;
            let payload = self.load_handle_pending_payload(outward.payload_transport)?;
            let boundary = self
                .callable
                .boundary_map()
                .boundary(outward.boundary_id)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "HandleDispatch site{} pending outward c{} 引用了不存在的 handle boundary bd{}",
                        finally.site_id.as_u32(),
                        outward.case_tag.as_u32(),
                        outward.boundary_id.as_u32(),
                    ))
                })?;
            let Some(LateLoweredBoundaryLowering::Handle(_)) = boundary.lowering() else {
                return Err(frontend_error(format!(
                    "HandleDispatch site{} pending outward c{} 的 boundary bd{} 不是 Handle lowering",
                    finally.site_id.as_u32(),
                    outward.case_tag.as_u32(),
                    outward.boundary_id.as_u32(),
                )));
            };
            let continuation = self.create_continuation_object(
                outward.resume_state,
                outward.case_tag,
                None,
                None,
            )?;
            self.emit_or_consume_outward_case(
                boundary,
                outward.case_tag,
                payload,
                outward.payload_tuple_ty,
                Some(continuation),
                None,
            )?;
        }

        self.codegen.builder.position_at_end(invalid_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    pub(super) fn handle_boundary_for_site(
        &self,
        site_id: SiteId,
    ) -> Result<&'a LateLoweredBoundary, LlvmEmitError> {
        let mut found = None;
        for boundary in self.callable.boundary_map().entries() {
            let LateLoweredBoundarySource::Site {
                site_id: boundary_site_id,
                kind: BoundarySiteKind::Handle,
            } = boundary.source()
            else {
                continue;
            };
            if boundary_site_id != site_id {
                continue;
            }
            let Some(LateLoweredBoundaryLowering::Handle(_)) = boundary.lowering() else {
                return Err(frontend_error(format!(
                    "HandleDispatch site{} 对应 boundary bd{} 不是 Handle lowering",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                )));
            };
            if found.replace(boundary).is_some() {
                return Err(frontend_error(format!(
                    "HandleDispatch site{} 命中多个 Handle boundary",
                    site_id.as_u32(),
                )));
            }
        }
        found.ok_or_else(|| {
            frontend_error(format!(
                "HandleDispatch site{} 缺少对应 Handle boundary",
                site_id.as_u32(),
            ))
        })
    }

    pub(super) fn build_complete_step_from_payload_source(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let payload = self.lower_completion_payload_as(
            payload_source,
            self.step_layout.complete_variant().payload_source_ty(),
        )?;
        if payload.is_none() && !self.step_layout.complete_variant().payload_is_elided() {
            return Err(frontend_error(format!(
                "HandleDispatch completion payload {:?} produced no payload for non-elided Complete layout {}",
                payload_source,
                self.step_layout.complete_variant().payload_anchor_name()
            )));
        }
        self.codegen.build_step_complete(self.step_layout, payload)
    }

    pub(super) fn store_case_payload_to_arm_binders(
        &mut self,
        binders: &[HandlePayloadBinderLayout],
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        if let [binder] = binders
            && self
                .lir_body
                .locals()
                .get(binder.local().as_u32() as usize)
                .is_some_and(|local| local.ty() == payload_ty)
        {
            if let Some(raw) = payload {
                let _ = self.store_loaded_raw_local(self.mir_fun.span, binder.local(), raw)?;
                if let Some(frame_slot) = binder.frame_slot() {
                    self.store_local_to_frame_slot(binder.local(), frame_slot)?;
                }
                return Ok(());
            }
            if !self.abi.source_value_layout(payload_ty)?.abi().is_elided() {
                return Err(frontend_error(format!(
                    "handle arm payload binder local{} 需要完整 non-elided payload t{}，但 boundary lowering 未提供 payload",
                    binder.local().as_u32(),
                    payload_ty.as_u32(),
                )));
            }
            return Ok(());
        }
        for binder in binders {
            let value = self.unpack_payload_field(payload, payload_ty, binder.ordinal())?;
            if let Some(raw) = value {
                let _ = self.store_loaded_raw_local(self.mir_fun.span, binder.local(), raw)?;
                if let Some(frame_slot) = binder.frame_slot() {
                    self.store_local_to_frame_slot(binder.local(), frame_slot)?;
                }
            } else if !self.payload_field_is_elided(payload_ty, binder.ordinal())? {
                return Err(frontend_error(format!(
                    "handle arm payload binder local{} ordinal {} 需要 non-elided payload t{}，但 boundary lowering 未提供 payload",
                    binder.local().as_u32(),
                    binder.ordinal(),
                    payload_ty.as_u32()
                )));
            }
        }
        Ok(())
    }

    pub(super) fn payload_field_is_elided(
        &self,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<bool, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(true);
        }
        match layout.kind() {
            SourceAbiLayoutKind::Scalar => Ok(false),
            SourceAbiLayoutKind::Tuple => layout
                .field(ordinal as usize)
                .map(|field| field.is_elided())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "payload tuple t{} 缺少 ordinal {}",
                        payload_ty.as_u32(),
                        ordinal
                    ))
                }),
        }
    }

    pub(super) fn store_gc_ref_to_binder(
        &mut self,
        binder: HandleContinuationBinderLayout,
        value: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_ref_to_local(binder.local(), value)?;
        if let Some(frame_slot) = binder.frame_slot() {
            self.store_local_to_frame_slot(binder.local(), frame_slot)?;
        }
        Ok(())
    }

    pub(super) fn store_handle_pending_payload(
        &mut self,
        transport: HandlePendingPayloadRuntime,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        if transport.payload_tuple_ty != payload_ty {
            return Err(frontend_error(format!(
                "HandleDispatch pending payload transport {:?} 类型漂移：transport=t{} payload=t{}",
                transport.completion,
                transport.payload_tuple_ty.as_u32(),
                payload_ty.as_u32()
            )));
        }
        let Some(payload) = payload else {
            let payload_layout = self.abi.source_value_layout(payload_ty)?;
            if payload_layout.abi().is_elided() {
                return Ok(());
            }
            return Err(frontend_error(format!(
                "HandleDispatch pending payload transport {:?} 需要 non-elided payload t{}",
                transport.completion,
                payload_ty.as_u32()
            )));
        };
        let field_ptr = self.frame_field_ptr(
            transport.frame_field_index,
            "handle_pending_payload_store_gep",
        )?;
        self.codegen.store_gc_aware_value(
            self.mir_fun.span,
            field_ptr,
            payload,
            "handle_pending_payload_store",
        )?;
        Ok(())
    }

    pub(super) fn load_handle_pending_payload(
        &mut self,
        transport: Option<HandlePendingPayloadRuntime>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let Some(transport) = transport else {
            return Ok(None);
        };
        let field_ty = self.frame_field_type(transport.frame_field_index)?;
        let field_ptr = self.frame_field_ptr(
            transport.frame_field_index,
            "handle_pending_payload_load_gep",
        )?;
        Ok(Some(self.codegen.builder.build_load(
            field_ty,
            field_ptr,
            "handle_pending_payload",
        )?))
    }

    pub(super) fn store_handle_completion_tag(
        &mut self,
        field_index: u32,
        tag_value: u32,
    ) -> Result<(), LlvmEmitError> {
        let field_ty = self.frame_field_type(field_index)?;
        let BasicTypeEnum::IntType(int_ty) = field_ty else {
            return Err(frontend_error(format!(
                "HandleDispatch completion tag field {field_index} 不是 integer"
            )));
        };
        let field_ptr = self.frame_field_ptr(field_index, "handle_completion_tag_gep")?;
        self.codegen
            .builder
            .build_store(field_ptr, int_ty.const_int(u64::from(tag_value), false))?;
        Ok(())
    }

    pub(super) fn load_handle_completion_tag(
        &mut self,
        field_index: u32,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let field_ty = self.frame_field_type(field_index)?;
        let BasicTypeEnum::IntType(int_ty) = field_ty else {
            return Err(frontend_error(format!(
                "HandleDispatch completion tag field {field_index} 不是 integer"
            )));
        };
        let field_ptr = self.frame_field_ptr(field_index, "handle_completion_tag_gep")?;
        Ok(self
            .codegen
            .builder
            .build_load(int_ty, field_ptr, "handle_completion_tag")?
            .into_int_value())
    }
}
