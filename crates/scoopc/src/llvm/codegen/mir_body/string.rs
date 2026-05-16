//! MIR interpolated-string lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_mir_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[crate::mir::InterpolatedStringPart],
        body: &crate::mir::Body,
        mir_types: &TypeStore,
        slots: &[MirLocalSlot<'ctx>],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let scoop_str_ty = self.llvm_scoop_string_type();
        let mut segments = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            let segment = match part {
                crate::mir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.current_source_slice(*text_span)?;
                    let bytes = parse_f_string_text_bytes(raw, text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "invalid MIR interpolated string text",
                            at: (*text_span).into(),
                        }
                    })?;
                    let gv = self.get_or_create_global_bytes(*text_span, &bytes);
                    let ptr = self.builder.build_pointer_cast(
                        gv.as_pointer_value(),
                        i8_ptr_ty,
                        "mir_fstr_text_ptr",
                    )?;
                    MirInterpolatedSegment {
                        ptr,
                        len: i64_ty.const_int(bytes.len() as u64, false),
                    }
                }
                crate::mir::InterpolatedStringPart::Expr {
                    span: expr_span,
                    value,
                    ty,
                } => {
                    let source_ty = self.mir_operand_type_id(body, value).unwrap_or(*ty);
                    let value_cg = match value {
                        crate::mir::Operand::Local(local) => {
                            self.mir_local_slot(*expr_span, slots, *local)?.cg_ty
                        }
                        crate::mir::Operand::Const(_) => self
                            .cg_ty_of_mir_type(mir_types, source_ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "MIR interpolated string expr type",
                                at: (*expr_span).into(),
                            })?,
                    };
                    let v = self.codegen_mir_operand_expected(
                        *expr_span,
                        value,
                        slots,
                        Some(value_cg),
                    )?;
                    let v = self.coerce_value(*expr_span, v, value_cg)?;
                    self.codegen_mir_interpolated_expr_segment(*expr_span, source_ty, v, mir_types)?
                }
            };
            total_len = self
                .builder
                .build_int_add(total_len, segment.len, "mir_fstr_total_len")?;
            segments.push(segment);
        }

        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            total_len,
            i64_ty.const_zero(),
            "mir_fstr_total_is_zero",
        )?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let malloc_bb = self.context.append_basic_block(func, "mir_fstr_malloc");
        let done_bb = self.context.append_basic_block(func, "mir_fstr_done");
        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[total_len.into()], "mir_fstr_malloc")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(buf) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return type",
                at: span.into(),
            });
        };

        let mut cursor = i64_ty.const_zero();
        for (idx, seg) in segments.iter().enumerate() {
            let dst = unsafe {
                self.builder.build_in_bounds_gep(
                    i8_ty,
                    buf,
                    &[cursor],
                    &format!("mir_fstr_dst_{idx}"),
                )?
            };
            let _ = self.builder.build_memcpy(dst, 1, seg.ptr, 1, seg.len)?;
            cursor = self
                .builder
                .build_int_add(cursor, seg.len, "mir_fstr_cursor")?;
        }
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "mir_fstr_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let buf_ptr = buf_phi.as_basic_value().into_pointer_value();

        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);
        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "mir_fstr_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_mir_fstr",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "mir_fstr_obj_ptr")?;
        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 1, "mir_fstr_len_gep")?;
        let data_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 2, "mir_fstr_data_gep")?;
        let _ = self.builder.build_store(len_ptr, total_len)?;
        let _ = self.builder.build_store(data_ptr, buf_ptr)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_mir_unresolved_name_with_source_ty(
        &mut self,
        span: crate::span::Span,
        name: &str,
        source_types: &TypeStore,
        source_ty: TypeId,
        target_cg: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_cg = self.cg_ty_of_mir_type(source_types, source_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "MIR unresolved name source type",
                at: span.into(),
            },
        )?;
        let value = self.codegen_unresolved_ident(span, name, Some(source_cg))?;
        self.coerce_value(span, value, target_cg)
    }

    pub(in crate::llvm::codegen) fn codegen_mir_interpolated_expr_segment(
        &mut self,
        span: crate::span::Span,
        source_ty: TypeId,
        v: CgValue<'ctx>,
        mir_types: &TypeStore,
    ) -> Result<MirInterpolatedSegment<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        match v.ty {
            CgTy::String => {
                let coerced = self.coerce_value(span, v, CgTy::String)?;
                let Some(BasicValueEnum::PointerValue(str_obj_ptr)) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation expr value",
                        at: span.into(),
                    });
                };
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Bool => {
                let Some(BasicValueEnum::IntValue(bool_val)) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation bool expr value",
                        at: span.into(),
                    });
                };
                let bool_as_i64 =
                    self.builder
                        .build_int_z_extend(bool_val, i64_ty, "mir_fstr_bool_zext")?;
                let rt_bool = self.declare_runtime_bool_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_bool,
                    &[bool_as_i64.into()],
                    "rt_bool_to_string_for_mir_fstr",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation bool return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation bool return type",
                        at: span.into(),
                    });
                };
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Float64 | CgTy::Float32 => {
                let str_v = self.codegen_float_to_string_value(span, span, v)?;
                let Some(BasicValueEnum::PointerValue(str_obj_ptr)) = str_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation float return type",
                        at: span.into(),
                    });
                };
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Int(from_ty)
                if matches!(
                    mir_types.kind(source_ty),
                    TypeKind::Value(ValueTypeKind::Char)
                ) =>
            {
                let Some(BasicValueEnum::IntValue(codepoint)) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation char expr value",
                        at: span.into(),
                    });
                };
                let codepoint = self.cast_int(
                    codepoint,
                    from_ty,
                    IntTy {
                        bits: 32,
                        signed: false,
                    },
                )?;
                let str_obj_ptr = self.codegen_char_to_string_value(span, codepoint)?;
                let (len, ptr) = self.load_scoop_string_len_and_data(str_obj_ptr)?;
                Ok(MirInterpolatedSegment { ptr, len })
            }
            CgTy::Int(from_ty) => {
                if from_ty.bits > 64 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "integer width for MIR string interpolation",
                        at: span.into(),
                    });
                }
                let (raw_int, _) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "MIR integer interpolation expr value",
                    at: span.into(),
                })?;
                let to_ty = IntTy {
                    bits: 64,
                    signed: from_ty.signed,
                };
                let int64 = self.cast_int(raw_int, from_ty, to_ty)?;
                let cap = i64_ty.const_int(64, false);
                let buf = self.builder.build_array_alloca(
                    self.context.i8_type(),
                    cap,
                    "mir_fstr_int_buf",
                )?;
                let fmt_name = if from_ty.signed {
                    "scoop_format_i64"
                } else {
                    "scoop_format_u64"
                };
                let fmt_fun = self.declare_runtime_format_int(fmt_name);
                let call_site = self.builder.build_call(
                    fmt_fun,
                    &[int64.into(), buf.into(), cap.into()],
                    "mir_fstr_fmt_int",
                )?;
                let len = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "MIR string interpolation int length",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(MirInterpolatedSegment { ptr: buf, len })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "MIR string interpolation expr type",
                at: span.into(),
            }),
        }
    }
}
