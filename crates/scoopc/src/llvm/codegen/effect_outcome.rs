//! Backend-owned explicit `EffectOutcome` / transport primitives.
//!
//! 这个模块只承载 target-shape 下仍然有效的共享 contract：
//! - `ScoopValueTransport`
//! - `ScoopEffectSignal`
//! - `ScoopEffectOutcome`
//!   的 builder/query helper；
//! - 标量 `<-> u64 word` transport；
//! - task transport tuple 的最小识别与拆分。

use inkwell::IntPredicate;
use inkwell::values::{BasicValueEnum, IntValue, PointerValue, StructValue};

use super::super::LlvmEmitError;
use super::{CgTy, CgValue, IntTy, MainCodegen};
use crate::ty::{TypeId, TypeKind, ValueTypeKind};

const EFFECT_OUTCOME_TAG_COMPLETE: u32 = 0;
const EFFECT_OUTCOME_TAG_PROPAGATE: u32 = 1;

#[derive(Clone, Copy)]
pub(in crate::llvm::codegen) struct ValueTransportParts<'ctx> {
    pub(in crate::llvm::codegen) word: IntValue<'ctx>,
    pub(in crate::llvm::codegen) gc_ref: PointerValue<'ctx>,
}

#[derive(Clone, Copy)]
pub(in crate::llvm::codegen) enum EffectOutcomeTag {
    Complete,
    Propagate,
}

impl EffectOutcomeTag {
    fn as_u32(self) -> u32 {
        match self {
            Self::Complete => EFFECT_OUTCOME_TAG_COMPLETE,
            Self::Propagate => EFFECT_OUTCOME_TAG_PROPAGATE,
        }
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn zero_value_transport_parts(&self) -> ValueTransportParts<'ctx> {
        ValueTransportParts {
            word: self.context.i64_type().const_zero(),
            gc_ref: self.llvm_gc_i8_ptr_type().const_null(),
        }
    }

