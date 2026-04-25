//! Sysroot API lowering that primarily forwards to runtime ABI calls.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_funptr_invoke(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // sysroot 里 `invoke` 是一个 extension fun：
        // - `FunPtr<...>.invoke(...)`
        // - HIR lowering 会把它降为：`scoop.unsafe.invoke(receiver, ...args)`
        //
        // 约束（v0）：
        // - receiver 必须是局部变量引用（需要借助 env.local.hir_ty 取回 `FunPtr<F>` 的精确签名）。
        let Some((receiver_arg, call_args)) = args.split_first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke arity mismatch",
                at: span.into(),
            });
        };

        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver (named arg)",
                at: span.into(),
            });
        };

        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver_expr.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver (non-local)",
                at: receiver_expr.span.into(),
            });
        };

        let local = self
            .function_cx
            .env
            .get(*id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown local funptr receiver",
                at: receiver_expr.span.into(),
            })?;

        let Some(hir_ty) = local.hir_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver type",
                at: receiver_expr.span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver kind",
                at: receiver_expr.span.into(),
            });
        };
        if nominal.fqn != "scoop.unsafe.FunPtr" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver kind",
                at: receiver_expr.span.into(),
            });
        }

        let sig_ty = nominal
            .args
            .first()
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke signature type",
                at: receiver_expr.span.into(),
            })?;
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke signature kind",
                at: receiver_expr.span.into(),
            });
        };

        let CgTy::Int(int_ty) = local.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver cg type",
                at: receiver_expr.span.into(),
            });
        };
        let local_ptr =
            self.local_ptr_for_use(receiver_expr.span, local, "load_funptr_receiver_slot")?;
        let loaded = self
            .builder
            .build_load(
                self.llvm_basic_type_of(receiver_expr.span, local.ty)?,
                local_ptr,
                "load_funptr",
            )?
            .into_int_value();

        self.codegen_funptr_value_call(
            loaded,
            int_ty,
            CallableValueCallSpec {
                span,
                callee_span,
                call_may_suspend: !fun_ty.effects.is_pure(),
                fun_ty,
                args: call_args,
            },
        )
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_funptr_to_uintptr(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptrToUIntPtr arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptrToUIntPtr named arg",
                at: span.into(),
            });
        };

        let v = self.codegen_expr(expr)?;
        let (raw, from_ty) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "funptrToUIntPtr arg type",
            at: expr.span.into(),
        })?;

        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let casted = self.cast_int(raw, from_ty, to_ty)?;
        Ok(CgValue::int(casted, to_ty))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_uintptr_to_funptr(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "uintPtrToFunPtr arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "uintPtrToFunPtr named arg",
                at: span.into(),
            });
        };

        let v = self.codegen_expr(expr)?;
        let (raw, from_ty) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "uintPtrToFunPtr arg type",
            at: expr.span.into(),
        })?;

        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let casted = self.cast_int(raw, from_ty, to_ty)?;
        Ok(CgValue::int(casted, to_ty))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_io_write_string(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        rt_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString named arg",
                at: span.into(),
            });
        };

        // v0：只支持 String 入参（与 sysroot 声明面一致）。
        let v = self.codegen_expr_in_expected_context(expr, Some(CgTy::String))?;
        let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString arg value",
                at: expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(str_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString arg type",
                at: expr.span.into(),
            });
        };

        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self
            .builder
            .build_call(rt_fun, &[str_ptr.into()], "rt_io_write_string")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_io_stdin_read_line_utf8(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_io_stdin_read_line_utf8();
        let call = self.builder.build_call(rt, &[], "stdin_read_line")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine return value",
                at: span.into(),
            })?;

        // 返回类型依赖 expected context（HIR v0 对大部分 call expr 仍用 `Any` 占位）。
        let Some(ret_cg_ty) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine missing expected type context",
                at: span.into(),
            });
        };
        if !matches!(ret_cg_ty, CgTy::Enum(_)) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine expected Option<String>",
                at: span.into(),
            });
        }

        Ok(CgValue {
            ty: ret_cg_ty,
            value: Some(raw),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_env_get(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(key_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull named arg",
                at: span.into(),
            });
        };

        let key_v = self.codegen_expr_in_expected_context(key_expr, Some(CgTy::String))?;
        let key_v = self.coerce_value(key_expr.span, key_v, CgTy::String)?;
        let Some(raw_key) = key_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull key value",
                at: key_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(key_ptr) = raw_key else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull key type",
                at: key_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_env_get();
        let call = self.builder.build_call(rt, &[key_ptr.into()], "env_get")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull return value",
                at: span.into(),
            })?;

        // 返回类型依赖 expected context（HIR v0 对大部分 call expr 仍用 `Any` 占位）。
        let Some(ret_cg_ty) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull missing expected type context",
                at: span.into(),
            });
        };
        if !matches!(ret_cg_ty, CgTy::Enum(_)) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull expected Option<String>",
                at: span.into(),
            });
        }
        Ok(CgValue {
            ty: ret_cg_ty,
            value: Some(raw),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_time_now_unix_millis(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "time.nowUnixMillis arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_time_now_unix_millis();
        let call = self.builder.build_call(rt, &[], "time_now_unix_millis")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "time.nowUnixMillis return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(raw_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "time.nowUnixMillis return type",
                at: span.into(),
            });
        };

        let from = IntTy {
            bits: 64,
            signed: true,
        };
        let to = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let casted = self.cast_int(raw_i64, from, to)?;
        Ok(CgValue::int(casted, to))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_fs_read_all_text_utf8(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_fs_read_all_text_utf8();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "fs_read_all_text")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText return value",
                at: span.into(),
            })?;

        // 返回类型依赖 expected context（HIR v0 对大部分 call expr 仍用 `Any` 占位）。
        let Some(ret_cg_ty) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText missing expected type context",
                at: span.into(),
            });
        };
        if !matches!(ret_cg_ty, CgTy::Enum(_)) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText expected Option<String>",
                at: span.into(),
            });
        }
        Ok(CgValue {
            ty: ret_cg_ty,
            value: Some(raw),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_fs_write_all_text_utf8(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText named arg (path)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(content_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText named arg (content)",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText path type",
                at: path_expr.span.into(),
            });
        };

        let content_v = self.codegen_expr_in_expected_context(content_expr, Some(CgTy::String))?;
        let content_v = self.coerce_value(content_expr.span, content_v, CgTy::String)?;
        let Some(raw_content) = content_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText content value",
                at: content_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(content_ptr) = raw_content else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText content type",
                at: content_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_fs_write_all_text_utf8();
        let call = self.builder.build_call(
            rt,
            &[path_ptr.into(), content_ptr.into()],
            "fs_write_all_text",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(raw_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText return type",
                at: span.into(),
            });
        };

        // runtime 返回 i64：向 host word size 的 `Int` 做一次 cast（与 time API 一致）。
        let from = IntTy {
            bits: 64,
            signed: true,
        };
        let to = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let casted = self.cast_int(raw_i64, from, to)?;
        Ok(CgValue::int(casted, to))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_process_exit(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.exit arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(code_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.exit named arg",
                at: span.into(),
            });
        };

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let code_v =
            self.codegen_expr_in_expected_context(code_expr, Some(CgTy::Int(value_word)))?;
        let code_v = self.coerce_value(code_expr.span, code_v, CgTy::Int(value_word))?;
        let (code_raw, code_from) = code_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "process.exit code value",
            at: code_expr.span.into(),
        })?;
        let code_to = IntTy {
            bits: 64,
            signed: true,
        };
        let code_i64 = self.cast_int(code_raw, code_from, code_to)?;

        let rt = self.declare_runtime_process_exit();
        let _ = self
            .builder
            .build_call(rt, &[code_i64.into()], "process_exit")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_process_args(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.args arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_process_args_array();
        let call = self.builder.build_call(rt, &[], "process_args")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "process.args return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(arr_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.args return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(arr_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_path_normalize(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_normalize();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "path_normalize")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_path_join(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(base_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join named arg (base)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(child_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join named arg (child)",
                at: span.into(),
            });
        };

        let base_v = self.codegen_expr_in_expected_context(base_expr, Some(CgTy::String))?;
        let base_v = self.coerce_value(base_expr.span, base_v, CgTy::String)?;
        let Some(raw_base) = base_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join base value",
                at: base_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(base_ptr) = raw_base else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join base type",
                at: base_expr.span.into(),
            });
        };

        let child_v = self.codegen_expr_in_expected_context(child_expr, Some(CgTy::String))?;
        let child_v = self.coerce_value(child_expr.span, child_v, CgTy::String)?;
        let Some(raw_child) = child_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join child value",
                at: child_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(child_ptr) = raw_child else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join child type",
                at: child_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_join();
        let call =
            self.builder
                .build_call(rt, &[base_ptr.into(), child_ptr.into()], "path_join")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_path_basename(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_basename();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "path_basename")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_path_dirname(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_dirname();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "path_dirname")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }
}
