//! Return / literal / string / interpolated-string lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn emit_return(
        &mut self,
        span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            // T1612: A function declared as returning Nothing never returns normally.
            // Emit `unreachable` instead of a return instruction.
            CgTy::Never => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: if self.function_cx.current_sret_return_ptr.is_some() {
                            "sret return value"
                        } else {
                            "return value"
                        },
                        at: span.into(),
                    });
                };
                if let Some(sret_ptr) = self.function_cx.current_sret_return_ptr
                    && matches!(
                        declared_return_ty,
                        CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)
                    )
                {
                    let _ = self.builder.build_store(sret_ptr, raw)?;
                    self.builder.build_return(None)?;
                } else {
                    self.builder.build_return(Some(&raw))?;
                }
                Ok(())
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_literal(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match lit {
            hir::LiteralKind::Unit => Ok(CgValue::unit()),
            hir::LiteralKind::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            hir::LiteralKind::Char(value) => Ok(CgValue::int(
                self.context.i32_type().const_int(*value as u64, false),
                IntTy {
                    bits: 32,
                    signed: false,
                },
            )),
            hir::LiteralKind::Int => {
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
                };
                let value = self.int_literal_bits_for_ty(span, int_ty)?;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            hir::LiteralKind::Float64(value) => Ok(CgValue::float(
                self.context.f64_type().const_float(*value),
                CgTy::Float64,
            )),
            hir::LiteralKind::Float32(value) => Ok(CgValue::float(
                self.context.f32_type().const_float(f64::from(*value)),
                CgTy::Float32,
            )),
            hir::LiteralKind::String => self.codegen_string_literal(span),
            hir::LiteralKind::SynthString(value) => {
                self.codegen_string_literal_from_text(span, value)
            }
            hir::LiteralKind::SynthInt(value) => {
                // Synthesized integer literal from compiler desugaring (T0110).
                let int_ty = IntTy {
                    bits: 64,
                    signed: true,
                };
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(*value as u64, false),
                    int_ty,
                ))
            }
        }
    }

    /// Emit LLVM IR for a string literal by parsing the current source text on demand.
    pub(in crate::llvm::codegen) fn codegen_string_literal(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let bytes = self.parse_current_string_literal_bytes(span)?;
        self.codegen_string_literal_from_bytes(span, &bytes)
    }

    pub(in crate::llvm::codegen) fn codegen_string_literal_from_text(
        &mut self,
        span: crate::span::Span,
        text: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_string_literal_from_bytes(span, text.as_bytes())
    }

    /// Emit LLVM IR for a string literal from already parsed bytes.
    pub(in crate::llvm::codegen) fn codegen_string_literal_from_bytes(
        &mut self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 分配一个 GC-managed `ScoopString` 对象：
        //    - LLVM 侧类型为 `ScoopString addrspace(1)*`
        //    - 分配通过 `scoop_alloc_typed(desc, sizeof(ScoopString))`（runtime 写入对象头 type_desc）
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = self.context.i64_type().const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "str_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_string_lit",
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
            .build_pointer_cast(raw_ptr, str_ptr_ty, "str_obj_ptr")?;

        // 2) 写入 `{ len, data }`。
        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "str_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "str_data_gep")?;

        let len = self.context.i64_type().const_int(bytes.len() as u64, false);
        let _ = self.builder.build_store(len_ptr, len)?;

        // 空串：保持 `data = NULL`（与 runtime 侧空串约定一致）。
        if bytes.is_empty() {
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let _ = self.builder.build_store(data_ptr, i8_ptr_ty.const_null())?;
        } else {
            // 把字节序列落到一个只读全局常量：`[N x i8] @__scoop_str_data_*`
            let data_gv = self.get_or_create_global_bytes(span, bytes);
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let data_i8_ptr = self.builder.build_pointer_cast(
                data_gv.as_pointer_value(),
                i8_ptr_ty,
                "str_data_ptr",
            )?;
            let _ = self.builder.build_store(data_ptr, data_i8_ptr)?;
        }

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn load_scoop_string_len_and_data(
        &mut self,
        str_obj_ptr: PointerValue<'ctx>,
    ) -> Result<(IntValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let scoop_str_ty = self.llvm_scoop_string_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_obj_ptr, 1, "str_len_gep_interp")?;
        let data_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_obj_ptr, 2, "str_data_gep_interp")?;

        let len = self
            .builder
            .build_load(i64_ty, len_ptr, "str_len_interp")?
            .into_int_value();
        let data = self
            .builder
            .build_load(i8_ptr_ty, data_ptr, "str_data_interp")?
            .into_pointer_value();

        Ok((len, data))
    }

    pub(in crate::llvm::codegen) fn codegen_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[hir::InterpolatedStringPart],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段的落点：把 f-string 分片后"拼接"为一段连续 UTF-8 字节序列，
        // 返回一个 GC-managed `ScoopString` 对象（addrspace(1)），其 `data` 指向 `malloc` 的 bytes buffer。
        //
        // 约束（与 TODO T0823 对齐）：
        // - 目前支持 `{Bool}` / `{Char}` / `{Int}` / `{String}` / `{Float}`；
        // - 先不支持 format spec / locale；
        // - 当前阶段不接入 type descriptor/release：`data` 的释放留给后续任务补齐（T1507/T1514）。
        // - 这是 compiler-synthesis formatting path，不是 public `String.*` helper surface；
        //   public scalar/string `toString` 已走 sysroot body / audited bridge。

        #[derive(Clone, Copy)]
        struct Segment<'ctx> {
            ptr: PointerValue<'ctx>,
            len: IntValue<'ctx>,
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let scoop_str_ty = self.llvm_scoop_string_type();

        // 1) 先做一遍：收集所有片段的 (ptr, len)，并计算总长度（运行期）。
        let mut segments: Vec<Segment<'ctx>> = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            match part {
                hir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.current_source_slice(*text_span)?;
                    let bytes = parse_f_string_text_bytes(raw, text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "invalid interpolated string text",
                            at: (*text_span).into(),
                        }
                    })?;

                    let gv = self.get_or_create_global_bytes(*text_span, &bytes);
                    let ptr = self.builder.build_pointer_cast(
                        gv.as_pointer_value(),
                        i8_ptr_ty,
                        "fstr_text_ptr",
                    )?;
                    let len = i64_ty.const_int(bytes.len() as u64, false);

                    segments.push(Segment { ptr, len });
                    total_len = self
                        .builder
                        .build_int_add(total_len, len, "fstr_total_len")?;
                }
                hir::InterpolatedStringPart::Expr { expr } => {
                    if self.expr_is_builtin_char(expr) {
                        let str_v = self.codegen_char_method_to_string(expr.span, expr)?;
                        let Some(raw) = str_v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation char expr value",
                                at: expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation char expr type",
                                at: expr.span.into(),
                            });
                        };

                        let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                        segments.push(Segment { ptr: data, len });
                        total_len = self
                            .builder
                            .build_int_add(total_len, len, "fstr_total_len")?;
                        continue;
                    }

                    let v = self.codegen_expr(expr)?;

                    match v.ty {
                        CgTy::String => {
                            let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                            let Some(raw) = coerced.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Bool => {
                            let Some(raw) = v.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::IntValue(bool_val) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let bool_as_i64 = self.builder.build_int_z_extend(
                                bool_val,
                                i64_ty,
                                "fstr_bool_zext",
                            )?;
                            let rt_bool = self.declare_runtime_bool_to_string();
                            let call = self.build_call_preserving_gc_local_roots(
                                expr.span,
                                rt_bool,
                                &[bool_as_i64.into()],
                                "rt_bool_to_string_for_fstr",
                            )?;
                            let ret = call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool return value",
                                    at: expr.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(str_obj_ptr) = ret else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool return type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Float64 | CgTy::Float32 => {
                            let str_v =
                                self.codegen_float_to_string_value(expr.span, expr.span, v)?;
                            let Some(raw) = str_v.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation float expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation float expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Int(from_ty) => {
                            if from_ty.bits > 64 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer width for string interpolation",
                                    at: expr.span.into(),
                                });
                            }

                            let (raw_int, _) =
                                v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer interpolation expr value",
                                    at: expr.span.into(),
                                })?;

                            // 先把整数提升/截断到 i64/u64，再调用 runtime 格式化到临时 buffer。
                            let to_ty = IntTy {
                                bits: 64,
                                signed: from_ty.signed,
                            };
                            let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

                            // i64 最长：`-9223372036854775808`（20 字符）；
                            // 这里给更宽松的 cap，避免后续扩展时踩坑。
                            let cap = i64_ty.const_int(64, false);
                            let buf =
                                self.builder
                                    .build_array_alloca(i8_ty, cap, "fstr_int_buf")?;

                            let fmt_name = if from_ty.signed {
                                "scoop_format_i64"
                            } else {
                                "scoop_format_u64"
                            };
                            let fmt_fun = self.declare_runtime_format_int(fmt_name);
                            let call_site = self.builder.build_call(
                                fmt_fun,
                                &[int64.into(), buf.into(), cap.into()],
                                "fstr_fmt_int",
                            )?;
                            let len = call_site
                                .try_as_basic_value()
                                .basic()
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation int length",
                                    at: expr.span.into(),
                                })?
                                .into_int_value();

                            segments.push(Segment { ptr: buf, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation expr type",
                                at: expr.span.into(),
                            });
                        }
                    }
                }
            }
        }

        // 2) 为拼接结果分配 heap buffer（malloc），并按顺序 memcpy 各段。
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            total_len,
            i64_ty.const_zero(),
            "fstr_total_is_zero",
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

        let malloc_bb = self.context.append_basic_block(func, "fstr_malloc");
        let done_bb = self.context.append_basic_block(func, "fstr_done");

        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        // --- malloc + memcpy ---
        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[total_len.into()], "fstr_malloc")?;
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
                    &format!("fstr_dst_{idx}"),
                )?
            };
            let _ = self.builder.build_memcpy(dst, 1, seg.ptr, 1, seg.len)?;
            cursor = self.builder.build_int_add(cursor, seg.len, "fstr_cursor")?;
        }

        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "fstr_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let buf_ptr = buf_phi.as_basic_value().into_pointer_value();

        // 3) 分配并初始化 `ScoopString` 对象（GC-managed）。
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "fstr_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_fstr",
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
            .build_pointer_cast(raw_ptr, str_ptr_ty, "fstr_obj_ptr")?;

        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "fstr_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "fstr_data_gep")?;

        let _ = self.builder.build_store(len_ptr, total_len)?;
        let _ = self.builder.build_store(data_ptr, buf_ptr)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }
}
