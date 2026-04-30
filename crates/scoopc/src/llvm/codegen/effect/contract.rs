use super::*;

#[derive(Clone, Copy)]
pub(super) struct ValueTransport<'ctx> {
    pub(super) word: IntValue<'ctx>,
    pub(super) gc_ref: PointerValue<'ctx>,
}

#[derive(Clone, Copy)]
pub(super) struct EffectSignal<'ctx> {
    pub(super) op_tag: IntValue<'ctx>,
    pub(super) effect_instance_key: IntValue<'ctx>,
    pub(super) payload: ValueTransport<'ctx>,
    pub(super) resume_token: PointerValue<'ctx>,
}

#[derive(Clone, Copy)]
pub(super) struct EffectOutcome<'ctx> {
    pub(super) is_propagating: IntValue<'ctx>,
    #[allow(dead_code)]
    pub(super) signal: EffectSignal<'ctx>,
}

#[derive(Clone, Copy)]
pub(in crate::llvm::codegen) struct LegacyEffectBoundary<'ctx> {
    outcome_slot: PointerValue<'ctx>,
    saved_handler_top: PointerValue<'ctx>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn register_effect_contract_types(&self) {
        let _ = self.llvm_effect_ctx_struct_type();
        let _ = self.llvm_value_transport_struct_type();
        let _ = self.llvm_effect_signal_struct_type();
        let _ = self.llvm_effect_outcome_struct_type();
    }

    pub(in crate::llvm::codegen) fn alloc_effect_ctx_slot(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.register_effect_contract_types();
        self.create_entry_alloca_raw(
            at,
            &format!("{label}_effect_ctx"),
            self.llvm_effect_ctx_struct_type().into(),
        )
    }

    pub(in crate::llvm::codegen) fn alloc_effect_outcome_slot(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.register_effect_contract_types();
        self.create_entry_alloca_raw(
            at,
            &format!("{label}_effect_outcome"),
            self.llvm_effect_outcome_struct_type().into(),
        )
    }

    pub(in crate::llvm::codegen) fn prepare_current_effect_call_contract(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<(PointerValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let ctx_slot = self.alloc_effect_ctx_slot(at, label)?;
        self.capture_current_effect_ctx_into_slot(at, ctx_slot, label)?;
        let outcome_slot = self.alloc_effect_outcome_slot(at, label)?;
        Ok((ctx_slot, outcome_slot))
    }

    pub(in crate::llvm::codegen) fn begin_legacy_effect_boundary(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<LegacyEffectBoundary<'ctx>, LlvmEmitError> {
        let (ctx_slot, outcome_slot) = self.prepare_current_effect_call_contract(at, label)?;
        let installed_top = self.load_effect_ctx_handler_top_from_slot(at, ctx_slot, label)?;
        let saved_handler_top =
            self.swap_effect_handler_stack_top(at, installed_top, &format!("{label}_install"))?;
        Ok(LegacyEffectBoundary {
            outcome_slot,
            saved_handler_top,
        })
    }

    pub(in crate::llvm::codegen) fn finish_legacy_effect_boundary(
        &mut self,
        at: crate::span::Span,
        boundary: LegacyEffectBoundary<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.consume_current_effect_outcome_into(at, boundary.outcome_slot, label)?;
        let _ = self.swap_effect_handler_stack_top(
            at,
            boundary.saved_handler_top,
            &format!("{label}_restore"),
        )?;
        Ok(boundary.outcome_slot)
    }

    pub(in crate::llvm::codegen) fn capture_current_effect_ctx_into_slot(
        &mut self,
        at: crate::span::Span,
        ctx_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let get_top = self.declare_runtime_effect_handler_stack_top();
        let top = self
            .build_call_preserving_gc_local_roots(at, get_top, &[], &format!("{label}_ctx_top"))?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect_handler_stack_top return",
                at: at.into(),
            })?
            .into_pointer_value();
        let field_ptr = self.builder.build_struct_gep(
            self.llvm_effect_ctx_struct_type(),
            ctx_slot,
            0,
            &format!("{label}_ctx_handler_top_ptr"),
        )?;
        self.builder.build_store(field_ptr, top)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn load_effect_ctx_handler_top_from_slot(
        &mut self,
        at: crate::span::Span,
        ctx_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let field_ptr = self.builder.build_struct_gep(
            self.llvm_effect_ctx_struct_type(),
            ctx_slot,
            0,
            &format!("{label}_ctx_handler_top_ptr"),
        )?;
        let loaded = self.builder.build_load(
            self.llvm_i8_ptr_type(),
            field_ptr,
            &format!("{label}_ctx_handler_top"),
        )?;
        let BasicValueEnum::PointerValue(ptr) = loaded else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect ctx handler_top value",
                at: at.into(),
            });
        };
        Ok(ptr)
    }

    pub(in crate::llvm::codegen) fn swap_effect_handler_stack_top(
        &mut self,
        at: crate::span::Span,
        new_top: PointerValue<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let swap = self.declare_runtime_effect_handler_stack_swap_top();
        let saved_top = self
            .build_call_preserving_gc_local_roots(
                at,
                swap,
                &[new_top.into()],
                &format!("{label}_swap_handler_top"),
            )?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect_handler_stack_swap_top return",
                at: at.into(),
            })?
            .into_pointer_value();
        Ok(saved_top)
    }

    pub(in crate::llvm::codegen) fn consume_current_effect_outcome_into(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let consume = self.declare_runtime_effect_outcome_consume_current();
        let _ = self.build_call_preserving_gc_local_roots(
            at,
            consume,
            &[outcome_slot.into()],
            &format!("{label}_consume_outcome"),
        )?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn publish_effect_outcome_from_slot(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let publish = self.declare_runtime_effect_outcome_publish();
        let _ = self.build_call_preserving_gc_local_roots(
            at,
            publish,
            &[outcome_slot.into()],
            &format!("{label}_publish_outcome"),
        )?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn effect_outcome_is_propagating(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let tag_ptr = self.builder.build_struct_gep(
            self.llvm_effect_outcome_struct_type(),
            outcome_slot,
            0,
            &format!("{label}_effect_outcome_tag_ptr"),
        )?;
        let tag = self
            .builder
            .build_load(
                self.context.i32_type(),
                tag_ptr,
                &format!("{label}_effect_outcome_tag"),
            )?
            .into_int_value();
        let is_propagating = self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            tag,
            self.context.i32_type().const_zero(),
            &format!("{label}_effect_outcome_is_propagating"),
        )?;
        if tag.get_type() != self.context.i32_type() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect outcome tag type",
                at: at.into(),
            });
        }
        Ok(is_propagating)
    }

    pub(in crate::llvm::codegen) fn effect_outcome_resume_token(
        &mut self,
        at: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let signal_ptr = self.builder.build_struct_gep(
            self.llvm_effect_outcome_struct_type(),
            outcome_slot,
            3,
            &format!("{label}_effect_outcome_signal_ptr"),
        )?;
        let resume_token_ptr = self.builder.build_struct_gep(
            self.llvm_effect_signal_struct_type(),
            signal_ptr,
            3,
            &format!("{label}_effect_signal_resume_token_ptr"),
        )?;
        let resume_token = self
            .builder
            .build_load(
                self.llvm_gc_i8_ptr_type(),
                resume_token_ptr,
                &format!("{label}_effect_signal_resume_token"),
            )?
            .into_pointer_value();
        let _ = at;
        Ok(resume_token)
    }

    pub(in crate::llvm::codegen) fn null_effect_resume_token(&self) -> PointerValue<'ctx> {
        self.llvm_gc_i8_ptr_type().const_null()
    }

    pub(super) fn build_value_transport(
        &self,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
    ) -> ValueTransport<'ctx> {
        self.register_effect_contract_types();
        ValueTransport { word, gc_ref }
    }

    pub(super) fn zero_value_transport(&self) -> ValueTransport<'ctx> {
        self.build_value_transport(
            self.context.i64_type().const_zero(),
            self.llvm_gc_i8_ptr_type().const_null(),
        )
    }

    pub(super) fn build_effect_signal(
        &self,
        op_tag: IntValue<'ctx>,
        effect_instance_key: IntValue<'ctx>,
        payload: ValueTransport<'ctx>,
        resume_token: PointerValue<'ctx>,
    ) -> EffectSignal<'ctx> {
        self.register_effect_contract_types();
        EffectSignal {
            op_tag,
            effect_instance_key,
            payload,
            resume_token,
        }
    }

    pub(super) fn zero_effect_signal(&self) -> EffectSignal<'ctx> {
        let i32_zero = self.context.i32_type().const_zero();
        self.build_effect_signal(
            i32_zero,
            i32_zero,
            self.zero_value_transport(),
            self.null_effect_resume_token(),
        )
    }

    pub(super) fn build_effect_outcome(
        &self,
        is_propagating: IntValue<'ctx>,
        signal: EffectSignal<'ctx>,
    ) -> EffectOutcome<'ctx> {
        self.register_effect_contract_types();
        EffectOutcome {
            is_propagating,
            signal,
        }
    }

    pub(super) fn read_current_effect_payload_transport(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<ValueTransport<'ctx>, LlvmEmitError> {
        let read_word_fn = self.declare_runtime_effect_perform_slot_read_u64();
        let word = self
            .builder
            .build_call(read_word_fn, &[], &format!("{label}_word"))?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform_slot_read_u64 return",
                at: at.into(),
            })?
            .into_int_value();
        let read_gc_ref_fn = self.declare_runtime_effect_perform_slot_read_gc_ref();
        let gc_ref = self
            .builder
            .build_call(read_gc_ref_fn, &[], &format!("{label}_gc_ref"))?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform_slot_read_gc_ref return",
                at: at.into(),
            })?
            .into_pointer_value();
        Ok(self.build_value_transport(word, gc_ref))
    }

    pub(super) fn read_current_effect_signal(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<EffectSignal<'ctx>, LlvmEmitError> {
        let read_op_tag_fn = self.declare_runtime_effect_perform_slot_read_op_tag();
        let op_tag = self
            .builder
            .build_call(read_op_tag_fn, &[], &format!("{label}_op_tag"))?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform_slot_read_op_tag return",
                at: at.into(),
            })?
            .into_int_value();
        let read_effect_instance_key_fn =
            self.declare_runtime_effect_perform_slot_read_effect_instance_key();
        let effect_instance_key = self
            .builder
            .build_call(
                read_effect_instance_key_fn,
                &[],
                &format!("{label}_effect_instance_key"),
            )?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "perform_slot_read_effect_instance_key return",
                at: at.into(),
            })?
            .into_int_value();
        let payload =
            self.read_current_effect_payload_transport(at, &format!("{label}_payload"))?;
        Ok(self.build_effect_signal(
            op_tag,
            effect_instance_key,
            payload,
            self.null_effect_resume_token(),
        ))
    }

    pub(super) fn read_current_effect_outcome_status(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<EffectOutcome<'ctx>, LlvmEmitError> {
        let is_active_fn = self.declare_runtime_effect_is_active();
        let active_raw = self
            .build_call_preserving_gc_local_roots(
                at,
                is_active_fn,
                &[],
                &format!("{label}_is_active"),
            )?
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return",
                at: at.into(),
            })?
            .into_int_value();
        let is_propagating = self.builder.build_int_compare(
            inkwell::IntPredicate::NE,
            active_raw,
            self.context.i32_type().const_zero(),
            &format!("{label}_propagates"),
        )?;
        Ok(self.build_effect_outcome(is_propagating, self.zero_effect_signal()))
    }

    pub(super) fn emit_current_effect_propagation_with_trace(
        &mut self,
        span: crate::span::Span,
        signal: EffectSignal<'ctx>,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        let _ = signal.resume_token;
        let write_fn = self.declare_runtime_effect_perform_slot_write_u64_with_gc_ref();
        self.builder.build_call(
            write_fn,
            &[
                signal.op_tag.into(),
                signal.effect_instance_key.into(),
                signal.payload.word.into(),
                signal.payload.gc_ref.into(),
            ],
            &format!("{label}_write_signal"),
        )?;
        self.emit_effect_set_active_with_trace(span, label)
    }
}
