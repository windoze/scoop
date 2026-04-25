//! Scalar builtins and core intrinsic lowering helpers.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_print_like(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println named arg",
                at: span.into(),
            });
        };

        let rt_name = match fqn {
            "scoop.core.print" => "scoop_print",
            "scoop.core.println" => "scoop_println",
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown sysroot print/println callee",
                    at: callee_span.into(),
                });
            }
        };

        if self.expr_is_builtin_char(expr) {
            let char_ty = CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            });
            let recv = self.codegen_expr_in_expected_context(expr, Some(char_ty))?;
            let Some(raw) = recv.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println char arg value",
                    at: expr.span.into(),
                });
            };
            let BasicValueEnum::IntValue(codepoint) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println char arg type",
                    at: expr.span.into(),
                });
            };
            let str_ptr = self.codegen_char_to_string_value(expr.span, codepoint)?;
            let rt_fun = self.declare_runtime_print_like(rt_name);
            let _ = self.build_call_preserving_gc_local_roots(
                expr.span,
                rt_fun,
                &[str_ptr.into()],
                "rt_print_char_as_string",
            )?;
            return Ok(CgValue::unit());
        }

        // 说明：
        // - sysroot 中允许 `print/println` 以 overload set 的形式声明（例如 `String` 与 `Int`）；
        // - HIR 当前阶段不保留"已选定 overload"的信息，因此这里以实参 codegen 后的 `CgTy`
        //   来决定使用哪条 lowering 路径。
        //
        // 注意：这里**不要**强制把 expected type 设为 `String`：
        // - 对于 `when/if/block` 等表达式，expected 会导致其 arm/body 被强制 coercion 为 `String`，
        //   进而在 `Int -> String` 这类尚未实现的 coercion 上报错；
        // - `print/println` 的整数路径会在 codegen 后把 `Int` 提升/截断到 i64/u64 并调用 runtime 直接打印（见下方分支），
        //   因此应先让表达式产出其"自然值类型"，再在这里做转换。
        let v = self.codegen_expr(expr)?;
        match v.ty {
            CgTy::String => {
                let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                let Some(raw) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(str_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println arg type",
                        at: expr.span.into(),
                    });
                };

                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self.build_call_preserving_gc_local_roots(
                    expr.span,
                    rt_fun,
                    &[str_ptr.into()],
                    "rt_print",
                )?;
                Ok(CgValue::unit())
            }
            CgTy::Int(from_ty) => {
                if from_ty.bits > 64 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "integer width for print/println",
                        at: expr.span.into(),
                    });
                }

                let (raw_int, _) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println int arg value",
                    at: expr.span.into(),
                })?;

                // 统一把整数提升/截断到 i64/u64，并在 codegen 侧构造一个 GC-managed `String` 再打印。
                //
                // 说明：
                // - 早期阶段曾用 `scoop_print{,ln}_{i64,u64}` 绕开 `rewrite-statepoints-for-gc` 的崩溃；
                // - GC-FIX Phase C2c：print/println 的整数路径与字符串路径对齐，确保字符串构造在 statepoint 下稳定。
                let to_ty = IntTy {
                    bits: 64,
                    signed: from_ty.signed,
                };
                let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

                let str_ptr = self.codegen_int_to_string(expr.span, int64, to_ty.signed)?;
                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self.build_call_preserving_gc_local_roots(
                    expr.span,
                    rt_fun,
                    &[str_ptr.into()],
                    "rt_print_int_as_string",
                )?;
                Ok(CgValue::unit())
            }
            // T0114: Bool → "true"/"false" → print.
            CgTy::Bool => {
                let Some(raw) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println bool arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(bool_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println bool arg type",
                        at: expr.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 =
                    self.builder
                        .build_int_z_extend(bool_val, i64_ty, "bool_zext_print")?;
                let rt_bool = self.declare_runtime_bool_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    expr.span,
                    rt_bool,
                    &[bool_as_i64.into()],
                    "rt_bool_to_string_print",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString return value for print",
                        at: expr.span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString return type for print",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self.build_call_preserving_gc_local_roots(
                    expr.span,
                    rt_fun,
                    &[str_ptr.into()],
                    "rt_print_bool_as_string",
                )?;
                Ok(CgValue::unit())
            }
            CgTy::Float64 | CgTy::Float32 => {
                let str_v = self.codegen_float_to_string_value(expr.span, expr.span, v)?;
                let Some(raw) = str_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println float arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(str_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println float arg type",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self.build_call_preserving_gc_local_roots(
                    expr.span,
                    rt_fun,
                    &[str_ptr.into()],
                    "rt_print_float_as_string",
                )?;
                Ok(CgValue::unit())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println arg type",
                at: expr.span.into(),
            }),
        }
    }

    /// T0131/T0146c2：where-bound `ToString.toString` 拦截——内建类型短路分发。
    ///
    /// monomorphized `print<Bool>` body 调用 `value.toString()`，HIR 将其改写为
    /// `scoop.core.ToString.toString(value)`。此函数在 itable dispatch 前拦截：
    /// - 内建类型（Bool/Char/Int/String）→ 调用 runtime → `Some(CgValue)`
    /// - 其它类型（class/struct）→ `None`（fall-through 到 itable 或 fun_index）
    pub(in crate::llvm::codegen) fn try_codegen_tostring_iface_builtin(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if args.len() != 1 {
            return Ok(None);
        }
        let hir::CallArg::Positional(expr) = &args[0] else {
            return Ok(None);
        };
        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_to_string(span, expr).map(Some);
        }
        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Bool => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(iv) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool type",
                        at: expr.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 = self.builder.build_int_z_extend(iv, i64_ty, "bool_zext")?;
                let rt_fun = self.declare_runtime_bool_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_fun,
                    &[bool_as_i64.into()],
                    "rt_bool_to_string",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool ret type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                }))
            }
            CgTy::Int(_) => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(int_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int type",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_int_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_fun,
                    &[int_val.into()],
                    "rt_int_to_string",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int ret type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                }))
            }
            CgTy::Float64 | CgTy::Float32 => self
                .codegen_float_to_string_value(span, expr.span, recv)
                .map(Some),
            CgTy::String => Ok(Some(recv)),
            _ => Ok(None), // fall-through: let itable/fun_index handle user types
        }
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

    /// T0131/T0146c2：body-less extension function `toString()`（Char/Int/Bool/String）codegen 拦截。
    ///
    /// HIR lowering 将 `receiver.toString()` 改写为 top-level call `scoop.core.toString(receiver)`。
    /// 按 receiver CgTy 分发到对应 runtime 函数。
    pub(in crate::llvm::codegen) fn codegen_sysroot_to_string_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString ext named arg",
                at: callee_span.into(),
            });
        };

        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_to_string(span, expr);
        }

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Bool => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(iv) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool type",
                        at: expr.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 = self.builder.build_int_z_extend(iv, i64_ty, "bool_zext")?;
                let rt_fun = self.declare_runtime_bool_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_fun,
                    &[bool_as_i64.into()],
                    "rt_bool_to_string",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool ret type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Int(_) => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(int_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int type",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_int_to_string();
                let call = self.build_call_preserving_gc_local_roots(
                    span,
                    rt_fun,
                    &[int_val.into()],
                    "rt_int_to_string",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int ret type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Float64 | CgTy::Float32 => {
                self.codegen_float_to_string_value(span, expr.span, recv)
            }
            CgTy::String => {
                // String.toString() → return self.
                Ok(recv)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString ext unsupported CgTy",
                at: span.into(),
            }),
        }
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
        // 语义：`sizeOf(x)` 在当前阶段返回 `x` 的静态类型在目标 ABI 下的 store size（bytes）。
        //
        // 说明：
        // - 规范中的 `sizeOf<T>()` 是 comptime 反射 intrinsic（spec §6.4）；
        // - 当前阶段尚未实现 comptime 执行链路，因此该 intrinsic 先作为 codegen 内建：
        //   直接把结果 lowering 为编译期常量（不产生对 `scoop.core.sizeOf` 的函数调用）。
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() named arg",
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

    fn codegen_int_to_string(
        &mut self,
        span: crate::span::Span,
        int64: IntValue<'ctx>,
        signed: bool,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // 1) 先把整数格式化到栈上的临时 buffer（native addrspace(0)），得到实际字节长度。
        //
        // 说明：
        // - `scoop_format_{i64,u64}` 为 "caller 提供 buffer + cap" 形式；
        // - 这里的 `buf` 是纯 native bytes，不应被当作 GC-managed roots。
        let cap = i64_ty.const_int(64, false);
        let buf = self
            .builder
            .build_array_alloca(i8_ty, cap, "print_int_buf")?;

        let fmt_name = if signed {
            "scoop_format_i64"
        } else {
            "scoop_format_u64"
        };
        let fmt_fun = self.declare_runtime_format_int(fmt_name);
        let call_site = self.builder.build_call(
            fmt_fun,
            &[int64.into(), buf.into(), cap.into()],
            "print_fmt_int",
        )?;
        let len = call_site
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "print/println int format length",
                at: span.into(),
            })?
            .into_int_value();

        // 2) 分配 heap buffer（malloc）并拷贝 bytes；len==0 时保持 data=NULL。
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            len,
            i64_ty.const_zero(),
            "print_int_len_is_zero",
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

        let malloc_bb = self.context.append_basic_block(func, "print_int_malloc");
        let done_bb = self.context.append_basic_block(func, "print_int_done");

        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        // --- malloc + memcpy ---
        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[len.into()], "print_int_malloc")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(heap_buf) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return type",
                at: span.into(),
            });
        };
        let _ = self.builder.build_memcpy(heap_buf, 1, buf, 1, len)?;
        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "print_int_data_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = heap_buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let data_ptr = buf_phi.as_basic_value().into_pointer_value();

        // 3) 分配并初始化 `ScoopString` 对象（GC-managed）。
        //
        // 注意：必须在 codegen 侧通过 `scoop_alloc_typed` 触发 statepoint safepoint，
        // 不能在 runtime helper 内部隐式分配并触发 GC（否则 caller frame 无 stackmap roots）。
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "print_int_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_print_int_str",
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
        let str_ptr =
            self.builder
                .build_pointer_cast(raw_ptr, str_ptr_ty, "print_int_str_obj_ptr")?;

        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 1, "print_int_len_gep")?;
        let data_ptr_gep =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 2, "print_int_data_gep")?;

        let _ = self.builder.build_store(len_ptr, len)?;
        let _ = self.builder.build_store(data_ptr_gep, data_ptr)?;
        Ok(str_ptr)
    }

    fn store_size_bytes_of_basic_type(&self, ty: BasicTypeEnum<'ctx>) -> u64 {
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
