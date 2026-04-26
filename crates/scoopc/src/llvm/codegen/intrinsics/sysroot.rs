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
}
