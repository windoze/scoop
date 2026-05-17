//! Scalar builtins and core intrinsic lowering helpers.

use super::super::*;
use crate::mir;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_panic(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot panic arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(message_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot panic named arg",
                at: span.into(),
            });
        };

        let message_v = self.codegen_expr_in_expected_context(message_expr, Some(CgTy::String))?;
        let message_v = self.coerce_value(message_expr.span, message_v, CgTy::String)?;
        let Some(raw) = message_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot panic message value",
                at: message_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(message_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot panic message type",
                at: message_expr.span.into(),
            });
        };

        let rt_fun = self.declare_runtime_panic();
        let _ = self.build_call_preserving_gc_local_roots(
            message_expr.span,
            rt_fun,
            &[message_ptr.into()],
            "rt_panic",
        )?;
        let _ = callee_span;
        Ok(CgValue::never())
    }

    pub(in crate::llvm::codegen) fn try_codegen_sysroot_print_string_like(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if args.len() != 1 {
            return Ok(None);
        }
        let hir::CallArg::Positional(expr) = &args[0] else {
            return Ok(None);
        };
        if self.resolve_expr_cg_ty(expr) != Some(CgTy::String) {
            return Ok(None);
        }

        let rt_name = match fqn {
            "scoop.core.print" => "scoop_print",
            "scoop.core.println" => "scoop_println",
            _ => return Ok(None),
        };
        let value = self.codegen_expr_in_expected_context(expr, Some(CgTy::String))?;
        let value = self.coerce_value(expr.span, value, CgTy::String)?;
        let Some(BasicValueEnum::PointerValue(str_ptr)) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println String arg value",
                at: expr.span.into(),
            });
        };
        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self.build_call_preserving_gc_local_roots(
            span,
            rt_fun,
            &[str_ptr.into()],
            "rt_print_string",
        )?;
        Ok(Some(CgValue::unit()))
    }

    /// T0131：`__scoop_print_string` / `__scoop_println_string` codegen 拦截。
    ///
    /// 泛型 `print<T>` / `println<T>` 的 monomorphized body 调用这些内部函数，
    /// 接受一个 String 参数并映射到 runtime 的 `scoop_print` / `scoop_println`。
    pub(in crate::llvm::codegen) fn codegen_sysroot_internal_print_string(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let rt_name = match fqn {
            "scoop.core.__scoop_print_string" => "scoop_print",
            "scoop.core.__scoop_println_string" => "scoop_println",
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "__scoop_print_string fqn",
                    at: span.into(),
                });
            }
        };

        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string named arg",
                at: span.into(),
            });
        };

        let val = self.codegen_expr(expr)?;
        let coerced = self.coerce_value(expr.span, val, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string arg value",
                at: expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(str_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string arg type",
                at: expr.span.into(),
            });
        };
        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self.build_call_preserving_gc_local_roots(
            expr.span,
            rt_fun,
            &[str_ptr.into()],
            "rt_internal_print",
        )?;
        Ok(CgValue::unit())
    }

    /// T0146c2：body-less extension function `toInt()` codegen 拦截。
    ///
    /// 当前主要用于 `Char.toInt()`：HIR lowering 会把它改写为 `scoop.core.toInt(receiver)`。
    pub(in crate::llvm::codegen) fn codegen_sysroot_to_int_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt ext named arg",
                at: callee_span.into(),
            });
        };

        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_to_int(expr);
        }

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_to_int_value(span, expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    /// T0146c2：body-less extension function `hash()` codegen 拦截。
    ///
    /// 当前主要用于 `Char.hash()`：HIR lowering 会把它改写为 `scoop.core.hash(receiver)`。
    /// 这里按 receiver 的内建类型分发到对应 hashing 路径。
    pub(in crate::llvm::codegen) fn codegen_sysroot_hash_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash ext named arg",
                at: callee_span.into(),
            });
        };

        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_hash(span, expr);
        }

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Int(_) => self.codegen_int_method_hash(span, expr),
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_hash_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_abs_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "abs ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "abs ext named arg",
                at: callee_span.into(),
            });
        };

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_abs_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "abs ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_is_nan_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isNaN ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isNaN ext named arg",
                at: callee_span.into(),
            });
        };

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_is_nan_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isNaN ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_is_infinite_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isInfinite ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isInfinite ext named arg",
                at: callee_span.into(),
            });
        };

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_is_infinite_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isInfinite ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_size_of(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 语义：`sizeOf(x)` 在 HIR-compatible path 返回静态类型的目标 ABI store size。
        let [hir::CallArg::Positional(expr)] = args else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() arity mismatch",
                at: span.into(),
            });
        };

        let arg_cg = self
            .cg_ty_of(expr.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() arg type",
                at: callee_span.into(),
            })?;
        let llvm_ty = self.llvm_basic_type_of(expr.span, arg_cg)?;
        let bytes = self.store_size_bytes_of_basic_type(llvm_ty);

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(bytes, false);
        Ok(CgValue::int(raw, value_word))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_align_of(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_ty = self.reflection_type_arg_for_current_call(span, "alignOf")?;
        let arg_cg = self
            .cg_ty_of(source_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "alignOf() arg type",
                at: span.into(),
            })?;
        let llvm_ty = self.llvm_basic_type_of(span, arg_cg)?;
        let align = self.abi_align_bytes_of_basic_type(llvm_ty);
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(u64::from(align), false);
        Ok(CgValue::int(raw, value_word))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_kind_of(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let source_ty = self.reflection_type_arg_for_current_call(span, "kindOf")?;
        let kind = self.array_elem_kind_for_type_id(source_ty);
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(kind, false);
        Ok(CgValue::int(raw, value_word))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_desc_of(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let source_ty = self.reflection_type_arg_for_current_call(span, "descOf")?;
        let raw = if self.array_elem_kind_for_type_id(source_ty) == 3 {
            let body_fqn = self
                .function_cx
                .current_callable_fqn
                .clone()
                .unwrap_or_else(|| "<descOf>".to_string());
            let metadata =
                mir::ValueTransportMetadata::plain(source_ty, mir::MirTransportKind::ArrayElement);
            let descriptor = self.get_or_create_value_composite_transport_descriptor_global(
                &body_fqn, span, self.types, &metadata,
            )?;
            let ptr = descriptor.as_pointer_value();
            let ptr_int_ty = self.llvm_ptr_sized_int_type(Some(ptr.get_type().get_address_space()));
            let raw = self
                .builder
                .build_ptr_to_int(ptr, ptr_int_ty, "desc_of_composite")?;
            self.cast_int(
                raw,
                IntTy {
                    bits: ptr_int_ty.get_bit_width(),
                    signed: false,
                },
                value_word,
            )?
        } else {
            self.int_type(value_word).const_int(0, false)
        };
        Ok(CgValue::int(raw, value_word))
    }

    fn reflection_type_arg_for_current_call(
        &self,
        span: crate::span::Span,
        name: &'static str,
    ) -> Result<TypeId, LlvmEmitError> {
        let binding = self.current_top_level_fun_call_binding(span)?.ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "reflection intrinsic call binding",
                at: span.into(),
            },
        )?;
        binding
            .type_args
            .first()
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: name,
                at: span.into(),
            })
    }

    fn array_elem_kind_for_type_id(&self, ty: TypeId) -> u64 {
        match self.types.kind(ty) {
            TypeKind::Ref(_) => 2,
            TypeKind::Value(ValueTypeKind::Unit)
            | TypeKind::Value(ValueTypeKind::Nothing)
            | TypeKind::Value(ValueTypeKind::Bool)
            | TypeKind::Value(ValueTypeKind::Char)
            | TypeKind::Value(ValueTypeKind::Float64)
            | TypeKind::Value(ValueTypeKind::Float32)
            | TypeKind::Value(ValueTypeKind::Int)
            | TypeKind::Value(ValueTypeKind::UInt)
            | TypeKind::Value(ValueTypeKind::IntN(_))
            | TypeKind::Value(ValueTypeKind::UIntN(_)) => 1,
            TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements.is_empty() => 1,
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                match self.nominal_kinds.get(&nominal.fqn) {
                    Some(
                        ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect,
                    ) => 2,
                    Some(ast::TypeKind::Enum)
                        if self.enum_layouts.get(&nominal.fqn).is_some_and(|layout| {
                            layout
                                .variants
                                .iter()
                                .all(|variant| variant.fields.is_empty())
                        }) =>
                    {
                        1
                    }
                    _ => 3,
                }
            }
            TypeKind::Value(ValueTypeKind::Tuple(_))
            | TypeKind::Value(ValueTypeKind::Option(_)) => 3,
            TypeKind::Param(_) | TypeKind::StarProjection(_) => 3,
        }
    }

    /// `Char` 在 LLVM 侧与 `Int` 同为 `CgTy::Int`，因此 builtin 分发需要额外看 HIR concrete type。
    pub(in crate::llvm::codegen) fn expr_is_builtin_char(&self, expr: &hir::Expr) -> bool {
        let ty = self.resolve_expr_concrete_type(expr).unwrap_or(expr.ty);
        matches!(self.types.kind(ty), TypeKind::Value(ValueTypeKind::Char))
    }

    /// Codegen for String byte-level substrate methods.
    pub(in crate::llvm::codegen) fn codegen_string_method(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        method_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // Evaluate receiver as String pointer.
        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::String))?;
        let coerced = self.coerce_value(receiver.span, recv, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "String method receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "String method receiver type",
                at: receiver.span.into(),
            });
        };
        let deferred_recv = self.defer_gc_sensitive_cg_value(
            receiver.span,
            &format!("string_method_{method_name}_recv"),
            CgValue {
                ty: CgTy::String,
                value: Some(recv_ptr.into()),
            },
        )?;

        match method_name {
            // T0120: String.byteLength() — 0 args → Int (inline LLVM IR: read ScoopString.len)
            "byteLength" => {
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.byteLength arity mismatch",
                        at: span.into(),
                    });
                }
                let scoop_str_ty = self.llvm_scoop_string_type();
                let len_ptr = self.builder.build_struct_gep(
                    scoop_str_ty,
                    recv_ptr,
                    1,
                    "str_byte_length_gep",
                )?;
                let len_val =
                    self.builder
                        .build_load(self.context.i64_type(), len_ptr, "str_byte_length")?;
                let BasicValueEnum::IntValue(iv) = len_val else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.byteLength load type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0120: String.getByte(index) — 1 Int arg → Int (inline LLVM IR: bounds-checked byte read)
            "getByte" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.getByte arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(idx_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.getByte named arg",
                        at: span.into(),
                    });
                };
                let idx = self.codegen_expr_in_expected_context(
                    idx_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let idx_val = idx.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.getByte index value",
                    at: span.into(),
                })?;
                let BasicValueEnum::IntValue(idx_int) = idx_val else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.getByte index type",
                        at: span.into(),
                    });
                };
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;

                let scoop_str_ty = self.llvm_scoop_string_type();
                let i64_ty = self.context.i64_type();
                let i8_ty = self.context.i8_type();

                // Load string length for bounds check
                let len_ptr =
                    self.builder
                        .build_struct_gep(scoop_str_ty, recv_ptr, 1, "get_byte_len_gep")?;
                let len_val = self
                    .builder
                    .build_load(i64_ty, len_ptr, "get_byte_len")?
                    .into_int_value();

                // Load data pointer
                let data_ptr_ptr = self.builder.build_struct_gep(
                    scoop_str_ty,
                    recv_ptr,
                    2,
                    "get_byte_data_gep",
                )?;
                let data_ptr = self
                    .builder
                    .build_load(self.llvm_i8_ptr_type(), data_ptr_ptr, "get_byte_data")?
                    .into_pointer_value();

                // Bounds check: index < 0 || index >= len → return 0
                let current_fn = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();
                let in_bounds_bb = self
                    .context
                    .append_basic_block(current_fn, "getByte_in_bounds");
                let out_of_bounds_bb = self
                    .context
                    .append_basic_block(current_fn, "getByte_out_of_bounds");
                let merge_bb = self.context.append_basic_block(current_fn, "getByte_merge");

                // Check index < 0
                let is_negative = self.builder.build_int_compare(
                    inkwell::IntPredicate::SLT,
                    idx_int,
                    i64_ty.const_zero(),
                    "idx_negative",
                )?;
                let not_negative_bb = self
                    .context
                    .append_basic_block(current_fn, "getByte_not_negative");
                self.builder.build_conditional_branch(
                    is_negative,
                    out_of_bounds_bb,
                    not_negative_bb,
                )?;

                // Check index >= len
                self.builder.position_at_end(not_negative_bb);
                let is_ge_len = self.builder.build_int_compare(
                    inkwell::IntPredicate::SGE,
                    idx_int,
                    len_val,
                    "idx_ge_len",
                )?;
                self.builder
                    .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

                // Out of bounds: return 0
                self.builder.position_at_end(out_of_bounds_bb);
                let zero_val = i64_ty.const_zero();
                self.builder.build_unconditional_branch(merge_bb)?;

                // In bounds: load byte and zero-extend to i64
                self.builder.position_at_end(in_bounds_bb);
                let byte_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        i8_ty,
                        data_ptr,
                        &[idx_int],
                        "get_byte_elem_gep",
                    )?
                };
                let byte_val = self
                    .builder
                    .build_load(i8_ty, byte_ptr, "get_byte_val")?
                    .into_int_value();
                let byte_i64 =
                    self.builder
                        .build_int_z_extend(byte_val, i64_ty, "get_byte_zext")?;
                self.builder.build_unconditional_branch(merge_bb)?;

                // Merge: phi node
                self.builder.position_at_end(merge_bb);
                let phi = self.builder.build_phi(i64_ty, "get_byte_result")?;
                phi.add_incoming(&[(&zero_val, out_of_bounds_bb), (&byte_i64, in_bounds_bb)]);
                let result = phi.as_basic_value().into_int_value();
                Ok(CgValue::int(
                    result,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown String method",
                at: span.into(),
            }),
        }
    }

    fn materialize_gc_sensitive_string_method_receiver(
        &mut self,
        at: crate::span::Span,
        method_name: &str,
        deferred: DeferredCgValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let reloaded = self.materialize_deferred_cg_value(
            at,
            &format!("string_method_{method_name}_recv_reload"),
            deferred,
        )?;
        let Some(raw) = reloaded.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "String method receiver reload value",
                at: at.into(),
            });
        };
        let BasicValueEnum::PointerValue(reloaded_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "String method receiver reload type",
                at: at.into(),
            });
        };
        Ok(reloaded_ptr)
    }

    /// T0146c1: `Char.toInt()` — zero-extend the runtime `i32` codepoint to `Int`.
    pub(in crate::llvm::codegen) fn codegen_char_method_to_int(
        &mut self,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let char_ty = CgTy::Int(IntTy {
            bits: 32,
            signed: false,
        });
        let int_ty = CgTy::Int(IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(char_ty))?;
        self.coerce_value(receiver.span, recv, int_ty)
    }

    /// T0146c2: `Char.hash()` —— 以 codepoint zero-extend 到 i64 后复用 `Int.hash()` mixing。
    pub(in crate::llvm::codegen) fn codegen_char_method_hash(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let char_ty = CgTy::Int(IntTy {
            bits: 32,
            signed: false,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(char_ty))?;
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.hash receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::IntValue(codepoint) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.hash receiver type",
                at: receiver.span.into(),
            });
        };
        let widened = self.builder.build_int_z_extend(
            codepoint,
            self.context.i64_type(),
            "char_hash_zext",
        )?;
        self.codegen_i64_hash_value(widened)
    }

    /// T1817: `Int.hash()` — SplitMix64-style bit-mixing (inline LLVM IR).
    ///
    /// Algorithm: x ^= x >> 30; x *= 0xbf58476d1ce4e5b9;
    ///            x ^= x >> 27; x *= 0x94d049bb133111eb;
    ///            x ^= x >> 31;
    pub(in crate::llvm::codegen) fn codegen_int_method_hash(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let int_cg_ty = CgTy::Int(IntTy {
            bits: 64,
            signed: true,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(int_cg_ty))?;
        let coerced = self.coerce_value(receiver.span, recv, int_cg_ty)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Int.hash receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::IntValue(x) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Int.hash receiver type",
                at: receiver.span.into(),
            });
        };

        self.codegen_i64_hash_value(x)
    }

    pub(in crate::llvm::codegen) fn codegen_i64_hash_value(
        &mut self,
        x: IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        // x ^= x >> 30
        let s1 =
            self.builder
                .build_right_shift(x, i64_ty.const_int(30, false), false, "hash_s1")?;
        let x1 = self.builder.build_xor(x, s1, "hash_x1")?;
        // x *= 0xbf58476d1ce4e5b9
        let c1 = i64_ty.const_int(0xbf58476d1ce4e5b9, false);
        let x2 = self.builder.build_int_mul(x1, c1, "hash_x2")?;
        // x ^= x >> 27
        let s2 =
            self.builder
                .build_right_shift(x2, i64_ty.const_int(27, false), false, "hash_s2")?;
        let x3 = self.builder.build_xor(x2, s2, "hash_x3")?;
        // x *= 0x94d049bb133111eb
        let c2 = i64_ty.const_int(0x94d049bb133111eb, false);
        let x4 = self.builder.build_int_mul(x3, c2, "hash_x4")?;
        // x ^= x >> 31
        let s3 =
            self.builder
                .build_right_shift(x4, i64_ty.const_int(31, false), false, "hash_s3")?;
        let x5 = self.builder.build_xor(x4, s3, "hash_x5")?;

        Ok(CgValue::int(
            x5,
            IntTy {
                bits: 64,
                signed: true,
            },
        ))
    }

    fn unpack_float_cg_value(
        &self,
        recv: CgValue<'ctx>,
        at: crate::span::Span,
        kind: &'static str,
    ) -> Result<(FloatValue<'ctx>, CgTy), LlvmEmitError> {
        if !matches!(recv.ty, CgTy::Float64 | CgTy::Float32) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        }
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        };
        let BasicValueEnum::FloatValue(float_val) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        };
        Ok((float_val, recv.ty))
    }

    pub(in crate::llvm::codegen) fn codegen_float_to_int_value(
        &mut self,
        span: crate::span::Span,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.toInt receiver type")?;
        let rt_fun = match float_ty {
            CgTy::Float64 => self.declare_runtime_float64_to_int(),
            CgTy::Float32 => self.declare_runtime_float32_to_int(),
            _ => unreachable!("filtered by unpack_float_cg_value"),
        };
        let call = self
            .builder
            .build_call(rt_fun, &[float_val.into()], "rt_float_to_int")?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toInt return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(int64_val) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toInt return type",
                at: span.into(),
            });
        };
        let runtime_int = CgValue::int(
            int64_val,
            IntTy {
                bits: 64,
                signed: true,
            },
        );
        self.coerce_value(
            span,
            runtime_int,
            CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            }),
        )
    }

    pub(in crate::llvm::codegen) fn codegen_float_hash_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.hash receiver type")?;
        let i64_bits = match float_ty {
            CgTy::Float64 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i64_type(),
                    "f64_hash_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.hash bits type",
                        at: receiver_span.into(),
                    });
                };
                bits
            }
            CgTy::Float32 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i32_type(),
                    "f32_hash_bits",
                )?;
                let BasicValueEnum::IntValue(bits32) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.hash bits type",
                        at: receiver_span.into(),
                    });
                };
                self.builder
                    .build_int_z_extend(bits32, self.context.i64_type(), "f32_hash_zext")?
            }
            _ => unreachable!("filtered by unpack_float_cg_value"),
        };
        self.codegen_i64_hash_value(i64_bits)
    }

    pub(in crate::llvm::codegen) fn codegen_float_abs_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.abs receiver type")?;
        match float_ty {
            CgTy::Float64 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i64_type(),
                    "f64_abs_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.abs bits type",
                        at: receiver_span.into(),
                    });
                };
                let masked = self.builder.build_and(
                    bits,
                    self.context
                        .i64_type()
                        .const_int(0x7fff_ffff_ffff_ffff, false),
                    "f64_abs_masked",
                )?;
                let raw =
                    self.builder
                        .build_bit_cast(masked, self.context.f64_type(), "f64_abs")?;
                let BasicValueEnum::FloatValue(abs_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.abs return type",
                        at: receiver_span.into(),
                    });
                };
                Ok(CgValue::float(abs_val, CgTy::Float64))
            }
            CgTy::Float32 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i32_type(),
                    "f32_abs_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.abs bits type",
                        at: receiver_span.into(),
                    });
                };
                let masked = self.builder.build_and(
                    bits,
                    self.context.i32_type().const_int(0x7fff_ffff, false),
                    "f32_abs_masked",
                )?;
                let raw =
                    self.builder
                        .build_bit_cast(masked, self.context.f32_type(), "f32_abs")?;
                let BasicValueEnum::FloatValue(abs_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.abs return type",
                        at: receiver_span.into(),
                    });
                };
                Ok(CgValue::float(abs_val, CgTy::Float32))
            }
            _ => unreachable!("filtered by unpack_float_cg_value"),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_float_is_nan_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, _) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.isNaN receiver type")?;
        let is_nan = self.builder.build_float_compare(
            FloatPredicate::UNO,
            float_val,
            float_val,
            "float_is_nan",
        )?;
        Ok(CgValue::bool(is_nan))
    }

    pub(in crate::llvm::codegen) fn codegen_float_is_infinite_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let abs_value = self.codegen_float_abs_value(receiver_span, recv)?;
        match abs_value.ty {
            CgTy::Float64 => {
                let (float_val, _) = self.unpack_float_cg_value(
                    abs_value,
                    receiver_span,
                    "Float.isInfinite receiver type",
                )?;
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i64_type(),
                    "f64_inf_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.isInfinite bits type",
                        at: receiver_span.into(),
                    });
                };
                let is_inf = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    bits,
                    self.context
                        .i64_type()
                        .const_int(0x7ff0_0000_0000_0000, false),
                    "f64_is_infinite",
                )?;
                Ok(CgValue::bool(is_inf))
            }
            CgTy::Float32 => {
                let (float_val, _) = self.unpack_float_cg_value(
                    abs_value,
                    receiver_span,
                    "Float.isInfinite receiver type",
                )?;
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i32_type(),
                    "f32_inf_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.isInfinite bits type",
                        at: receiver_span.into(),
                    });
                };
                let is_inf = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    bits,
                    self.context.i32_type().const_int(0x7f80_0000, false),
                    "f32_is_infinite",
                )?;
                Ok(CgValue::bool(is_inf))
            }
            _ => unreachable!("Float.abs preserves float CgTy"),
        }
    }

    pub(in crate::llvm::codegen) fn store_size_bytes_of_basic_type(
        &self,
        ty: BasicTypeEnum<'ctx>,
    ) -> u64 {
        match ty {
            BasicTypeEnum::ArrayType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::FloatType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::IntType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::PointerType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::StructType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::VectorType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::ScalableVectorType(t) => self.target_data.get_store_size(&t),
        }
    }

    pub(in crate::llvm::codegen) fn abi_align_bytes_of_basic_type(
        &self,
        ty: BasicTypeEnum<'ctx>,
    ) -> u32 {
        match ty {
            BasicTypeEnum::ArrayType(t) => self.target_data.get_abi_alignment(&t),
            BasicTypeEnum::FloatType(t) => self.target_data.get_abi_alignment(&t),
            BasicTypeEnum::IntType(t) => self.target_data.get_abi_alignment(&t),
            BasicTypeEnum::PointerType(t) => self.target_data.get_abi_alignment(&t),
            BasicTypeEnum::StructType(t) => self.target_data.get_abi_alignment(&t),
            BasicTypeEnum::VectorType(t) => self.target_data.get_abi_alignment(&t),
            BasicTypeEnum::ScalableVectorType(t) => self.target_data.get_abi_alignment(&t),
        }
    }
}
