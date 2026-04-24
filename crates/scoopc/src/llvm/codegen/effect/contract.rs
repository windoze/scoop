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

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn register_effect_contract_types(&self) {
        let _ = self.llvm_effect_ctx_struct_type();
        let _ = self.llvm_value_transport_struct_type();
        let _ = self.llvm_effect_signal_struct_type();
        let _ = self.llvm_effect_outcome_struct_type();
    }

    pub(super) fn null_effect_resume_token(&self) -> PointerValue<'ctx> {
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
