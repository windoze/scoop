//! MIR constant + pattern-match lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_const(
        &mut self,
        span: crate::span::Span,
        value: &crate::mir::ConstValue,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match value {
            crate::mir::ConstValue::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            crate::mir::ConstValue::Char => {
                let text = self.current_source_slice(span)?;
                let value =
                    crate::syntax::char_literal::parse_char_literal(text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR char literal",
                            at: span.into(),
                        }
                    })?;
                Ok(CgValue::int(
                    self.context.i32_type().const_int(value as u64, false),
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                ))
            }
            crate::mir::ConstValue::Unit => Ok(CgValue::unit()),
            crate::mir::ConstValue::Int => {
                let int_ty = match expected.or_else(|| self.cg_ty_of(self.builtins.int)) {
                    Some(CgTy::Int(int_ty)) => int_ty,
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR int builtin type",
                            at: span.into(),
                        });
                    }
                };
                let bits = match self.int_literal_bits_from_source_span_if_present(span, int_ty)? {
                    Some(bits) => bits,
                    None => self.int_literal_bits_for_ty(span, int_ty)?,
                };
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(bits, false),
                    int_ty,
                ))
            }
            crate::mir::ConstValue::SynthInt(value) => {
                let int_ty = match expected.or_else(|| self.cg_ty_of(self.builtins.int)) {
                    Some(CgTy::Int(int_ty)) => int_ty,
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR synthesized int builtin type",
                            at: span.into(),
                        });
                    }
                };
                Ok(CgValue::int(
                    self.int_type(int_ty)
                        .const_int(*value as u64, int_ty.signed),
                    int_ty,
                ))
            }
            crate::mir::ConstValue::Float64 => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                Ok(CgValue::float(
                    self.context.f64_type().const_float(parsed.value),
                    CgTy::Float64,
                ))
            }
            crate::mir::ConstValue::Float32 => {
                let parsed = crate::syntax::float_literal::parse_float_literal(
                    self.current_source_slice(span)?,
                );
                Ok(CgValue::float(
                    self.context.f32_type().const_float(parsed.value),
                    CgTy::Float32,
                ))
            }
            crate::mir::ConstValue::String => self.codegen_string_literal(span),
            crate::mir::ConstValue::SynthString(value) => {
                self.codegen_string_literal_from_text(span, value)
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: &crate::mir::Operand,
        pattern: &crate::mir::Pattern,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let subject = self.codegen_mir_operand(span, subject, slots)?;
        let cond = self.codegen_mir_pattern_match_value(span, mir_types, subject, pattern)?;
        Ok(CgValue::bool(cond))
    }

    pub(in crate::llvm::codegen) fn codegen_mir_pattern_match_value(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        pattern: &crate::mir::Pattern,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pattern {
            crate::mir::Pattern::Else
            | crate::mir::Pattern::Wildcard
            | crate::mir::Pattern::Rest
            | crate::mir::Pattern::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            crate::mir::Pattern::Or { pats } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for pat in pats {
                    let pat_cond =
                        self.codegen_mir_pattern_match_value(span, mir_types, subject, pat)?;
                    cond = self
                        .builder
                        .build_or(cond, pat_cond, "pass_mir_pattern_or")?;
                }
                Ok(cond)
            }
            crate::mir::Pattern::Is { ty, metadata } => {
                self.codegen_mir_is_pattern_match(span, mir_types, subject, *ty, metadata)
            }
            crate::mir::Pattern::Tuple { elements } => {
                self.codegen_mir_tuple_pattern_match(span, mir_types, subject, elements)
            }
            crate::mir::Pattern::Variant { name, args } => {
                self.codegen_mir_variant_pattern_match(span, mir_types, subject, name, args)
            }
            crate::mir::Pattern::IntLit { raw } => {
                let (value, int_ty) =
                    subject.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern int subject",
                        at: span.into(),
                    })?;
                let expected = self.int_literal_bits_from_text_for_ty(span, raw, int_ty)?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.int_type(int_ty).const_int(expected, false),
                    "pass_mir_pattern_int_eq",
                )?)
            }
            crate::mir::Pattern::CharLit { value: expected } => {
                let (value, int_ty) =
                    subject.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern char subject",
                        at: span.into(),
                    })?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.int_type(int_ty).const_int(*expected as u64, false),
                    "pass_mir_pattern_char_eq",
                )?)
            }
            crate::mir::Pattern::StringLit { value } => {
                let CgTy::String = subject.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string subject",
                        at: span.into(),
                    });
                };
                let Some(BasicValueEnum::PointerValue(subject_ptr)) = subject.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string value",
                        at: span.into(),
                    });
                };
                let deferred_subject =
                    self.defer_gc_ref_pointer(span, "pass_mir_pattern_str_subject", subject_ptr)?;
                let expected = self.codegen_string_literal_from_text(span, value)?;
                let Some(BasicValueEnum::PointerValue(expected_ptr)) = expected.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string literal",
                        at: span.into(),
                    });
                };
                let subject_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    "pass_mir_pattern_str_subject_reload",
                    &deferred_subject,
                )?;
                let fn_val = self.declare_runtime_string_equals();
                let call = self.builder.build_call(
                    fn_val,
                    &[subject_ptr.into(), expected_ptr.into()],
                    "pass_mir_pattern_str_eq",
                )?;
                let raw_result = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string equals return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern string equals return type",
                        at: span.into(),
                    });
                };
                Ok(self.builder.build_int_compare(
                    IntPredicate::NE,
                    eq_i64,
                    self.context.i64_type().const_zero(),
                    "pass_mir_pattern_str_eq_bool",
                )?)
            }
            crate::mir::Pattern::BoolLit { value: expected } => {
                let value = subject
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR pattern bool subject",
                        at: span.into(),
                    })?;
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    self.context.bool_type().const_int(*expected as u64, false),
                    "pass_mir_pattern_bool_eq",
                )?)
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_is_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        target_ty: TypeId,
        metadata: &crate::mir::RuntimePatternTypeTestMetadata,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if metadata.target_ty != target_ty || metadata.descriptor.ty != target_ty {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is metadata",
                at: span.into(),
            });
        }
        let metadata_subject_ty = self
            .cg_ty_of_mir_type(mir_types, metadata.subject_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject metadata",
                at: span.into(),
            })?;
        if !self.cg_ty_layout_equivalent(metadata_subject_ty, subject.ty) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject type drift",
                at: span.into(),
            });
        }
        match metadata.static_fold {
            crate::mir::RuntimeTypeStaticFold::AlwaysTrue => {
                return Ok(self.context.bool_type().const_int(1, false));
            }
            crate::mir::RuntimeTypeStaticFold::AlwaysFalse => {
                return Ok(self.context.bool_type().const_int(0, false));
            }
            crate::mir::RuntimeTypeStaticFold::Dynamic => {}
        }
        if !self.runtime_pattern_type_descriptor_is_codegen_supported(mir_types, metadata) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is runtime type descriptor",
                at: span.into(),
            });
        }
        let target_ty = self
            .equivalent_runtime_ref_codegen_type_id(mir_types, metadata.target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target type",
                at: span.into(),
            })?;
        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target type",
                at: span.into(),
            })?;
        if !matches!(target_cg, CgTy::Ref | CgTy::String) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is target runtime type",
                at: span.into(),
            });
        }

        let subject = match subject.ty {
            CgTy::Ref => subject,
            CgTy::String => self.coerce_value(span, subject, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR pattern is subject type",
                    at: span.into(),
                });
            }
        };
        let Some(BasicValueEnum::PointerValue(subject_ptr)) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR pattern is subject value",
                at: span.into(),
            });
        };
        self.codegen_ref_is_instance_of(span, subject_ptr, target_ty)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_tuple_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        elements: &[crate::mir::Pattern],
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = subject.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern subject type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern tuple type",
                at: span.into(),
            });
        };
        let Some(raw) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern subject value",
                at: span.into(),
            });
        };
        let tuple_v = raw.into_struct_value();
        let (prefix_pats, has_rest) = match elements.last() {
            Some(crate::mir::Pattern::Rest) => {
                (&elements[..elements.len().saturating_sub(1)], true)
            }
            _ => (elements, false),
        };
        let pat_arity = prefix_pats.len();
        if (!has_rest && pat_arity != tuple_elems.len())
            || (has_rest && pat_arity > tuple_elems.len())
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple pattern arity",
                at: span.into(),
            });
        }

        let mut cond = self.context.bool_type().const_int(1, false);
        for (idx, pat) in prefix_pats.iter().enumerate() {
            let elem_ty = self.tuple_element_cg_ty(tuple_ty, idx).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple pattern element type",
                    at: span.into(),
                },
            )?;
            let elem_value = self.extract_mir_tuple_element_value(span, tuple_v, idx, elem_ty)?;
            let elem_cond =
                self.codegen_mir_pattern_match_value(span, mir_types, elem_value, pat)?;
            cond = self
                .builder
                .build_and(cond, elem_cond, "pass_mir_tuple_pattern_and")?;
        }
        Ok(cond)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_variant_pattern_match(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        subject: CgValue<'ctx>,
        variant_name: &str,
        args: &[crate::mir::Pattern],
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let CgTy::Enum(enum_ty) = subject.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern subject type",
                at: span.into(),
            });
        };
        let Some(raw) = subject.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern subject value",
                at: span.into(),
            });
        };
        let (repr, variant) = {
            let layout = self.cg_enum_layout(span, enum_ty)?;
            let repr = layout.repr;
            let variant = layout
                .variants
                .iter()
                .find(|variant| variant.name == variant_name)
                .cloned()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR unknown enum variant",
                    at: span.into(),
                })?;
            (repr, variant)
        };
        let (prefix_pats, has_rest) = match args.last() {
            Some(crate::mir::Pattern::Rest) => (&args[..args.len().saturating_sub(1)], true),
            _ => (args, false),
        };
        let expected_arity = variant.fields.len();
        let found_arity = prefix_pats.len();
        if (!has_rest && expected_arity != found_arity)
            || (has_rest && found_arity > expected_arity)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR variant pattern arity",
                at: span.into(),
            });
        }

        let tag = self.extract_mir_enum_tag_value(span, enum_ty, repr, raw)?;
        let expected = tag.get_type().const_int(variant.tag, false);
        let tag_eq = self.builder.build_int_compare(
            IntPredicate::EQ,
            tag,
            expected,
            "pass_mir_variant_tag_eq",
        )?;
        if !prefix_pats
            .iter()
            .any(Self::mir_pattern_needs_payload_match)
        {
            return Ok(tag_eq);
        }

        let subject_ptr = self.create_entry_alloca(span, "pass_mir_variant_subject", subject.ty)?;
        let _ = self.store_local_value(span, subject_ptr, subject.ty, subject)?;
        let current_bb =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = current_bb
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let payload_bb = self
            .context
            .append_basic_block(func, "pass_mir_variant_payload");
        let merge_bb = self
            .context
            .append_basic_block(func, "pass_mir_variant_merge");
        self.builder
            .build_conditional_branch(tag_eq, payload_bb, merge_bb)?;

        self.builder.position_at_end(payload_bb);
        let mut payload_cond = self.context.bool_type().const_int(1, false);
        for (idx, pat) in prefix_pats.iter().enumerate() {
            if !Self::mir_pattern_needs_payload_match(pat) {
                continue;
            }
            let extracted = self.extract_matched_when_variant_field_value(
                enum_ty,
                repr,
                &variant,
                idx,
                span,
                subject_ptr,
            )?;
            let field_cond =
                self.codegen_mir_pattern_match_value(span, mir_types, extracted, pat)?;
            payload_cond =
                self.builder
                    .build_and(payload_cond, field_cond, "pass_mir_variant_payload_and")?;
        }

        let payload_tail =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR variant payload tail",
                    at: span.into(),
                })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "pass_mir_variant_match")?;
        let no_match = self.context.bool_type().const_int(0, false);
        phi.add_incoming(&[(&no_match, current_bb), (&payload_cond, payload_tail)]);
        Ok(phi.as_basic_value().into_int_value())
    }

    pub(in crate::llvm::codegen) fn codegen_mir_pattern_extract(
        &mut self,
        span: crate::span::Span,
        subject: &crate::mir::Operand,
        path: &[crate::mir::PatternBindingStep],
        slots: &[MirLocalSlot<'ctx>],
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut current = self.codegen_mir_operand(span, subject, slots)?;
        for step in path {
            current = match step {
                crate::mir::PatternBindingStep::TupleIndex(index) => {
                    let CgTy::Tuple(tuple_ty) = current.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple subject type",
                            at: span.into(),
                        });
                    };
                    let Some(raw) = current.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple subject value",
                            at: span.into(),
                        });
                    };
                    let elem_ty = self.tuple_element_cg_ty(tuple_ty, *index).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract tuple field type",
                            at: span.into(),
                        },
                    )?;
                    self.extract_mir_tuple_element_value(
                        span,
                        raw.into_struct_value(),
                        *index,
                        elem_ty,
                    )?
                }
                crate::mir::PatternBindingStep::VariantField {
                    variant,
                    field_index,
                } => {
                    let CgTy::Enum(enum_ty) = current.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract variant subject type",
                            at: span.into(),
                        });
                    };
                    let layout = self.cg_enum_layout(span, enum_ty)?;
                    let variant = layout
                        .variants
                        .iter()
                        .find(|item| item.name == *variant)
                        .cloned()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "pass MIR pattern extract unknown enum variant",
                            at: span.into(),
                        })?;
                    let subject_ptr =
                        self.create_entry_alloca(span, "pass_mir_extract_subject", current.ty)?;
                    let _ = self.store_local_value(span, subject_ptr, current.ty, current)?;
                    self.extract_matched_when_variant_field_value(
                        enum_ty,
                        layout.repr,
                        &variant,
                        *field_index,
                        span,
                        subject_ptr,
                    )?
                }
            };
        }
        self.coerce_value(span, current, target_cg)
    }

    pub(in crate::llvm::codegen) fn extract_mir_tuple_element_value(
        &mut self,
        span: crate::span::Span,
        tuple_v: inkwell::values::StructValue<'ctx>,
        index: usize,
        elem_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let raw = self
            .builder
            .build_extract_value(tuple_v, index as u32, "pass_mir_tuple_elem")?;
        self.cg_value_from_loaded(span, elem_ty, raw)
    }

    pub(in crate::llvm::codegen) fn extract_mir_enum_tag_value(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        repr: CgEnumRepr,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match repr {
            CgEnumRepr::TaggedUnion => Ok(self
                .builder
                .build_extract_value(raw.into_struct_value(), 0, "pass_mir_when_tag")?
                .into_int_value()),
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                let is_none = match storage {
                    NicheStorage::Pointer => {
                        let ptr = raw.into_pointer_value();
                        if none_value != 0 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "pass MIR niche pointer none_value",
                                at: span.into(),
                            });
                        }
                        self.builder.build_is_null(ptr, "pass_mir_option_is_none")?
                    }
                    NicheStorage::U8 => {
                        let value = raw.into_int_value();
                        let expected = self.context.i8_type().const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            value,
                            expected,
                            "pass_mir_option_is_none",
                        )?
                    }
                };
                let some_tag = self.context.i32_type().const_int(0, false);
                let none_tag = self.context.i32_type().const_int(1, false);
                Ok(self
                    .builder
                    .build_select(is_none, none_tag, some_tag, "pass_mir_option_tag")?
                    .into_int_value())
            }
            CgEnumRepr::ValueOnly { .. } => {
                let _ = enum_ty;
                Ok(raw.into_int_value())
            }
        }
    }

    pub(in crate::llvm::codegen) fn mir_pattern_needs_payload_match(
        pattern: &crate::mir::Pattern,
    ) -> bool {
        !matches!(
            pattern,
            crate::mir::Pattern::Else
                | crate::mir::Pattern::Wildcard
                | crate::mir::Pattern::Rest
                | crate::mir::Pattern::Bind { .. }
        )
    }
}
