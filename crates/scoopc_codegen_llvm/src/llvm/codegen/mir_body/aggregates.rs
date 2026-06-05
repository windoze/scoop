//! MIR tuple / struct / closure-impl make + size-of lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_make_tuple(
        &mut self,
        span: crate::span::Span,
        _body: &mir_source::Body,
        _mir_types: &TypeStore,
        elements: &[mir_source::Operand],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = target_cg else {
            panic!("codegen_mir_make_tuple: MIR verifier accepted non-tuple aggregate target type");
        };
        let element_tys = {
            let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) =
                self.types.kind(tuple_ty.inner())
            else {
                panic!(
                    "codegen_mir_make_tuple: MIR verifier accepted tuple target without tuple schema"
                );
            };
            element_tys.clone()
        };
        if element_tys.len() != elements.len() {
            panic!("codegen_mir_make_tuple: MIR verifier accepted tuple aggregate arity drift");
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (operand, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self.try_cg_ty_of_type_id(*elem_ty).unwrap_or_else(|| {
                panic!(
                    "codegen_mir_make_tuple: MIR verifier accepted unsupported tuple element type"
                )
            });
            let value = self.codegen_mir_operand_expected(span, operand, slots, Some(elem_cg))?;
            let coerced = self.coerce_value(span, value, elem_cg)?;
            let deferred = self.defer_gc_sensitive_cg_value(
                span,
                &format!("pass_mir_tuple_elem_{idx}"),
                coerced,
            )?;
            deferred_elements.push((idx, span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, elem_span, deferred) in deferred_elements {
            let materialized = self.materialize_deferred_cg_value(
                elem_span,
                &format!("pass_mir_tuple_elem_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized.value.unwrap_or_else(|| {
                    panic!("codegen_mir_make_tuple: MIR verifier accepted valueless tuple element")
                }),
            };
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, "pass_mir_tuple_insert")?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_lir_make_tuple(
        &mut self,
        span: crate::span::Span,
        elements: &[LirOperand],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = target_cg else {
            panic!("codegen_lir_make_tuple: LIR verifier accepted non-tuple aggregate target type");
        };
        let element_tys = {
            let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) =
                self.types.kind(tuple_ty.inner())
            else {
                panic!(
                    "codegen_lir_make_tuple: LIR verifier accepted tuple target without tuple schema"
                );
            };
            element_tys.clone()
        };
        if element_tys.len() != elements.len() {
            panic!("codegen_lir_make_tuple: LIR verifier accepted tuple aggregate arity drift");
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (operand, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self.try_cg_ty_of_type_id(*elem_ty).unwrap_or_else(|| {
                panic!(
                    "codegen_lir_make_tuple: LIR verifier accepted unsupported tuple element type"
                )
            });
            let value = self.codegen_lir_operand_expected(span, operand, slots, Some(elem_cg))?;
            let coerced = self.coerce_value(span, value, elem_cg)?;
            let deferred =
                self.defer_gc_sensitive_cg_value(span, &format!("lir_tuple_elem_{idx}"), coerced)?;
            deferred_elements.push((idx, span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();
        for (idx, elem_span, deferred) in deferred_elements {
            let materialized = self.materialize_deferred_cg_value(
                elem_span,
                &format!("lir_tuple_elem_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized.value.unwrap_or_else(|| {
                    panic!("codegen_lir_make_tuple: LIR verifier accepted valueless tuple element")
                }),
            };
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, "lir_tuple_insert")?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_size_of(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg_cg = self
            .cg_ty_of_mir_type(mir_types, value_ty)
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "codegen_mir_size_of",
                    "unsupported sizeOf argument type",
                )
            });
        let llvm_ty = self.llvm_basic_type_of(span, arg_cg)?;
        let bytes = self.store_size_bytes_of_basic_type(llvm_ty);
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(bytes, false);
        Ok(CgValue::int(raw, value_word))
    }

    pub(in crate::llvm::codegen) fn codegen_mir_kind_of(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let kind = self.mir_array_elem_kind(span, mir_types, value_ty)?;
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(kind, false);
        Ok(CgValue::int(raw, value_word))
    }

    pub(in crate::llvm::codegen) fn codegen_mir_align_of(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg_cg = self
            .cg_ty_of_mir_type(mir_types, value_ty)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_mir_align_of: MIR verifier accepted unsupported alignOf argument type"
                )
            });
        let llvm_ty = self.llvm_basic_type_of(span, arg_cg)?;
        let align = self.abi_align_bytes_of_basic_type(llvm_ty);
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(u64::from(align), false);
        Ok(CgValue::int(raw, value_word))
    }

    fn mir_array_elem_kind(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<u64, LlvmEmitError> {
        let arg_cg = self
            .cg_ty_of_mir_type(mir_types, value_ty)
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "mir_array_elem_kind",
                    "unsupported kindOf argument type",
                )
            });
        match arg_cg {
            CgTy::String | CgTy::Ref => Ok(2),
            CgTy::Unit
            | CgTy::Never
            | CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_) => Ok(1),
            CgTy::Enum(enum_ty) => {
                let layout = self.cg_enum_layout(span, enum_ty)?;
                if matches!(layout.repr, CgEnumRepr::ValueOnly { .. })
                    || layout
                        .variants
                        .iter()
                        .all(|variant| variant.fields.is_empty())
                {
                    Ok(1)
                } else {
                    Ok(3)
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) => Ok(3),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_mir_desc_of(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let arg_cg = self
            .cg_ty_of_mir_type(mir_types, value_ty)
            .unwrap_or_else(|| {
                self.panic_verified_intrinsic_contract(
                    "codegen_mir_desc_of",
                    "unsupported descOf argument type",
                )
            });
        if !matches!(arg_cg, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) {
            return Ok(CgValue::int(
                self.int_type(value_word).const_zero(),
                value_word,
            ));
        }

        let body_fqn = self.current_codegen_body_fqn();
        let metadata = mir_source::ValueTransportMetadata::plain(
            value_ty,
            mir_source::MirTransportKind::ArrayElement,
        );
        let descriptor = self.get_or_create_value_composite_transport_descriptor_global(
            &body_fqn, span, mir_types, &metadata,
        )?;
        let ptr = descriptor.as_pointer_value();
        let ptr_int_ty = self.llvm_ptr_sized_int_type(Some(ptr.get_type().get_address_space()));
        let raw = self
            .builder
            .build_ptr_to_int(ptr, ptr_int_ty, "mir_desc_of_composite")?;
        let raw = self.cast_int(
            raw,
            IntTy {
                bits: ptr_int_ty.get_bit_width(),
                signed: false,
            },
            value_word,
        )?;
        Ok(CgValue::int(raw, value_word))
    }

    pub(in crate::llvm::codegen) fn codegen_mir_make_struct(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        fields: &[mir_source::StructLitField],
        transport: &mir_source::AggregateTransportMetadata,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let aggregate_ty = self
            .equivalent_codegen_type_id(mir_types, transport.aggregate_ty)
            .unwrap_or_else(|| {
                panic!("codegen_mir_make_struct: MIR verifier accepted aggregate TypeStore drift")
            });
        if let Some((layout_field, field_cg)) =
            self.scalar_layout_struct_field(aggregate_ty, target_cg)?
        {
            let Some(init) = fields.iter().find(|field| field.name == layout_field.name) else {
                unreachable!(
                    "typecheck must reject MIR scalar-layout struct literals missing required fields"
                );
            };
            let value =
                self.codegen_mir_operand_expected(init.span, &init.value, slots, Some(field_cg))?;
            let coerced = self.coerce_value(init.span, value, field_cg)?;
            return self.coerce_value(span, coerced, target_cg);
        }

        let CgTy::Struct(struct_ty) = target_cg else {
            panic!(
                "codegen_mir_make_struct: MIR verifier accepted non-struct aggregate target type"
            );
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            panic!(
                "codegen_mir_make_struct: MIR verifier accepted struct target without nominal schema"
            );
        };
        let layout_key = self.nominal_layout_key(nominal);
        let layout = self.struct_layouts.get(&layout_key).unwrap_or_else(|| {
            panic!("codegen_mir_make_struct: MIR verifier accepted struct without layout")
        });
        if layout.fields.len() != fields.len() {
            panic!(
                "codegen_mir_make_struct: MIR verifier accepted struct literal field count drift"
            );
        }

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut deferred_fields: Vec<(u32, String, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());

        for (idx, layout_field) in layout.fields.iter().enumerate() {
            let mut matches = fields
                .iter()
                .filter(|field| field.name == layout_field.name);
            let Some(init) = matches.next() else {
                // User-facing struct literal field coverage is owned by typecheck.
                unreachable!(
                    "typecheck must reject MIR struct literals missing required fields before LLVM codegen"
                );
            };
            if matches.next().is_some() {
                panic!(
                    "codegen_mir_make_struct: MIR verifier accepted duplicate struct literal field"
                );
            }

            let field_cg = self.cg_ty_of_layout_field(
                init.span,
                layout_field.ty,
                layout_field.ty_fqn.as_deref(),
            )?;
            let value =
                self.codegen_mir_operand_expected(init.span, &init.value, slots, Some(field_cg))?;
            let coerced = if field_cg == CgTy::Unit {
                CgValue::unit()
            } else if value.ty != field_cg {
                self.coerce_value(init.span, value, field_cg)?
            } else {
                value
            };
            let deferred = self.defer_gc_sensitive_cg_value(
                init.span,
                &format!("pass_mir_struct_field_{idx}"),
                coerced,
            )?;
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, layout_field.name.clone(), init.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, field_span, deferred)) in
            deferred_fields.into_iter().enumerate()
        {
            let materialized = self.materialize_deferred_cg_value(
                field_span,
                &format!("pass_mir_struct_field_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized.value.unwrap_or_else(|| {
                    panic!("codegen_mir_make_struct: MIR verifier accepted valueless struct field")
                }),
            };
            agg = self.builder.build_insert_value(
                agg,
                raw,
                llvm_idx,
                &format!("pass_mir_struct_insert_{field_name}"),
            )?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_lir_make_struct(
        &mut self,
        span: crate::span::Span,
        source_types: &TypeStore,
        fields: &[crate::effect_lowered::LirStructLitField],
        transport: &mir_source::AggregateTransportMetadata,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let aggregate_ty = self
            .equivalent_codegen_type_id(source_types, transport.aggregate_ty)
            .unwrap_or_else(|| {
                panic!("codegen_lir_make_struct: LIR verifier accepted aggregate TypeStore drift")
            });
        if let Some((layout_field, field_cg)) =
            self.scalar_layout_struct_field(aggregate_ty, target_cg)?
        {
            let Some(init) = fields.iter().find(|field| field.name == layout_field.name) else {
                unreachable!(
                    "typecheck must reject LIR scalar-layout struct literals missing required fields"
                );
            };
            let value =
                self.codegen_lir_operand_expected(init.span, &init.value, slots, Some(field_cg))?;
            let coerced = self.coerce_value(init.span, value, field_cg)?;
            return self.coerce_value(span, coerced, target_cg);
        }

        let CgTy::Struct(struct_ty) = target_cg else {
            panic!(
                "codegen_lir_make_struct: LIR verifier accepted non-struct aggregate target type"
            );
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty.inner())
        else {
            panic!(
                "codegen_lir_make_struct: LIR verifier accepted struct target without nominal schema"
            );
        };
        let layout_key = self.nominal_layout_key(nominal);
        let layout = self.struct_layouts.get(&layout_key).unwrap_or_else(|| {
            panic!("codegen_lir_make_struct: LIR verifier accepted struct without layout")
        });
        if layout.fields.len() != fields.len() {
            panic!(
                "codegen_lir_make_struct: LIR verifier accepted struct literal field count drift"
            );
        }

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut deferred_fields: Vec<(u32, String, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(layout.fields.len());

        for (idx, layout_field) in layout.fields.iter().enumerate() {
            let mut matches = fields
                .iter()
                .filter(|field| field.name == layout_field.name);
            let Some(init) = matches.next() else {
                unreachable!(
                    "typecheck must reject LIR struct literals missing required fields before LLVM codegen"
                );
            };
            if matches.next().is_some() {
                panic!(
                    "codegen_lir_make_struct: LIR verifier accepted duplicate struct literal field"
                );
            }

            let field_cg = self.cg_ty_of_layout_field(
                init.span,
                layout_field.ty,
                layout_field.ty_fqn.as_deref(),
            )?;
            let value =
                self.codegen_lir_operand_expected(init.span, &init.value, slots, Some(field_cg))?;
            let coerced = if field_cg == CgTy::Unit {
                CgValue::unit()
            } else if value.ty != field_cg {
                self.coerce_value(init.span, value, field_cg)?
            } else {
                value
            };
            let deferred = self.defer_gc_sensitive_cg_value(
                init.span,
                &format!("lir_struct_field_{idx}"),
                coerced,
            )?;
            let llvm_idx = self
                .shared_caches
                .pack_field_indices
                .borrow()
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);
            deferred_fields.push((llvm_idx, layout_field.name.clone(), init.span, deferred));
        }

        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        for (idx, (llvm_idx, field_name, field_span, deferred)) in
            deferred_fields.into_iter().enumerate()
        {
            let materialized = self.materialize_deferred_cg_value(
                field_span,
                &format!("lir_struct_field_reload_{idx}"),
                deferred,
            )?;
            let raw: BasicValueEnum<'ctx> = match materialized.ty {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => materialized.value.unwrap_or_else(|| {
                    panic!("codegen_lir_make_struct: LIR verifier accepted valueless struct field")
                }),
            };
            agg = self.builder.build_insert_value(
                agg,
                raw,
                llvm_idx,
                &format!("lir_struct_insert_{field_name}"),
            )?;
        }

        Ok(CgValue {
            ty: target_cg,
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_tuple_get(
        &mut self,
        span: crate::span::Span,
        body: &mir_source::Body,
        mir_types: &TypeStore,
        tuple: &mir_source::Operand,
        index: usize,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let tuple_ty = self.mir_operand_type_id(body, tuple).unwrap_or_else(|| {
            panic!("codegen_mir_tuple_get: MIR verifier accepted tuple get without operand type")
        });
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = mir_types.kind(tuple_ty) else {
            panic!("codegen_mir_tuple_get: MIR verifier accepted tuple get on non-tuple type");
        };
        let elem_ty = *elements.get(index).unwrap_or_else(|| {
            panic!("codegen_mir_tuple_get: MIR verifier accepted tuple index drift")
        });
        let elem_cg = self
            .cg_ty_of_mir_type(mir_types, elem_ty)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_mir_tuple_get: MIR verifier accepted unsupported tuple element type"
                )
            });
        let tuple_cg = self.mir_operand_cg_ty(body, mir_types, tuple).unwrap_or_else(|| {
            panic!("codegen_mir_tuple_get: TypeStore equivalence verifier accepted unsupported tuple operand codegen type")
        });
        let value = self.codegen_mir_operand_expected(span, tuple, slots, Some(tuple_cg))?;
        let tuple_v = value
            .value
            .unwrap_or_else(|| {
                panic!("codegen_mir_tuple_get: MIR verifier accepted valueless tuple operand")
            })
            .into_struct_value();
        self.extract_mir_tuple_element_value(span, tuple_v, index, elem_cg)
    }

    pub(in crate::llvm::codegen) fn codegen_lir_tuple_get(
        &mut self,
        span: crate::span::Span,
        body: &LirExecutableBody,
        source_types: &TypeStore,
        tuple: &LirOperand,
        index: usize,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let tuple_ty = self.lir_operand_type_id(body, tuple).unwrap_or_else(|| {
            panic!("codegen_lir_tuple_get: LIR verifier accepted tuple get without operand type")
        });
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = source_types.kind(tuple_ty) else {
            panic!("codegen_lir_tuple_get: LIR verifier accepted tuple get on non-tuple type");
        };
        let elem_ty = *elements.get(index).unwrap_or_else(|| {
            panic!("codegen_lir_tuple_get: LIR verifier accepted tuple index drift")
        });
        let elem_cg = self
            .cg_ty_of_mir_type(source_types, elem_ty)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_lir_tuple_get: LIR verifier accepted unsupported tuple element type"
                )
            });
        let tuple_cg = self.lir_operand_cg_ty(body, source_types, tuple).unwrap_or_else(|| {
            panic!("codegen_lir_tuple_get: TypeStore equivalence verifier accepted unsupported tuple operand codegen type")
        });
        let value = self.codegen_lir_operand_expected(span, tuple, slots, Some(tuple_cg))?;
        let tuple_v = value
            .value
            .unwrap_or_else(|| {
                panic!("codegen_lir_tuple_get: LIR verifier accepted valueless tuple operand")
            })
            .into_struct_value();
        self.extract_mir_tuple_element_value(span, tuple_v, index, elem_cg)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_make_closure(
        &mut self,
        span: crate::span::Span,
        env: &mir_source::Operand,
        fn_ptr: &str,
        env_contract: &mir_source::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_make_closure_impl(
            span,
            env,
            fn_ptr,
            env_contract,
            mir_types,
            env_cg,
            target_cg,
            slots,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_make_closure(
        &mut self,
        span: crate::span::Span,
        env: &LirOperand,
        fn_ptr: LirCallableId,
        env_contract: &mir_source::ClosureEnvTransportMetadata,
        source_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_lir_make_closure_impl(
            span,
            env,
            fn_ptr,
            env_contract,
            source_types,
            env_cg,
            target_cg,
            slots,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_make_closure_with_target_fn_ptr(
        &mut self,
        span: crate::span::Span,
        env: &LirOperand,
        fn_ptr: LirCallableId,
        env_contract: &mir_source::ClosureEnvTransportMetadata,
        source_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: PointerValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_lir_make_closure_impl(
            span,
            env,
            fn_ptr,
            env_contract,
            source_types,
            env_cg,
            target_cg,
            slots,
            Some(target_fn_ptr),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_make_closure_with_target_fn_ptr(
        &mut self,
        span: crate::span::Span,
        env: &mir_source::Operand,
        fn_ptr: &str,
        env_contract: &mir_source::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: PointerValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_mir_make_closure_impl(
            span,
            env,
            fn_ptr,
            env_contract,
            mir_types,
            env_cg,
            target_cg,
            slots,
            Some(target_fn_ptr),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_lir_make_closure_impl(
        &mut self,
        span: crate::span::Span,
        env: &LirOperand,
        fn_ptr: LirCallableId,
        env_contract: &mir_source::ClosureEnvTransportMetadata,
        source_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: Option<PointerValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            panic!(
                "codegen_lir_make_closure_impl: LIR verifier accepted non-reference closure target"
            )
        }

        let (fn_root, carrier_key) = {
            let program = self.published_late_lowered_program().unwrap_or_else(|| {
                panic!("codegen_lir_make_closure_impl: missing published LIR program")
            });
            let callable = program.callable_by_id(fn_ptr).unwrap_or_else(|| {
                panic!(
                    "codegen_lir_make_closure_impl: LIR verifier accepted unknown closure callable id"
                )
            });
            let key = callable_carrier_target_key_for_ref(
                program,
                CallableCarrierKind::ClosureObject,
                scoopc_lir_facts::LirCallableRef::Local(fn_ptr),
                "LIR closure carrier target",
            )?;
            (callable.root_fqn().to_string(), key)
        };

        let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys_from_contract(
            span,
            &fn_root,
            source_types,
            env_cg,
            env_contract,
        )?;

        let deferred_env = if capture_field_cgs.is_empty() {
            None
        } else {
            let value = self.codegen_lir_operand_expected(span, env, slots, Some(env_cg))?;
            let coerced = self.coerce_value(span, value, env_cg)?;
            Some(self.defer_gc_sensitive_cg_value(span, "lir_closure_env", coerced)?)
        };

        let closure_obj_ty = self.llvm_closure_object_type();
        let obj_size_bytes = self.target_data.get_store_size(&closure_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let closure_desc =
            self.get_or_create_mir_closure_object_type_desc_global(span, &fn_root)?;
        let closure_desc_i8 = self.builder.build_pointer_cast(
            closure_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "lir_closure_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[closure_desc_i8.into(), size_v.into()],
            "rt_alloc_lir_closure",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .expect("scoop_alloc_typed closure allocation must return a value");
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            panic!("scoop_alloc_typed closure allocation must return a pointer");
        };

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self
            .builder
            .build_pointer_cast(obj_i8, obj_ptr_ty, "lir_closure_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "lir_closure_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_closure_obj_init",
            &deferred_obj,
        )?;
        let env_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 1, "lir_closure_env_gep")?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(gc_i8_ptr_ty.const_null().into()),
            },
        )?;

        let env_i8 = if capture_field_cgs.is_empty() {
            gc_i8_ptr_ty.const_null()
        } else {
            let closure_key = self.stable_closure_key_for_lir_source_callable(&fn_root, span)?;
            let env_ty =
                self.mir_closure_env_object_type(span, &closure_key, &capture_field_cgs)?;
            let env_size_bytes = self.target_data.get_store_size(&env_ty);
            let env_size_v = self.context.i64_type().const_int(env_size_bytes, false);
            let env_desc =
                self.get_or_create_mir_closure_env_type_desc_global(span, &closure_key, env_ty)?;
            let env_desc_i8 = self.builder.build_pointer_cast(
                env_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "lir_closure_env_desc_i8",
            )?;
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt_alloc,
                &[env_desc_i8.into(), env_size_v.into()],
                "rt_alloc_lir_closure_env",
            )?;
            let raw = call
                .try_as_basic_value()
                .basic()
                .expect("scoop_alloc_typed closure env allocation must return a value");
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                panic!("scoop_alloc_typed closure env allocation must return a pointer");
            };

            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let env_ptr =
                self.builder
                    .build_pointer_cast(env_i8, env_ptr_ty, "lir_closure_env_ptr")?;
            let deferred_env_obj =
                self.defer_gc_ref_pointer(span, "lir_closure_env_root", env_ptr)?;
            let env_value = self.materialize_deferred_cg_value(
                span,
                "lir_closure_env_reload",
                deferred_env.expect("non-empty env must have been deferred"),
            )?;
            let tuple_v = env_value
                .value
                .unwrap_or_else(|| {
                    panic!(
                        "codegen_lir_make_closure_impl: LIR verifier accepted non-value closure env"
                    )
                })
                .into_struct_value();
            for (idx, field_cg) in capture_field_cgs.iter().enumerate() {
                let env_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    "lir_closure_env_field_reload",
                    &deferred_env_obj,
                )?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    "lir_closure_env_field_gep",
                )?;
                let field_value =
                    self.extract_mir_tuple_element_value(span, tuple_v, idx, *field_cg)?;
                let _ = self.store_local_value(span, field_gep, *field_cg, field_value)?;
            }
            env_i8
        };
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_closure_obj_store_env",
            &deferred_obj,
        )?;
        let env_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 1, "lir_closure_env_gep")?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(env_i8.into()),
            },
        )?;

        let use_plain_fallback = self.plain_callable_carrier_fallback_allowed(carrier_key);
        let fallback_target = if target_fn_ptr.is_some()
            || (self.callable_carrier_contract_enabled() && !use_plain_fallback)
        {
            self.llvm_i8_ptr_type().const_null()
        } else if let Some(plain_entry) = self
            .module
            .get_function(&self.lir_source_closure_body_symbol(&fn_root, span)?)
        {
            plain_entry.as_global_value().as_pointer_value()
        } else {
            self.ensure_lir_source_closure_callable_defined(span, &fn_root)?
                .as_global_value()
                .as_pointer_value()
        };
        let fn_ptr = match target_fn_ptr {
            Some(ptr) => ptr,
            None => self.callable_carrier_target_fn_ptr(carrier_key, &fn_root, fallback_target)?,
        };
        let fn_i8 = self
            .builder
            .build_pointer_cast(fn_ptr, i8_ptr_ty, "lir_closure_fn_i8")?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_closure_obj_store_fn",
            &deferred_obj,
        )?;
        let fn_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 2, "lir_closure_fn_gep")?;
        let _ = self.builder.build_store(fn_gep, fn_i8)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "lir_closure_obj_return",
            &deferred_obj,
        )?;
        let obj_i8 =
            self.builder
                .build_pointer_cast(obj_ptr, gc_i8_ptr_ty, "lir_closure_obj_i8")?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_make_closure_impl(
        &mut self,
        span: crate::span::Span,
        env: &mir_source::Operand,
        fn_ptr: &str,
        env_contract: &mir_source::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: Option<PointerValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            panic!(
                "codegen_mir_make_closure_impl: MIR verifier accepted non-reference closure target"
            )
        }

        let capture_field_cgs = self.mir_closure_env_capture_element_cg_tys_from_contract(
            span,
            fn_ptr,
            mir_types,
            env_cg,
            env_contract,
        )?;

        let deferred_env = if capture_field_cgs.is_empty() {
            None
        } else {
            let value = self.codegen_mir_operand_expected(span, env, slots, Some(env_cg))?;
            let coerced = self.coerce_value(span, value, env_cg)?;
            Some(self.defer_gc_sensitive_cg_value(span, "pass_mir_closure_env", coerced)?)
        };

        let closure_obj_ty = self.llvm_closure_object_type();
        let obj_size_bytes = self.target_data.get_store_size(&closure_obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let closure_desc = self.get_or_create_mir_closure_object_type_desc_global(span, fn_ptr)?;
        let closure_desc_i8 = self.builder.build_pointer_cast(
            closure_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "pass_mir_closure_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[closure_desc_i8.into(), size_v.into()],
            "rt_alloc_pass_mir_closure",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .expect("scoop_alloc_typed closure allocation must return a value");
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            panic!("scoop_alloc_typed closure allocation must return a pointer");
        };

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr =
            self.builder
                .build_pointer_cast(obj_i8, obj_ptr_ty, "pass_mir_closure_obj_ptr")?;
        let deferred_obj = self.defer_gc_ref_pointer(span, "pass_mir_closure_obj_root", obj_ptr)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_init",
            &deferred_obj,
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_obj_ty,
            obj_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(gc_i8_ptr_ty.const_null().into()),
            },
        )?;

        let env_i8 = if capture_field_cgs.is_empty() {
            gc_i8_ptr_ty.const_null()
        } else {
            let closure_key = self.stable_closure_key_for_lir_source_callable(fn_ptr, span)?;
            let env_ty =
                self.mir_closure_env_object_type(span, &closure_key, &capture_field_cgs)?;
            let env_size_bytes = self.target_data.get_store_size(&env_ty);
            let env_size_v = self.context.i64_type().const_int(env_size_bytes, false);
            let env_desc =
                self.get_or_create_mir_closure_env_type_desc_global(span, &closure_key, env_ty)?;
            let env_desc_i8 = self.builder.build_pointer_cast(
                env_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "pass_mir_closure_env_desc_i8",
            )?;
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt_alloc,
                &[env_desc_i8.into(), env_size_v.into()],
                "rt_alloc_pass_mir_closure_env",
            )?;
            let raw = call
                .try_as_basic_value()
                .basic()
                .expect("scoop_alloc_typed closure env allocation must return a value");
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                panic!("scoop_alloc_typed closure env allocation must return a pointer");
            };

            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let env_ptr =
                self.builder
                    .build_pointer_cast(env_i8, env_ptr_ty, "pass_mir_closure_env_ptr")?;
            let deferred_env_obj =
                self.defer_gc_ref_pointer(span, "pass_mir_closure_env_root", env_ptr)?;
            let env_value = self.materialize_deferred_cg_value(
                span,
                "pass_mir_closure_env_reload",
                deferred_env.expect("non-empty env must have been deferred"),
            )?;
            let tuple_v = env_value
                .value
                .unwrap_or_else(|| {
                    panic!(
                        "codegen_mir_make_closure_impl: MIR verifier accepted non-value closure env"
                    )
                })
                .into_struct_value();
            for (idx, field_cg) in capture_field_cgs.iter().enumerate() {
                let env_ptr = self.reload_deferred_gc_ref_without_clearing(
                    span,
                    "pass_mir_closure_env_field_reload",
                    &deferred_env_obj,
                )?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    "pass_mir_closure_env_field_gep",
                )?;
                let field_value =
                    self.extract_mir_tuple_element_value(span, tuple_v, idx, *field_cg)?;
                let _ = self.store_local_value(span, field_gep, *field_cg, field_value)?;
            }
            env_i8
        };
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_store_env",
            &deferred_obj,
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_obj_ty,
            obj_ptr,
            1,
            "pass_mir_closure_env_gep",
        )?;
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(env_i8.into()),
            },
        )?;

        let carrier_key = if let Some((callable_id, _, _)) = self.lir_source_callable(fn_ptr) {
            let program = self.expect_active_lir_program("MIR closure carrier target");
            Some(callable_carrier_target_key_for_ref(
                program,
                CallableCarrierKind::ClosureObject,
                scoopc_lir_facts::LirCallableRef::Local(callable_id),
                "MIR closure carrier target",
            )?)
        } else {
            None
        };
        let use_plain_fallback =
            carrier_key.is_some_and(|key| self.plain_callable_carrier_fallback_allowed(key));
        let fallback_target = if target_fn_ptr.is_some() {
            self.llvm_i8_ptr_type().const_null()
        } else if self.callable_carrier_contract_enabled() && !use_plain_fallback {
            // Callable carriers publish their own dynamic entry shell; do
            // not define a fallback lambda body just to obtain a fallback pointer.
            self.llvm_i8_ptr_type().const_null()
        } else if let Some(plain_entry) = self
            .module
            .get_function(&self.lir_source_closure_body_symbol(fn_ptr, span)?)
        {
            plain_entry.as_global_value().as_pointer_value()
        } else {
            self.ensure_lir_source_closure_callable_defined(span, fn_ptr)?
                .as_global_value()
                .as_pointer_value()
        };
        let fn_ptr = match target_fn_ptr {
            Some(ptr) => ptr,
            None => match carrier_key {
                Some(key) => self.callable_carrier_target_fn_ptr(key, fn_ptr, fallback_target)?,
                None => fallback_target,
            },
        };
        let fn_i8 = self
            .builder
            .build_pointer_cast(fn_ptr, i8_ptr_ty, "pass_mir_closure_fn_i8")?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_store_fn",
            &deferred_obj,
        )?;
        let fn_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 2, "pass_mir_closure_fn_gep")?;
        let _ = self.builder.build_store(fn_gep, fn_i8)?;
        let obj_ptr = self.reload_deferred_gc_ref_without_clearing(
            span,
            "pass_mir_closure_obj_return",
            &deferred_obj,
        )?;
        let obj_i8 =
            self.builder
                .build_pointer_cast(obj_ptr, gc_i8_ptr_ty, "pass_mir_closure_obj_i8")?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_funptr_invoke_call(
        &mut self,
        span: crate::span::Span,
        args: &[mir_source::CallArg],
        body: &mir_source::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some((receiver_arg, call_args)) = args.split_first() else {
            panic!(
                "codegen_mir_funptr_invoke_call: materialized MIR verifier accepted missing FunPtr receiver"
            );
        };
        if receiver_arg.name.is_some() {
            panic!(
                "codegen_mir_funptr_invoke_call: materialized MIR verifier accepted named FunPtr receiver"
            );
        }
        let fun_ty = self
            .mir_operand_funptr_function_type(body, mir_types, &receiver_arg.value)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_mir_funptr_invoke_call: materialized MIR verifier accepted non-FunPtr receiver type"
                )
            });
        self.codegen_mir_funptr_value_call(
            span,
            &receiver_arg.value,
            call_args,
            &fun_ty,
            (body, mir_types, slots),
        )
    }

    pub(in crate::llvm::codegen) fn codegen_lir_funptr_invoke_call(
        &mut self,
        span: crate::span::Span,
        args: &[LirCallArg],
        body: &LirExecutableBody,
        source_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some((receiver_arg, call_args)) = args.split_first() else {
            panic!("codegen_lir_funptr_invoke_call: LIR verifier accepted missing FunPtr receiver");
        };
        if receiver_arg.name.is_some() {
            panic!("codegen_lir_funptr_invoke_call: LIR verifier accepted named FunPtr receiver");
        }
        let fun_ty = self
            .lir_operand_funptr_function_type(body, source_types, &receiver_arg.value)
            .unwrap_or_else(|| {
                panic!(
                    "codegen_lir_funptr_invoke_call: LIR verifier accepted non-FunPtr receiver type"
                )
            });
        self.codegen_lir_funptr_value_call(
            span,
            &receiver_arg.value,
            call_args,
            &fun_ty,
            (source_types, slots),
        )
    }

    fn get_or_create_mir_closure_object_type_desc_global(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        if let Some((_, callable_types, callable)) = self.lir_source_callable(fn_ptr) {
            let mut params = Vec::with_capacity(callable.params.len().saturating_sub(1));
            let mut value_params = callable.params.iter().skip(1).peekable();
            let receiver = if value_params
                .peek()
                .is_some_and(|param| param.name == "this")
            {
                let Some(receiver) = value_params.next() else {
                    return Err(frontend_error(format!(
                        "closure allocation at {span:?} lost receiver parameter after peek"
                    )));
                };
                Some(self.mir_closure_signature_type_id(span, callable_types, receiver.ty)?)
            } else {
                None
            };
            for param in value_params {
                params.push(self.mir_closure_signature_type_id(span, callable_types, param.ty)?);
            }
            let return_ty =
                self.mir_closure_signature_type_id(span, callable_types, callable.return_ty)?;
            return self.get_or_create_closure_object_type_desc_for_signature(
                span, receiver, &params, return_ty,
            );
        }

        let Some(signature) = self.published_codegen_callable_signature(fn_ptr) else {
            return Err(frontend_error(format!(
                "closure allocation at {span:?} cannot find callable signature `{fn_ptr}`"
            )));
        };
        self.get_or_create_closure_object_type_desc_for_signature(
            span,
            None,
            &signature.param_tys,
            signature.return_ty,
        )
    }

    fn mir_closure_signature_type_id(
        &self,
        span: crate::span::Span,
        callable_types: &TypeStore,
        source_ty: TypeId,
    ) -> Result<TypeId, LlvmEmitError> {
        let Some(codegen_ty) = self.equivalent_codegen_type_id(callable_types, source_ty) else {
            return Err(frontend_error(format!(
                "closure allocation at {span:?} cannot map MIR signature type {} to codegen TypeStore",
                callable_types.display(source_ty)
            )));
        };
        Ok(codegen_ty)
    }
}
