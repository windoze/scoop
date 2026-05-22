//! Per-callable emission entry points: plain direct emission, the resume-method shell, resume-state dispatch, double-resume runtime errors, and the generated continuation step / driver / outcome wrappers.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn emit_direct(
        mut self,
        entry_layout: &CallableEntryLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.bind_direct_args(entry_layout).map_err(|err| {
            frontend_error(format!(
                "direct entry `{}` bind args failed: {err}",
                entry_layout.symbol_name()
            ))
        })?;
        self.initialize_new_frame()?;
        let entry_state = self.callable.state_graph().entry_state();
        self.branch_to_state(entry_state)?;
        self.emit_states()
    }

    pub(super) fn emit_plain_direct(
        mut self,
        source_types: &TypeStore,
        param_offset: u32,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        self.return_mode = CallableReturnMode::Plain { declared_return_cg };
        self.codegen.bind_lir_source_params(
            self.mir_fun,
            source_types,
            self.function,
            param_offset,
            &mut self.slots,
        )?;
        self.initialize_new_frame()?;
        let entry_state = self.callable.state_graph().entry_state();
        self.branch_to_state(entry_state)?;
        self.emit_states()
    }

    pub(super) fn emit_plain_direct_mir_params(
        mut self,
        param_offset: u32,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        self.return_mode = CallableReturnMode::Plain { declared_return_cg };
        self.codegen.bind_mir_params_without_hir(
            self.mir_fun,
            self.function,
            param_offset,
            &mut self.slots,
        )?;
        self.initialize_new_frame()?;
        let entry_state = self.callable.state_graph().entry_state();
        self.branch_to_state(entry_state)?;
        self.emit_states()
    }

    pub(super) fn emit_resume_method(
        self,
        _case_tag: CaseTag,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.emit_resume_entry(resume_tuple_ty)
    }

    pub(super) fn emit_resume_state_dispatch(
        &mut self,
        resume_state_tag: IntValue<'ctx>,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let invalid_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_invalid_state");
        let mut bindings_by_state = BTreeMap::<StateId, LateLoweredResumePayloadBinding>::new();
        for binding in self.callable.frame_schema().resume_payload_bindings() {
            if !self.resume_payload_binding_accepts_tuple(binding, resume_tuple_ty)? {
                continue;
            }
            if let Some(existing) = bindings_by_state.get(&binding.resume_state()) {
                if existing.consumer_local() != binding.consumer_local()
                    || existing.consumer_frame_slot() != binding.consumer_frame_slot()
                {
                    return Err(frontend_error(format!(
                        "resume entry st{} 的 resumed local/home contract 冲突：bd{} 与 bd{}",
                        binding.resume_state().as_u32(),
                        existing.boundary_id().as_u32(),
                        binding.boundary_id().as_u32()
                    )));
                }
                continue;
            }
            let _ = self
                .abi
                .resume_payload_binding_for_state(self.abi_step_schema, binding.resume_state())?;
            bindings_by_state.insert(binding.resume_state(), *binding);
        }
        let mut cases = Vec::new();
        for binding in bindings_by_state.values().copied() {
            let bb = self.codegen.context.append_basic_block(
                self.function,
                &format!("resume_payload_st{}", binding.resume_state().as_u32()),
            );
            cases.push((
                self.codegen
                    .context
                    .i32_type()
                    .const_int(binding.resume_state().as_u32() as u64, false),
                bb,
                binding,
            ));
        }
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.codegen
            .builder
            .build_switch(resume_state_tag, invalid_bb, &switch_cases)?;
        for (_, bb, binding) in cases {
            self.codegen.builder.position_at_end(bb);
            self.inject_resume_payload(binding, resume_tuple_ty, payload)?;
            self.branch_to_state(binding.resume_state())?;
        }
        self.codegen.builder.position_at_end(invalid_bb);
        self.codegen.builder.build_unreachable()?;
        self.emit_states()
    }

    pub(super) fn emit_resume_entry(
        mut self,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        let cont = self.function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "resume method `{}` 缺少 continuation 参数",
                self.function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let cont_root_slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, "resume_cont_root")?;
        let cont_ptr = self.root_gc_pointer_in_slot(
            cont_root_slot,
            cont.into_pointer_value(),
            "resume_cont_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let captured_frame = self.load_frame_from_continuation(cont_ptr)?;
        self.store_frame_root(captured_frame)?;
        self.restore_frame_slots_to_locals()?;
        let current_effect_ctx = self.load_captured_effect_ctx_from_continuation(cont_ptr)?;
        self.store_current_effect_ctx(current_effect_ctx, "resume_effect_ctx")?;
        let cont_ptr = self.codegen.load_gc_root_slot(
            self.mir_fun.span,
            cont_root_slot,
            "resume_cont_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let payload = if self.function.count_params() > 1 {
            Some(
                self.function
                    .get_nth_param(1)
                    .ok_or_else(|| frontend_error("resume method 缺少 payload 参数".to_string()))?,
            )
        } else {
            None
        };
        let resume_state_tag = self.load_continuation_resume_state(cont_ptr)?;
        let first_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_first");
        let double_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_double");
        let first_resume = self.try_mark_continuation_resumed(cont_ptr, "surface_resume")?;
        self.codegen.builder.build_conditional_branch(
            first_resume,
            first_resume_bb,
            double_resume_bb,
        )?;

        self.codegen.builder.position_at_end(double_resume_bb);
        self.emit_double_resume_runtime_error(resume_state_tag)?;

        self.codegen.builder.position_at_end(first_resume_bb);
        let composed_callee = self.load_captured_callee_suspend_state_ref(cont_ptr)?;
        let composed_is_null = self
            .codegen
            .builder
            .build_is_null(composed_callee, "composed_callee_is_null")?;
        let ordinary_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_plain_dispatch");
        let composed_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_composed_dispatch");
        self.codegen.builder.build_conditional_branch(
            composed_is_null,
            ordinary_resume_bb,
            composed_resume_bb,
        )?;

        self.codegen.builder.position_at_end(composed_resume_bb);
        let handled = self.dispatch_composed_call_boundary_resume(
            resume_state_tag,
            composed_callee,
            resume_tuple_ty,
            payload,
        )?;
        if !handled {
            self.codegen.builder.build_unreachable()?;
        }

        self.codegen.builder.position_at_end(ordinary_resume_bb);
        self.emit_resume_state_dispatch(resume_state_tag, resume_tuple_ty, payload)
    }

    pub(super) fn emit_double_resume_runtime_error_to_ptr(
        &mut self,
        outcome_ptr: PointerValue<'ctx>,
        resume_state_tag: IntValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let (case_tag, payload_tuple_ty) = self.double_resume_runtime_error_case()?;
        let payload = self.lower_runtime_error_boundary_payload(payload_tuple_ty)?;
        let continuation = self.create_continuation_object_with_state_tag(
            None,
            resume_state_tag,
            case_tag,
            None,
            None,
        )?;
        let outcome = self.build_propagating_effect_outcome_for_case(
            case_tag,
            payload,
            payload_tuple_ty,
            continuation,
        )?;
        self.emit_effect_outcome_return_to_ptr(outcome_ptr, outcome)
    }

    pub(super) fn emit_resume_outcome_wrapper(
        mut self,
        core_fun: FunctionValue<'ctx>,
        _resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        let cont = self.function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "outcome resume wrapper `{}` 缺少 continuation 参数",
                self.function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont.into_pointer_value())?;
        let cont_ptr = self.root_gc_pointer(cont_ptr, "outcome_resume_cont_root")?;
        let captured_frame = self.load_frame_from_continuation(cont_ptr)?;
        self.store_frame_root(captured_frame)?;
        let payload = if self.function.count_params() > 2 {
            Some(self.function.get_nth_param(1).ok_or_else(|| {
                frontend_error("outcome resume wrapper 缺少 payload 参数".to_string())
            })?)
        } else {
            None
        };
        let outcome_ptr = self
            .function
            .get_nth_param(self.function.count_params().saturating_sub(1))
            .ok_or_else(|| frontend_error("outcome resume wrapper 缺少 outcome 参数".to_string()))?
            .into_pointer_value();
        let resume_state_tag = self.load_continuation_resume_state(cont_ptr)?;
        let first_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_first");
        let double_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_double");
        let first_resume =
            self.try_mark_continuation_resumed(cont_ptr, "surface_resume_outcome")?;
        self.codegen.builder.build_conditional_branch(
            first_resume,
            first_resume_bb,
            double_resume_bb,
        )?;

        self.codegen.builder.position_at_end(double_resume_bb);
        self.emit_double_resume_runtime_error_to_ptr(outcome_ptr, resume_state_tag)?;

        self.codegen.builder.position_at_end(first_resume_bb);
        self.store_current_state_tag(resume_state_tag, "outcome_resume_state")?;
        let current_effect_ctx = self.load_captured_effect_ctx_from_continuation(cont_ptr)?;
        let incoming_resume_token = self.load_captured_callee_suspend_state_ref(cont_ptr)?;
        let state_ref = self.current_frame_gc_ref("outcome_resume_state_ref")?;
        let mut args = vec![state_ref.into()];
        if let Some(payload) = payload {
            args.push(payload.into());
        }
        args.push(current_effect_ctx.into());
        args.push(incoming_resume_token.into());
        args.push(outcome_ptr.into());
        self.codegen.build_call_preserving_gc_local_roots(
            self.mir_fun.span,
            core_fun,
            &args,
            "outcome_resume_core",
        )?;
        self.codegen.builder.build_return(None)?;
        self.seal_unterminated_state_blocks_as_unreachable()?;
        Ok(())
    }

    pub(super) fn emit_resume_outcome_core(
        mut self,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.return_mode = CallableReturnMode::EffectOutcome;
        let payload_param_index = (self.function.count_params() > 4).then_some(1u32);
        self.codegen.bind_explicit_effect_hidden_abi_slots(
            self.mir_fun.span,
            self.function,
            if payload_param_index.is_some() { 2 } else { 1 },
            true,
        )?;
        let state_ref = self.function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "outcome core `{}` 缺少 state_ref 参数",
                self.function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let state_ref = self.codegen.cast_ptr(
            state_ref.into_pointer_value(),
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "outcome_core_state_ref",
        )?;
        self.store_frame_root(state_ref)?;
        self.restore_frame_slots_to_locals()?;
        let current_effect_ctx =
            self.codegen
                .function_cx
                .current_effect_ctx_ref
                .ok_or_else(|| {
                    frontend_error("outcome core 缺少 current_effect_ctx_ref".to_string())
                })?;
        self.store_current_effect_ctx(current_effect_ctx, "outcome_core_effect_ctx")?;
        let resume_state_tag = self.load_current_state_tag("outcome_core_state_tag")?;
        let incoming_resume_token = self
            .codegen
            .function_cx
            .current_incoming_resume_token_ref
            .ok_or_else(|| {
                frontend_error("outcome core 缺少 incoming_resume_token_ref".to_string())
            })?;
        let payload = payload_param_index
            .map(|index| {
                self.function
                    .get_nth_param(index)
                    .ok_or_else(|| frontend_error("outcome core 缺少 payload 参数".to_string()))
            })
            .transpose()?;
        let ordinary_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_core_plain_dispatch");
        let composed_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_core_composed_dispatch");
        let incoming_is_null = self
            .codegen
            .builder
            .build_is_null(incoming_resume_token, "outcome_core_incoming_is_null")?;
        self.codegen.builder.build_conditional_branch(
            incoming_is_null,
            ordinary_resume_bb,
            composed_resume_bb,
        )?;

        self.codegen.builder.position_at_end(composed_resume_bb);
        if !self.dispatch_composed_call_boundary_resume(
            resume_state_tag,
            incoming_resume_token,
            resume_tuple_ty,
            payload,
        )? {
            self.codegen
                .builder
                .build_unconditional_branch(ordinary_resume_bb)?;
        }

        self.codegen.builder.position_at_end(ordinary_resume_bb);
        self.emit_resume_state_dispatch(resume_state_tag, resume_tuple_ty, payload)
    }

    pub(super) fn emit_generated_continuation_step(
        mut self,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.return_mode = CallableReturnMode::EffectOutcome;
        self.codegen.bind_explicit_effect_hidden_abi_slots(
            self.mir_fun.span,
            self.function,
            3,
            true,
        )?;
        let state_ref = self.function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "continuation step `{}` 缺少 state_ref 参数",
                self.function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let state_ref = self.codegen.cast_ptr(
            state_ref.into_pointer_value(),
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "cont_step_state_ref",
        )?;
        let resume_word = self
            .function
            .get_nth_param(1)
            .ok_or_else(|| frontend_error("continuation step 缺少 resume_word 参数".to_string()))?
            .into_int_value();
        let resume_gc_ref = self
            .function
            .get_nth_param(2)
            .ok_or_else(|| frontend_error("continuation step 缺少 resume_gc_ref 参数".to_string()))?
            .into_pointer_value();
        self.store_frame_root(state_ref)?;
        self.restore_frame_slots_to_locals()?;
        let current_effect_ctx =
            self.codegen
                .function_cx
                .current_effect_ctx_ref
                .ok_or_else(|| {
                    frontend_error("continuation step 缺少 current_effect_ctx_ref".to_string())
                })?;
        self.store_current_effect_ctx(current_effect_ctx, "cont_step_effect_ctx")?;
        let resume_state_tag = self.load_current_state_tag("cont_step_state_tag")?;
        let incoming_resume_token = self
            .codegen
            .function_cx
            .current_incoming_resume_token_ref
            .ok_or_else(|| {
                frontend_error("continuation step 缺少 incoming_resume_token_ref".to_string())
            })?;
        let payload = self.decode_effect_transport_parts(
            resume_tuple_ty,
            ValueTransportParts {
                word: resume_word,
                gc_ref: resume_gc_ref,
            },
            "cont_step_payload",
        )?;
        let ordinary_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "cont_step_plain_dispatch");
        let composed_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "cont_step_composed_dispatch");
        let incoming_is_null = self
            .codegen
            .builder
            .build_is_null(incoming_resume_token, "cont_step_incoming_is_null")?;
        self.codegen.builder.build_conditional_branch(
            incoming_is_null,
            ordinary_resume_bb,
            composed_resume_bb,
        )?;

        self.codegen.builder.position_at_end(composed_resume_bb);
        if !self.dispatch_composed_call_boundary_resume(
            resume_state_tag,
            incoming_resume_token,
            resume_tuple_ty,
            payload,
        )? {
            self.codegen
                .builder
                .build_unconditional_branch(ordinary_resume_bb)?;
        }

        self.codegen.builder.position_at_end(ordinary_resume_bb);
        self.emit_resume_state_dispatch(resume_state_tag, resume_tuple_ty, payload)
    }

    pub(super) fn write_generated_continuation_answer_slot(
        &mut self,
        surface: &ContinuationSurfaceResumeLayout<'ctx>,
        answer_slot: PointerValue<'ctx>,
        outcome_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(answer_cg) = self.codegen.cg_ty_of(surface.answer_ty()) else {
            return Err(frontend_error(format!(
                "continuation drive k{} answer t{} 缺少 codegen type",
                surface.continuation_schema().as_u32(),
                surface.answer_ty().as_u32()
            )));
        };
        if matches!(answer_cg, CgTy::Unit | CgTy::Never) {
            return Ok(());
        }
        let complete_transport = self.codegen.effect_outcome_complete_transport(
            self.mir_fun.span,
            outcome_ptr,
            "continuation_answer_transport",
        )?;
        let Some(answer) = self.decode_effect_transport_parts(
            surface.answer_ty(),
            complete_transport,
            "continuation_answer",
        )?
        else {
            return Ok(());
        };
        let slot_ptr = self.codegen.builder.build_pointer_cast(
            answer_slot,
            self.codegen.context.ptr_type(AddressSpace::default()),
            "continuation_answer_slot",
        )?;
        self.codegen.builder.build_store(slot_ptr, answer)?;
        Ok(())
    }

    pub(super) fn emit_generated_continuation_resume_driver(
        mut self,
        surface: &ContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let cont = self.function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "continuation drive `{}` 缺少 continuation 参数",
                self.function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let cont_root_slot = self
            .codegen
            .create_gc_root_slot(self.mir_fun.span, "continuation_drive_root")?;
        let cont_ptr = self.root_gc_pointer_in_slot(
            cont_root_slot,
            cont.into_pointer_value(),
            "continuation_drive_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let state_ref = self.load_frame_from_continuation(cont_ptr)?;
        self.store_frame_root(state_ref)?;
        let resume_word = self
            .function
            .get_nth_param(1)
            .ok_or_else(|| frontend_error("continuation drive 缺少 resume_word 参数".to_string()))?
            .into_int_value();
        let resume_gc_ref = self
            .function
            .get_nth_param(2)
            .ok_or_else(|| {
                frontend_error("continuation drive 缺少 resume_gc_ref 参数".to_string())
            })?
            .into_pointer_value();
        let answer_slot = self
            .function
            .get_nth_param(3)
            .ok_or_else(|| frontend_error("continuation drive 缺少 answer_slot 参数".to_string()))?
            .into_pointer_value();
        let outcome_ptr = self
            .function
            .get_nth_param(4)
            .ok_or_else(|| frontend_error("continuation drive 缺少 outcome 参数".to_string()))?
            .into_pointer_value();
        let resume_state_tag = self.load_continuation_resume_state(cont_ptr)?;
        let first_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_first");
        let double_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_double");
        let finalize_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_finalize");
        let return_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_return");
        let first_resume = self.try_mark_continuation_resumed(cont_ptr, "continuation_drive")?;
        self.codegen.builder.build_conditional_branch(
            first_resume,
            first_resume_bb,
            double_resume_bb,
        )?;

        self.codegen.builder.position_at_end(double_resume_bb);
        self.emit_double_resume_runtime_error_to_ptr(outcome_ptr, resume_state_tag)?;

        self.codegen.builder.position_at_end(first_resume_bb);
        self.store_continuation_resume_payload(
            cont_ptr,
            ValueTransportParts {
                word: resume_word,
                gc_ref: resume_gc_ref,
            },
            "continuation_drive",
        )?;
        self.store_current_state_tag(resume_state_tag, "continuation_drive_state")?;
        let cont_ptr = self.codegen.load_gc_root_slot(
            self.mir_fun.span,
            cont_root_slot,
            "continuation_drive_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let current_effect_ctx = self.load_captured_effect_ctx_from_continuation(cont_ptr)?;
        let incoming_resume_token = self.load_captured_callee_suspend_state_ref(cont_ptr)?;
        let step_fn = self.load_continuation_step_fn(cont_ptr)?;
        let state_ref = self.current_frame_gc_ref("continuation_drive_state_reload")?;
        let step_args = [
            state_ref.into(),
            resume_word.into(),
            resume_gc_ref.into(),
            current_effect_ctx.into(),
            incoming_resume_token.into(),
            outcome_ptr.into(),
        ];
        self.codegen
            .with_conservative_gc_local_root_spills(self.mir_fun.span, |codegen| {
                let typed_step = codegen.builder.build_pointer_cast(
                    step_fn,
                    codegen.context.ptr_type(AddressSpace::default()),
                    "continuation_step_fn_typed",
                )?;
                codegen.builder.build_indirect_call(
                    codegen.continuation_step_llvm_ty(),
                    typed_step,
                    &step_args,
                    "continuation_step_call",
                )?;
                Ok(())
            })?;
        self.codegen
            .builder
            .build_unconditional_branch(finalize_bb)?;

        self.codegen.builder.position_at_end(finalize_bb);
        let answer_has_runtime_value = self
            .codegen
            .cg_ty_of(surface.answer_ty())
            .is_some_and(|cg| !matches!(cg, CgTy::Unit | CgTy::Never));
        if !answer_has_runtime_value {
            self.codegen.builder.build_unconditional_branch(return_bb)?;
        } else {
            let write_answer_bb = self
                .codegen
                .context
                .append_basic_block(self.function, "resume_write_answer");
            let answer_slot_is_null = self
                .codegen
                .builder
                .build_is_null(answer_slot, "continuation_answer_slot_is_null")?;
            let is_propagating = self.codegen.effect_outcome_is_propagating(
                self.mir_fun.span,
                outcome_ptr,
                "continuation_drive_outcome",
            )?;
            let should_skip = self.codegen.builder.build_or(
                answer_slot_is_null,
                is_propagating,
                "continuation_skip_answer",
            )?;
            self.codegen.builder.build_conditional_branch(
                should_skip,
                return_bb,
                write_answer_bb,
            )?;
            self.codegen.builder.position_at_end(write_answer_bb);
            self.write_generated_continuation_answer_slot(surface, answer_slot, outcome_ptr)?;
            self.codegen.builder.build_unconditional_branch(return_bb)?;
        }

        self.codegen.builder.position_at_end(return_bb);
        self.codegen.builder.build_return(None)?;
        self.seal_unterminated_state_blocks_as_unreachable()?;
        Ok(())
    }

    pub(super) fn emit_double_resume_runtime_error(
        &mut self,
        resume_state_tag: IntValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let (case_tag, payload_tuple_ty) = self.double_resume_runtime_error_case()?;
        let payload = self.lower_runtime_error_boundary_payload(payload_tuple_ty)?;
        let continuation = self.create_continuation_object_with_state_tag(
            None,
            resume_state_tag,
            case_tag,
            None,
            None,
        )?;
        match self.return_mode {
            CallableReturnMode::EffectOutcome => {
                let outcome = self.build_propagating_effect_outcome_for_case(
                    case_tag,
                    payload,
                    payload_tuple_ty,
                    continuation,
                )?;
                self.emit_effect_outcome_return(outcome)
            }
            _ => {
                let out_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
                    frontend_error(format!(
                        "callable `{}` step schema s{} 缺少 double resume runtime error case c{}",
                        self.callable.root_fqn(),
                        self.abi_step_schema.as_u32(),
                        case_tag.as_u32(),
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
}
