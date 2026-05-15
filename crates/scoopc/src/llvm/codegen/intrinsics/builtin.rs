//! Scalar builtins and core intrinsic lowering helpers.

use super::super::*;

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
            CgTy::String => self.codegen_string_method(span, expr, "toInt", &[]),
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
            CgTy::String => self.codegen_string_method(span, expr, "hash", &[]),
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

    /// `Char` 在 LLVM 侧与 `Int` 同为 `CgTy::Int`，因此 builtin 分发需要额外看 HIR concrete type。
    pub(in crate::llvm::codegen) fn expr_is_builtin_char(&self, expr: &hir::Expr) -> bool {
        let ty = self.resolve_expr_concrete_type(expr).unwrap_or(expr.ty);
        matches!(self.types.kind(ty), TypeKind::Value(ValueTypeKind::Char))
    }

    pub(in crate::llvm::codegen) fn codegen_string_trim_indent(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent arity mismatch",
                at: span.into(),
            });
        }

        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::String))?;
        let coerced = self.coerce_value(receiver.span, recv, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent receiver type",
                at: receiver.span.into(),
            });
        };

        let rt_fun = self.declare_runtime_trim_indent();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_fun,
            &[recv_ptr.into()],
            "rt_trim_indent",
        )?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    /// T1811: codegen for String P0 methods (length/substring/startsWith/endsWith/indexOf/contains/split).
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
            "length" => {
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                // scoop_string_length(s) -> i64
                let rt_fun = self.declare_runtime_string_length();
                let call =
                    self.builder
                        .build_call(rt_fun, &[recv_ptr.into()], "rt_string_length")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.length return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.length return type",
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
            // T0122/T0143: substring/indexOf/contains/startsWith/endsWith/split 已迁移到 sysroot/string.scoop
            "toInt" => {
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                // scoop_string_to_int(s) -> i64
                let rt_fun = self.declare_runtime_string_to_int();
                let call =
                    self.builder
                        .build_call(rt_fun, &[recv_ptr.into()], "rt_string_to_int")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.toInt return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.toInt return type",
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
            "concat" => {
                // scoop_string_concat(a, b) -> ScoopString*
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(other_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat named arg",
                        at: span.into(),
                    });
                };
                let other =
                    self.codegen_expr_in_expected_context(other_expr, Some(CgTy::String))?;
                let other_coerced = self.coerce_value(other_expr.span, other, CgTy::String)?;
                let Some(other_raw) = other_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat arg value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(other_ptr) = other_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat arg type",
                        at: span.into(),
                    });
                };
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_concat();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_fun,
                    &[recv_ptr.into(), other_ptr.into()],
                    "rt_string_concat",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(result_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(result_ptr.into()),
                })
            }
            // T1817: String.hash() — FNV-1a via C runtime.
            "hash" => {
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_hash();
                let call = self
                    .builder
                    .build_call(rt_fun, &[recv_ptr.into()], "rt_string_hash")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.hash return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.hash return type",
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
            // T0122/T0143: trim/trimStart/trimEnd 已迁移到 sysroot/string.scoop
            // T0115: String.isEmpty() — 0 args → Bool (i64 0/1 → i1)
            "isEmpty" => {
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_is_empty();
                let call =
                    self.builder
                        .build_call(rt_fun, &[recv_ptr.into()], "rt_string_isEmpty")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.isEmpty return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.isEmpty return type",
                        at: span.into(),
                    });
                };
                let bool_val = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    iv,
                    self.context.i64_type().const_zero(),
                    "to_bool",
                )?;
                Ok(CgValue::bool(bool_val))
            }
            // T0115: String.replace(old, new) — 2 String args → String
            "replace" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(old_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(new_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace named arg",
                        at: span.into(),
                    });
                };
                let old_val =
                    self.codegen_expr_in_expected_context(old_expr, Some(CgTy::String))?;
                let old_coerced = self.coerce_value(old_expr.span, old_val, CgTy::String)?;
                let Some(old_raw) = old_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace old value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(old_ptr) = old_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace old type",
                        at: span.into(),
                    });
                };
                let new_val =
                    self.codegen_expr_in_expected_context(new_expr, Some(CgTy::String))?;
                let new_coerced = self.coerce_value(new_expr.span, new_val, CgTy::String)?;
                let Some(new_raw) = new_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace new value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(new_ptr) = new_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace new type",
                        at: span.into(),
                    });
                };
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_replace();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), old_ptr.into(), new_ptr.into()],
                    "rt_string_replace",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(out_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(out_ptr.into()),
                })
            }
            // T0115: String.charAt(index) — 1 Int arg → Int
            "charAt" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(idx_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt named arg",
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
                    kind: "String.charAt index value",
                    at: span.into(),
                })?;
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_char_at();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), idx_val.into()],
                    "rt_string_charAt",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt return type",
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
            // T0115: String.repeat(n) — 1 Int arg → String
            "repeat" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(n_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat named arg",
                        at: span.into(),
                    });
                };
                let n = self.codegen_expr_in_expected_context(
                    n_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let n_val = n.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.repeat n value",
                    at: span.into(),
                })?;
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_repeat();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), n_val.into()],
                    "rt_string_repeat",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(out_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(out_ptr.into()),
                })
            }
            // T0115: String.compareTo(other) — 1 String arg → Int
            "compareTo" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(other_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo named arg",
                        at: span.into(),
                    });
                };
                let other =
                    self.codegen_expr_in_expected_context(other_expr, Some(CgTy::String))?;
                let other_coerced = self.coerce_value(other_expr.span, other, CgTy::String)?;
                let Some(other_raw) = other_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo arg value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(other_ptr) = other_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo arg type",
                        at: span.into(),
                    });
                };
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_compare_to();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), other_ptr.into()],
                    "rt_string_compareTo",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo return type",
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
            // T0121: String.unsafeSliceBytes(byteOffset, byteLength) — @Unsafe, 2 Int args → String
            "unsafeSliceBytes" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(offset_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(len_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes named arg",
                        at: span.into(),
                    });
                };
                let offset_cg = self.codegen_expr_in_expected_context(
                    offset_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let len_cg = self.codegen_expr_in_expected_context(
                    len_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let offset_val = offset_cg.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.unsafeSliceBytes offset value",
                    at: span.into(),
                })?;
                let len_val = len_cg.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.unsafeSliceBytes len value",
                    at: span.into(),
                })?;
                let recv_ptr = self.materialize_gc_sensitive_string_method_receiver(
                    receiver.span,
                    method_name,
                    deferred_recv.clone(),
                )?;
                let rt_fun = self.declare_runtime_string_unsafe_slice_bytes();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), offset_val.into(), len_val.into()],
                    "rt_string_unsafe_slice_bytes",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(out_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(out_ptr.into()),
                })
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

    /// T0146c2: `Char.toString()` —— 调 runtime 把 Unicode scalar value 编码为 UTF-8 String。
    pub(in crate::llvm::codegen) fn codegen_char_method_to_string(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let char_ty = CgTy::Int(IntTy {
            bits: 32,
            signed: false,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(char_ty))?;
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::IntValue(codepoint) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString receiver type",
                at: receiver.span.into(),
            });
        };
        let str_ptr = self.codegen_char_to_string_value(span, codepoint)?;
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
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

    pub(in crate::llvm::codegen) fn codegen_char_to_string_value(
        &mut self,
        span: crate::span::Span,
        codepoint: IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let rt_fun = self.declare_runtime_char_to_string();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_fun,
            &[codepoint.into()],
            "rt_char_to_string",
        )?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString return type",
                at: span.into(),
            });
        };
        Ok(str_ptr)
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

    pub(in crate::llvm::codegen) fn codegen_float_to_string_value(
        &mut self,
        span: crate::span::Span,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.toString receiver type")?;
        let rt_fun = match float_ty {
            CgTy::Float64 => self.declare_runtime_float64_to_string(),
            CgTy::Float32 => self.declare_runtime_float32_to_string(),
            _ => unreachable!("filtered by unpack_float_cg_value"),
        };
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_fun,
            &[float_val.into()],
            "rt_float_to_string",
        )?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toString return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toString return type",
                at: span.into(),
            });
        };
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
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
}