    fn zero_effect_signal(&mut self) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let zero_transport = self.zero_value_transport_parts();
        self.build_effect_signal(
            self.context.i32_type().const_zero(),
            self.context.i32_type().const_zero(),
            zero_transport,
            self.llvm_gc_i8_ptr_type().const_null(),
        )
    }

    /// Keep the default complete outcome construction centralized in the backend-owned
    /// primitive instead of hand-rolling the aggregate at individual bridge sites.
    pub(in crate::llvm::codegen) fn build_zero_complete_effect_outcome(
        &mut self,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let zero_transport = self.zero_value_transport_parts();
        let zero_signal = self.zero_effect_signal()?;
        self.build_effect_outcome(EffectOutcomeTag::Complete, zero_transport, zero_signal)
    }

    pub(in crate::llvm::codegen) fn build_value_transport(
        &mut self,
        transport: ValueTransportParts<'ctx>,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let mut aggregate = self.llvm_value_transport_struct_type().get_undef();
        aggregate = self
            .builder
            .build_insert_value(aggregate, transport.word, 0, "effect_transport_word")?
            .into_struct_value();
        aggregate = self
            .builder
            .build_insert_value(aggregate, transport.gc_ref, 1, "effect_transport_gc_ref")?
            .into_struct_value();
        Ok(aggregate)
    }

    pub(in crate::llvm::codegen) fn build_effect_signal(
        &mut self,
        op_tag: IntValue<'ctx>,
        effect_instance_key: IntValue<'ctx>,
        payload: ValueTransportParts<'ctx>,
        resume_token: PointerValue<'ctx>,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let payload = self.build_value_transport(payload)?;
        let mut signal = self.llvm_effect_signal_struct_type().get_undef();
        signal = self
            .builder
            .build_insert_value(signal, op_tag, 0, "effect_signal_op_tag")?
            .into_struct_value();
        signal = self
            .builder
            .build_insert_value(signal, effect_instance_key, 1, "effect_signal_instance_key")?
            .into_struct_value();
        signal = self
            .builder
            .build_insert_value(signal, payload, 2, "effect_signal_payload")?
            .into_struct_value();
        signal = self
            .builder
            .build_insert_value(signal, resume_token, 3, "effect_signal_resume_token")?
            .into_struct_value();
        Ok(signal)
    }

    pub(in crate::llvm::codegen) fn build_effect_outcome(
        &mut self,
        tag: EffectOutcomeTag,
        complete: ValueTransportParts<'ctx>,
        signal: StructValue<'ctx>,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let tag_value = self
            .context
            .i32_type()
            .const_int(u64::from(tag.as_u32()), false);
        let complete = self.build_value_transport(complete)?;
        let mut outcome = self.llvm_effect_outcome_struct_type().get_undef();
        outcome = self
            .builder
            .build_insert_value(outcome, tag_value, 0, "effect_outcome_tag")?
            .into_struct_value();
        outcome = self
            .builder
            .build_insert_value(
                outcome,
                self.context.i32_type().const_zero(),
                1,
                "effect_outcome_reserved",
            )?
            .into_struct_value();
        outcome = self
            .builder
            .build_insert_value(outcome, complete, 2, "effect_outcome_complete")?
            .into_struct_value();
        outcome = self
            .builder
            .build_insert_value(outcome, signal, 3, "effect_outcome_signal")?
            .into_struct_value();
        Ok(outcome)
    }

    pub(in crate::llvm::codegen) fn alloc_effect_outcome_slot(
        &mut self,
        at: crate::span::Span,
        label: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self.create_entry_alloca_raw(
            at,
            &format!("{label}_outcome"),
            self.llvm_effect_outcome_struct_type().into(),
        )?;
        let zero_outcome = self.build_zero_complete_effect_outcome()?;
        self.builder.build_store(slot, zero_outcome)?;
        Ok(slot)
    }

    fn load_effect_outcome_struct(
        &mut self,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        Ok(self
            .builder
            .build_load(
                self.llvm_effect_outcome_struct_type(),
                outcome_ptr,
                &format!("{name}_load"),
            )?
            .into_struct_value())
    }

    fn effect_outcome_signal_struct(
        &mut self,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<StructValue<'ctx>, LlvmEmitError> {
        let outcome = self.load_effect_outcome_struct(outcome_ptr, name)?;
        Ok(self
            .builder
            .build_extract_value(outcome, 3, &format!("{name}_signal"))?
            .into_struct_value())
    }

    fn extract_value_transport_parts(
        &mut self,
        transport: StructValue<'ctx>,
        name: &str,
    ) -> Result<ValueTransportParts<'ctx>, LlvmEmitError> {
        let word = self
            .builder
            .build_extract_value(transport, 0, &format!("{name}_word"))?
            .into_int_value();
        let gc_ref = self
            .builder
            .build_extract_value(transport, 1, &format!("{name}_gc_ref"))?
            .into_pointer_value();
        Ok(ValueTransportParts { word, gc_ref })
    }

    pub(in crate::llvm::codegen) fn effect_outcome_is_propagating(
        &mut self,
        _at: crate::span::Span,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let outcome = self.load_effect_outcome_struct(outcome_ptr, name)?;
        let tag = self
            .builder
            .build_extract_value(outcome, 0, &format!("{name}_tag"))?
            .into_int_value();
        self.builder
            .build_int_compare(
                IntPredicate::EQ,
                tag,
                self.context
                    .i32_type()
                    .const_int(u64::from(EFFECT_OUTCOME_TAG_PROPAGATE), false),
                &format!("{name}_is_propagating"),
            )
            .map_err(Into::into)
    }

    pub(in crate::llvm::codegen) fn effect_outcome_payload_transport(
        &mut self,
        _at: crate::span::Span,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<ValueTransportParts<'ctx>, LlvmEmitError> {
        let signal = self.effect_outcome_signal_struct(outcome_ptr, name)?;
        let payload = self
            .builder
            .build_extract_value(signal, 2, &format!("{name}_payload"))?
            .into_struct_value();
        self.extract_value_transport_parts(payload, &format!("{name}_payload"))
    }

    pub(in crate::llvm::codegen) fn effect_outcome_complete_transport(
        &mut self,
        _at: crate::span::Span,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<ValueTransportParts<'ctx>, LlvmEmitError> {
        let outcome = self.load_effect_outcome_struct(outcome_ptr, name)?;
        let complete = self
            .builder
            .build_extract_value(outcome, 2, &format!("{name}_complete"))?
            .into_struct_value();
        self.extract_value_transport_parts(complete, &format!("{name}_complete"))
    }

    pub(in crate::llvm::codegen) fn effect_outcome_signal_op_tag(
        &mut self,
        _at: crate::span::Span,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let signal = self.effect_outcome_signal_struct(outcome_ptr, name)?;
        Ok(self
            .builder
            .build_extract_value(signal, 0, &format!("{name}_op_tag"))?
            .into_int_value())
    }

    pub(in crate::llvm::codegen) fn effect_outcome_signal_effect_instance_key(
        &mut self,
        _at: crate::span::Span,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let signal = self.effect_outcome_signal_struct(outcome_ptr, name)?;
        Ok(self
            .builder
            .build_extract_value(signal, 1, &format!("{name}_effect_instance_key"))?
            .into_int_value())
    }

    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn effect_outcome_resume_token(
        &mut self,
        _at: crate::span::Span,
        outcome_ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let signal = self.effect_outcome_signal_struct(outcome_ptr, name)?;
        Ok(self
            .builder
            .build_extract_value(signal, 3, &format!("{name}_resume_token"))?
            .into_pointer_value())
    }

    pub(in crate::llvm::codegen) fn is_task_transport_tuple_ty(&self, tuple_ty: TypeId) -> bool {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return false;
        };
        if elements.len() != 2 {
            return false;
        }

        let Some(first) = self.cg_ty_of(elements[0]) else {
            return false;
        };
        let Some(second) = self.cg_ty_of(elements[1]) else {
            return false;
        };

        matches!(first, CgTy::Int(IntTy { bits: 64, .. }))
            && matches!(second, CgTy::Ref | CgTy::String)
    }

    pub(in crate::llvm::codegen) fn split_task_transport_tuple_value(
        &mut self,
        at: crate::span::Span,
        transport: CgValue<'ctx>,
    ) -> Result<ValueTransportParts<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = transport.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task transport tuple value",
                at: at.into(),
            });
        };
        if !self.is_task_transport_tuple_ty(tuple_ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task transport tuple type",
                at: at.into(),
            });
        }
        let Some(raw) = transport.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task transport tuple raw value",
                at: at.into(),
            });
        };
        let tuple = raw.into_struct_value();
        let word_raw = self
            .builder
            .build_extract_value(tuple, 0, "task_transport_word")?
            .into_int_value();
        let gc_ref_raw = self
            .builder
            .build_extract_value(tuple, 1, "task_transport_gc_ref")?
            .into_pointer_value();

        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            unreachable!("validated above")
        };
        let Some(CgTy::Int(word_ty)) = self.cg_ty_of(elements[0]) else {
            unreachable!("validated above")
        };
        let word = self.cast_int(
            word_raw,
            word_ty,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;
        let gc_ref = self.builder.build_pointer_cast(
            gc_ref_raw,
            self.llvm_gc_i8_ptr_type(),
            "task_transport_gc_ref_cast",
        )?;
        Ok(ValueTransportParts { word, gc_ref })
    }

    pub(in crate::llvm::codegen) fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let to = IntTy {
            bits: 64,
            signed: false,
        };
        let i64_ty = self.context.i64_type();

        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(i64_ty.const_zero()),
            CgTy::Bool => {
                let raw = self.expect_cg_bool(value, "effect transport Bool word coercion");
                self.builder
                    .build_int_z_extend(raw, i64_ty, "bool_to_u64_word")
                    .map_err(Into::into)
            }
            CgTy::Int(from) if from.bits <= 64 => {
                let (raw, _) = self.expect_cg_int(value, "effect transport Int word coercion");
                self.cast_int(raw, from, to)
            }
            CgTy::Float64 => {
                let (raw, _) =
                    self.expect_cg_float(value, "effect transport Float64 word coercion");
                Ok(self
                    .builder
                    .build_bit_cast(raw, i64_ty, "f64_to_u64_bits")?
                    .into_int_value())
            }
            CgTy::Float32 => {
                let (raw, _) =
                    self.expect_cg_float(value, "effect transport Float32 word coercion");
                let bits32 = self
                    .builder
                    .build_bit_cast(raw, self.context.i32_type(), "f32_to_i32_bits")?
                    .into_int_value();
                self.builder
                    .build_int_z_extend(bits32, i64_ty, "i32_to_u64_word")
                    .map_err(Into::into)
            }
            CgTy::String | CgTy::Ref => {
                panic!("coerce_u64_word: verifier accepted GC ref as scalar transport word")
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "coerce composite value to u64 word",
                    at: at.into(),
                })
            }
            CgTy::Int(_) => {
                panic!("coerce_u64_word: verifier accepted integer wider than u64 transport word")
            }
        }
    }

    pub(in crate::llvm::codegen) fn encode_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        source_ty: Option<TypeId>,
        value: CgValue<'ctx>,
        name: &str,
    ) -> Result<ValueTransportParts<'ctx>, LlvmEmitError> {
        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(self.zero_value_transport_parts()),
            CgTy::Bool | CgTy::Float32 | CgTy::Float64 | CgTy::Int(_) => {
                let word = self.coerce_u64_word(at, value)?;
                Ok(ValueTransportParts {
                    word,
                    gc_ref: self.llvm_gc_i8_ptr_type().const_null(),
                })
            }
            CgTy::Ref => Ok(ValueTransportParts {
                word: self.context.i64_type().const_zero(),
                gc_ref: value
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect transport ref value",
                        at: at.into(),
                    })?
                    .into_pointer_value(),
            }),
            CgTy::String => Ok(ValueTransportParts {
                word: self.context.i64_type().const_zero(),
                gc_ref: self.builder.build_pointer_cast(
                    value
                        .value
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "effect transport string value",
                            at: at.into(),
                        })?
                        .into_pointer_value(),
                    self.llvm_gc_i8_ptr_type(),
                    &format!("{name}_string_ref"),
                )?,
            }),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Ok(ValueTransportParts {
                word: self.context.i64_type().const_zero(),
                gc_ref: self.box_composite_effect_transport_value(
                    at,
                    source_ty.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect transport composite source type",
                        at: at.into(),
                    })?,
                    value,
                    name,
                )?,
            }),
        }
    }

    fn load_composite_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        source_ty: TypeId,
        target: CgTy,
        gc_ref: PointerValue<'ctx>,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let box_ty = self.mir_value_box_object_type(at, source_ty, target)?;
        let box_ptr = self.builder.build_pointer_cast(
            gc_ref,
            self.llvm_ptr_type(self.gc_address_space()),
            &format!("{name}_box_ptr"),
        )?;
        let payload_ptr =
            self.builder
                .build_struct_gep(box_ty, box_ptr, 1, &format!("{name}_payload_gep"))?;
        Ok(self.builder.build_load(
            self.llvm_basic_type_of(at, target)?,
            payload_ptr,
            &format!("{name}_payload"),
        )?)
    }

    pub(in crate::llvm::codegen) fn decode_effect_transport_value_as(
        &mut self,
        at: crate::span::Span,
        source_ty: Option<TypeId>,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        match target {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool => {
                let is_true = self.builder.build_int_compare(
                    IntPredicate::NE,
                    word,
                    self.context.i64_type().const_zero(),
                    "effect_transport_to_bool",
                )?;
                Ok(CgValue::bool(is_true))
            }
            CgTy::Float64 => {
                let raw = self
                    .builder
                    .build_bit_cast(word, self.context.f64_type(), "effect_transport_to_f64")?
                    .into_float_value();
                Ok(CgValue::float(raw, CgTy::Float64))
            }
            CgTy::Float32 => {
                let bits32 = self.builder.build_int_truncate(
                    word,
                    self.context.i32_type(),
                    "effect_transport_to_i32",
                )?;
                let raw = self
                    .builder
                    .build_bit_cast(bits32, self.context.f32_type(), "effect_transport_to_f32")?
                    .into_float_value();
                Ok(CgValue::float(raw, CgTy::Float32))
            }
            CgTy::Int(int_ty) => {
                let raw = self.cast_int(word, from_u64, int_ty)?;
                Ok(CgValue::int(raw, int_ty))
            }
            CgTy::Ref => Ok(CgValue {
                ty: CgTy::Ref,
                value: Some(
                    self.builder
                        .build_pointer_cast(gc_ref, self.llvm_gc_i8_ptr_type(), "transport_ref")?
                        .into(),
                ),
            }),
            CgTy::String => Ok(CgValue {
                ty: CgTy::String,
                value: Some(
                    self.builder
                        .build_pointer_cast(
                            gc_ref,
                            self.llvm_scoop_string_ptr_type(),
                            "transport_string",
                        )?
                        .into(),
                ),
            }),
            CgTy::Tuple(tuple_ty) if self.is_task_transport_tuple_ty(tuple_ty) => {
                let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty)
                else {
                    unreachable!("validated above")
                };
                let first_cg =
                    self.cg_ty_of(elements[0])
                        .unwrap_or_else(|| {
                            panic!("decode_effect_transport_value: TypeStore equivalence verifier accepted unsupported task transport tuple first element type")
                        });
                let second_cg =
                    self.cg_ty_of(elements[1])
                        .unwrap_or_else(|| {
                            panic!("decode_effect_transport_value: TypeStore equivalence verifier accepted unsupported task transport tuple second element type")
                        });
                let first = self.decode_effect_transport_value(at, word, gc_ref, first_cg)?;
                let second = self.decode_effect_transport_value(at, word, gc_ref, second_cg)?;
                let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
                let first_raw = first.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task transport tuple first raw value",
                    at: at.into(),
                })?;
                let second_raw = second.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task transport tuple second raw value",
                    at: at.into(),
                })?;
                let mut tuple = llvm_tuple_ty.get_undef();
                tuple = self
                    .builder
                    .build_insert_value(tuple, first_raw, 0, "task_transport_tuple_word")?
                    .into_struct_value();
                tuple = self
                    .builder
                    .build_insert_value(tuple, second_raw, 1, "task_transport_tuple_gc_ref")?
                    .into_struct_value();
                Ok(CgValue {
                    ty: CgTy::Tuple(tuple_ty),
                    value: Some(tuple.into()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Ok(CgValue {
                ty: target,
                value: Some(self.load_composite_effect_transport_value(
                    at,
                    source_ty.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "decode effect transport composite source type",
                        at: at.into(),
                    })?,
                    target,
                    gc_ref,
                    "effect_transport_composite",
                )?),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn decode_effect_transport_value(
        &mut self,
        at: crate::span::Span,
        word: IntValue<'ctx>,
        gc_ref: PointerValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.decode_effect_transport_value_as(at, None, word, gc_ref, target)
    }
}
