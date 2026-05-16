//! Continuation-object operations: GC allocation of continuation objects, frame loading from a continuation, resume state loading, captured effect-context retrieval, and the resume-payload mark/store path.

use super::*;

impl<'cg, 'a, 'ctx> RefactorCallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn store_no_outward_call_complete(
        &mut self,
        span: crate::span::Span,
        site_id: SiteId,
        step_schema: StepSchemaId,
        step: BasicValueEnum<'ctx>,
        target: LocalId,
    ) -> Result<(), LlvmEmitError> {
        let step_layout = self.abi.step_layout(step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic call site {} 缺少 return step schema s{} layout",
                site_id.as_u32(),
                step_schema.as_u32()
            ))
        })?;
        if !step_layout.cases().is_empty() {
            return Err(frontend_error(format!(
                "refactor source-slice dynamic call site {} return step schema s{} 含 outward case，必须走 boundary lowering",
                site_id.as_u32(),
                step_schema.as_u32()
            )));
        }
        let payload = self.codegen.refactor_extract_step_payload(
            step_layout,
            step,
            step_layout.complete_variant(),
            "refactor_dynamic_complete_payload",
        )?;
        match payload {
            Some(raw) => {
                let _ = self.store_loaded_raw_local(span, target, raw)?;
            }
            None => {
                let slot = self.codegen.mir_local_slot(span, &self.slots, target)?;
                if slot.cg_ty != CgTy::Unit && slot.cg_ty != CgTy::Never {
                    return Err(frontend_error(format!(
                        "refactor dynamic call site {} non-Unit target 缺少 Complete payload",
                        site_id.as_u32()
                    )));
                }
                let _ = self.store_local_value(span, target, CgValue::unit())?;
            }
        }
        Ok(())
    }

    pub(super) fn create_continuation_object(
        &mut self,
        resume_state: StateId,
        case_tag: CaseTag,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let resume_state_tag = self
            .codegen
            .context
            .i32_type()
            .const_int(resume_state.as_u32() as u64, false);
        self.create_continuation_object_with_state_tag(
            Some(resume_state),
            resume_state_tag,
            case_tag,
            callee_continuation,
            composition,
        )
    }

    pub(super) fn create_continuation_object_with_state_tag(
        &mut self,
        resume_state: Option<StateId>,
        resume_state_tag: IntValue<'ctx>,
        case_tag: CaseTag,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if let Some(composition) = composition
            && Some(composition.caller_resume_state()) != resume_state
        {
            return Err(frontend_error(format!(
                "refactor continuation composition resume_state 漂移：object={:?} contract=st{}",
                resume_state,
                composition.caller_resume_state().as_u32(),
            )));
        }
        // A continuation extracted from a Step case may only exist as an SSA value here.
        // Root it in an explicit-frame slot before allocating the wrapper continuation, then
        // reload from that slot after the allocation safepoint before writing the composition
        // edge. Otherwise moving GC can relocate the callee continuation while the stale SSA
        // value still gets written into the wrapper.
        let callee_continuation_root = match callee_continuation {
            Some(callee_continuation) => {
                let slot = self.codegen.create_refactor_gc_root_slot(
                    self.mir_fun.span,
                    "refactor_composed_callee_root",
                )?;
                self.codegen.store_refactor_gc_root_slot(
                    self.mir_fun.span,
                    slot,
                    callee_continuation,
                    "refactor_composed_callee_root",
                )?;
                Some(slot)
            }
            None => None,
        };
        let owner_step = self
            .program
            .step_type(self.abi_step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor callable `{}` 缺少 owner step schema s{}",
                    self.callable.root_fqn(),
                    self.abi_step_schema.as_u32()
                ))
            })?;
        let continuation_case = owner_step
            .cases()
            .iter()
            .find(|case| case.case_tag() == case_tag)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor callable `{}` step schema s{} 缺少 continuation case c{}",
                    self.callable.root_fqn(),
                    self.abi_step_schema.as_u32(),
                    case_tag.as_u32()
                ))
            })?;
        let continuation_schema = continuation_case
            .continuation_contract()
            .continuation_schema();
        let _surface = self.abi.surface_resume_layout(continuation_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor callable `{}` case c{} 缺少 continuation schema k{} 的 surface resume ABI",
                self.callable.root_fqn(),
                case_tag.as_u32(),
                continuation_schema.as_u32(),
            ))
        })?;
        let dispatch = self
            .abi
            .surface_resume_dispatch_layout(continuation_schema)?;
        let target = match dispatch
            .target()
            .owner_trampolines()
            .iter()
            .find(|candidate| {
                candidate.owner_continuation_object() == self.callable.continuation_object()
                    || candidate.owner_version_key() == self.callable.body_version_key()
            }) {
            Some(target) => target,
            None if matches!(
                dispatch.target(),
                RefactorContinuationSurfaceResumeDispatchTarget::Unreachable
            ) =>
            {
                // Non-resuming exits like local `Raise<RuntimeError>` may still travel through
                // Step/EffectOutcome payloads, but they intentionally publish no resume target.
                return Ok(self.codegen.llvm_gc_i8_ptr_type().const_null());
            }
            None => {
                return Err(frontend_error(format!(
                    "refactor callable `{}` case c{} continuation schema k{} 缺少 owner continuation drive target",
                    self.callable.root_fqn(),
                    case_tag.as_u32(),
                    continuation_schema.as_u32(),
                )));
            }
        };
        let step_fun = self.codegen.refactor_continuation_step_function(target);
        let cont_layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor callable `{}` 缺少 continuation object ko{} layout",
                    self.callable.root_fqn(),
                    self.callable.continuation_object().as_u32()
                ))
            })?;
        let cont_ptr = self.codegen.refactor_alloc_gc_struct(
            self.mir_fun.span,
            cont_layout.llvm_ty(),
            cont_layout.layout_anchor_name(),
            "refactor_cont",
        )?;
        let cont_root_slot = self
            .codegen
            .create_refactor_gc_root_slot(self.mir_fun.span, "refactor_cont_root")?;
        let cont_ptr =
            self.root_gc_pointer_in_slot(cont_root_slot, cont_ptr, "refactor_cont_root")?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let current_effect_ctx = self.load_current_effect_ctx("refactor_cont_effect_ctx")?;
        let resumed_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUMED,
            "refactor_cont_resumed_gep",
        )?;
        self.codegen
            .builder
            .build_store(resumed_gep, self.codegen.context.i32_type().const_zero())?;
        let state_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_STATE,
            "refactor_cont_state_gep",
        )?;
        self.codegen
            .builder
            .build_store(state_gep, resume_state_tag)?;
        let effect_ctx_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_EFFECT_CTX,
            "refactor_cont_effect_ctx_gep",
        )?;
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            effect_ctx_gep,
            current_effect_ctx,
        )?;
        let cont_ptr = self.codegen.load_refactor_gc_root_slot(
            self.mir_fun.span,
            cont_root_slot,
            "refactor_cont_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let current_frame = self.current_frame_gc_ref("refactor_cont_state_ref_reload")?;
        let state_ref_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_STATE_REF,
            "refactor_cont_state_ref_gep",
        )?;
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            state_ref_gep,
            current_frame,
        )?;
        let cont_ptr = self.codegen.load_refactor_gc_root_slot(
            self.mir_fun.span,
            cont_root_slot,
            "refactor_cont_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let step_fn_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_STEP_FN,
            "refactor_cont_step_fn_gep",
        )?;
        let step_fn_ptr = self.codegen.builder.build_pointer_cast(
            step_fun.as_global_value().as_pointer_value(),
            self.codegen.llvm_i8_ptr_type(),
            "refactor_cont_step_fn",
        )?;
        self.codegen.builder.build_store(step_fn_gep, step_fn_ptr)?;
        let resume_word_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_WORD,
            "refactor_cont_resume_word_gep",
        )?;
        self.codegen.builder.build_store(
            resume_word_gep,
            self.codegen.context.i64_type().const_zero(),
        )?;
        let resume_gc_ref_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_GC_REF,
            "refactor_cont_resume_gc_ref_gep",
        )?;
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            resume_gc_ref_gep,
            self.codegen.llvm_gc_i8_ptr_type().const_null(),
        )?;
        let cont_ptr = self.codegen.load_refactor_gc_root_slot(
            self.mir_fun.span,
            cont_root_slot,
            "refactor_cont_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        let captured_token_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_CALLEE_SUSPEND_STATE,
            "refactor_cont_captured_token_gep",
        )?;
        let captured_token = match callee_continuation_root {
            Some(slot) => self.codegen.load_refactor_gc_root_slot(
                self.mir_fun.span,
                slot,
                "refactor_composed_callee_root",
            )?,
            None => self.codegen.llvm_gc_i8_ptr_type().const_null(),
        };
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            captured_token_gep,
            captured_token,
        )?;
        let cont_ptr = self.codegen.load_refactor_gc_root_slot(
            self.mir_fun.span,
            cont_root_slot,
            "refactor_cont_root",
        )?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont_ptr)?;
        if let Some(slot) = callee_continuation_root {
            self.clear_root_gc_slot(slot, "refactor_composed_callee_root_clear")?;
        }
        self.clear_root_gc_slot(cont_root_slot, "refactor_cont_root_clear")?;
        self.codegen.refactor_cast_ptr(
            cont_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            "refactor_cont_gc",
        )
    }

    pub(super) fn cast_gc_ref_to_continuation(
        &mut self,
        ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let target_ty = self.codegen.llvm_ptr_type(self.codegen.gc_address_space());
        self.codegen
            .refactor_cast_ptr(ptr, target_ty, "refactor_cont_typed")
    }

    pub(super) fn load_frame_from_continuation(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .ok_or_else(|| {
                frontend_error("refactor resume 缺少 continuation layout".to_string())
            })?;
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_STATE_REF,
            "refactor_load_frame_gep",
        )?;
        let raw = self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_gc_i8_ptr_type(),
                gep,
                "refactor_load_frame_gc",
            )?
            .into_pointer_value();
        self.codegen.refactor_cast_ptr(
            raw,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "refactor_frame_typed",
        )
    }

    pub(super) fn load_continuation_resume_state(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_STATE,
            "refactor_resume_state_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(
                self.codegen.context.i32_type(),
                gep,
                "refactor_resume_state",
            )?
            .into_int_value())
    }

    pub(super) fn load_captured_effect_ctx_from_continuation(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_EFFECT_CTX,
            "refactor_load_captured_effect_ctx_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_gc_i8_ptr_type(),
                gep,
                "refactor_captured_effect_ctx",
            )?
            .into_pointer_value())
    }

    pub(super) fn load_captured_callee_suspend_state_ref(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_CALLEE_SUSPEND_STATE,
            "refactor_captured_callee_suspend_state_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_gc_i8_ptr_type(),
                gep,
                "refactor_captured_callee_suspend_state",
            )?
            .into_pointer_value())
    }

    pub(super) fn load_continuation_step_fn(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_STEP_FN,
            "refactor_cont_step_fn_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_i8_ptr_type(),
                gep,
                "refactor_cont_step_fn",
            )?
            .into_pointer_value())
    }

    pub(super) fn try_mark_continuation_resumed(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUMED,
            &format!("{name}_resumed_gep"),
        )?;
        let cx = self.codegen.builder.build_cmpxchg(
            gep,
            self.codegen.context.i32_type().const_zero(),
            self.codegen.context.i32_type().const_int(1, false),
            AtomicOrdering::SequentiallyConsistent,
            AtomicOrdering::SequentiallyConsistent,
        )?;
        Ok(self
            .codegen
            .builder
            .build_extract_value(cx, 1, &format!("{name}_resumed_ok"))?
            .into_int_value())
    }

    pub(super) fn store_continuation_resume_payload(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
        transport: ValueTransportParts<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let word_gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_WORD,
            &format!("{name}_resume_word_gep"),
        )?;
        self.codegen.builder.build_store(word_gep, transport.word)?;
        let gc_ref_gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_GC_REF,
            &format!("{name}_resume_gc_ref_gep"),
        )?;
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            gc_ref_gep,
            transport.gc_ref,
        )
    }
}
