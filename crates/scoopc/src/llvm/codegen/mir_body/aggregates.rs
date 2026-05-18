//! MIR tuple / struct / closure-impl make + size-of lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_make_tuple(
        &mut self,
        span: crate::span::Span,
        _body: &crate::mir::Body,
        mir_types: &TypeStore,
        elements: &[crate::mir::Operand],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Tuple(tuple_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple target type",
                at: span.into(),
            });
        };
        let (element_tys, use_primary_types) = {
            let tuple_types = self
                .codegen_type_store_for_type_id(tuple_ty)
                .unwrap_or(mir_types);
            let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = tuple_types.kind(tuple_ty)
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple type",
                    at: span.into(),
                });
            };
            (element_tys.clone(), std::ptr::eq(tuple_types, self.types))
        };
        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut deferred_elements: Vec<(usize, crate::span::Span, DeferredCgValue<'ctx>)> =
            Vec::with_capacity(elements.len());

        for (idx, (operand, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = if use_primary_types {
                self.cg_ty_of(*elem_ty)
            } else {
                self.cg_ty_of_mir_type(mir_types, *elem_ty)
            }
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple element type",
                at: span.into(),
            })?;
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
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR tuple element value",
                        at: elem_span.into(),
                    })?,
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

    pub(in crate::llvm::codegen) fn codegen_mir_size_of(
        &mut self,
        span: crate::span::Span,
        mir_types: &TypeStore,
        value_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let arg_cg = self.cg_ty_of_mir_type(mir_types, value_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR sizeOf arg type",
                at: span.into(),
            },
        )?;
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
        let arg_cg = self.cg_ty_of_mir_type(mir_types, value_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR alignOf arg type",
                at: span.into(),
            },
        )?;
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
        let arg_cg = self.cg_ty_of_mir_type(mir_types, value_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR kindOf arg type",
                at: span.into(),
            },
        )?;
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
        let arg_cg = self.cg_ty_of_mir_type(mir_types, value_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR descOf arg type",
                at: span.into(),
            },
        )?;
        if !matches!(arg_cg, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) {
            return Ok(CgValue::int(
                self.int_type(value_word).const_zero(),
                value_word,
            ));
        }

        let body_fqn = self
            .function_cx
            .current_callable_fqn
            .clone()
            .unwrap_or_else(|| "<mir-descOf>".to_string());
        let metadata = crate::mir::ValueTransportMetadata::plain(
            value_ty,
            crate::mir::MirTransportKind::ArrayElement,
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
        fields: &[crate::mir::StructLitField],
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CgTy::Struct(struct_ty) = target_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR struct literal target type",
                at: span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR struct literal type",
                at: span.into(),
            });
        };
        let layout_key = self.nominal_layout_key(nominal);
        let layout =
            self.struct_layouts
                .get(&layout_key)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR struct literal layout",
                    at: span.into(),
                })?;
        if layout.fields.len() != fields.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR struct literal field count",
                at: span.into(),
            });
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
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR struct literal duplicate field",
                    at: init.span.into(),
                });
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
                _ => materialized
                    .value
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "pass MIR struct literal field value",
                        at: field_span.into(),
                    })?,
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

    pub(in crate::llvm::codegen) fn codegen_mir_tuple_get(
        &mut self,
        span: crate::span::Span,
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        tuple: &crate::mir::Operand,
        index: usize,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let tuple_ty =
            self.mir_operand_type_id(body, tuple)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR tuple operand type",
                    at: span.into(),
                })?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = mir_types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand type",
                at: span.into(),
            });
        };
        let elem_ty = *elements
            .get(index)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple index",
                at: span.into(),
            })?;
        let elem_cg = self.cg_ty_of_mir_type(mir_types, elem_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple element type",
                at: span.into(),
            },
        )?;
        let tuple_cg = self.mir_operand_cg_ty(body, mir_types, tuple).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand cg type",
                at: span.into(),
            },
        )?;
        let value = self.codegen_mir_operand_expected(span, tuple, slots, Some(tuple_cg))?;
        let tuple_v = value
            .value
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR tuple operand value",
                at: span.into(),
            })?
            .into_struct_value();
        self.extract_mir_tuple_element_value(span, tuple_v, index, elem_cg)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::llvm::codegen) fn codegen_mir_make_closure(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_contract: &crate::mir::ClosureEnvTransportMetadata,
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
    pub(in crate::llvm::codegen) fn codegen_mir_make_closure_with_target_fn_ptr(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_contract: &crate::mir::ClosureEnvTransportMetadata,
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
    pub(in crate::llvm::codegen) fn codegen_mir_make_closure_impl(
        &mut self,
        span: crate::span::Span,
        env: &crate::mir::Operand,
        fn_ptr: &str,
        env_contract: &crate::mir::ClosureEnvTransportMetadata,
        mir_types: &TypeStore,
        env_cg: CgTy,
        target_cg: CgTy,
        slots: &[MirLocalSlot<'ctx>],
        target_fn_ptr: Option<PointerValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if target_cg != CgTy::Ref {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR closure target type",
                at: span.into(),
            });
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
        let closure_desc = self.get_or_create_closure_object_type_desc_global(span)?;
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
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
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
            let closure_key = self.stable_closure_key_for_materialized_callable(fn_ptr, span)?;
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
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "scoop_alloc_typed return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return type",
                    at: span.into(),
                });
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
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "pass MIR closure env value",
                    at: span.into(),
                })?
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

        let use_plain_fallback = self
            .plain_callable_carrier_fallback_allowed(CallableCarrierKind::ClosureObject, fn_ptr);
        let fallback_target = if target_fn_ptr.is_some() {
            self.llvm_i8_ptr_type().const_null()
        } else if self.callable_carrier_contract_enabled() && !use_plain_fallback {
            // Callable carriers publish their own dynamic entry shell; do
            // not define a fallback lambda body just to obtain a fallback pointer.
            self.llvm_i8_ptr_type().const_null()
        } else if let Some(plain_entry) = self
            .module
            .get_function(&self.materialized_mir_closure_body_symbol(fn_ptr, span)?)
        {
            plain_entry.as_global_value().as_pointer_value()
        } else {
            self.ensure_materialized_mir_closure_callable_defined(span, fn_ptr)?
                .as_global_value()
                .as_pointer_value()
        };
        let fn_ptr = match target_fn_ptr {
            Some(ptr) => ptr,
            None => self.callable_carrier_target_fn_ptr(
                CallableCarrierKind::ClosureObject,
                fn_ptr,
                fallback_target,
            )?,
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
        args: &[crate::mir::CallArg],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some((receiver_arg, call_args)) = args.split_first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr invoke arity mismatch",
                at: span.into(),
            });
        };
        if receiver_arg.name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr invoke receiver binding",
                at: receiver_arg.span.into(),
            });
        }
        let fun_ty = self
            .mir_operand_funptr_function_type(body, mir_types, &receiver_arg.value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "pass MIR FunPtr invoke receiver type",
                at: receiver_arg.span.into(),
            })?;
        self.codegen_mir_funptr_value_call(
            span,
            &receiver_arg.value,
            call_args,
            &fun_ty,
            (body, mir_types, slots),
        )
    }
}
