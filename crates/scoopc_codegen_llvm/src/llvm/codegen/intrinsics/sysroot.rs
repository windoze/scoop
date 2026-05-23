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
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "missing receiver",
            );
        };

        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "named receiver argument",
            );
        };

        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver_expr.kind else {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "receiver is not a local FunPtr binding",
            );
        };

        let local = self.function_cx.env.get(*id).unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "unknown local receiver",
            )
        });

        let Some(hir_ty) = local.hir_ty else {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "receiver is missing HIR type",
            );
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty) else {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "receiver HIR type is not nominal FunPtr",
            );
        };
        if nominal.fqn != "scoop.unsafe.FunPtr" {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "receiver HIR type is not scoop.unsafe.FunPtr",
            );
        }

        let sig_ty = nominal.args.first().copied().unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "missing function signature type argument",
            )
        });
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty) else {
            self.panic_verified_intrinsic_contract(
                "FunPtr.invoke HIR lowering",
                "signature type argument is not a function type",
            );
        };

        let CgTy::Int(int_ty) = local.ty else {
            panic!(
                "codegen_sysroot_funptr_invoke: TypeStore equivalence verifier accepted FunPtr receiver with non-int codegen type"
            );
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
            FunPtrCallSpec {
                span,
                callee_span,
                fun_ty,
                args: call_args,
            },
        )
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_funptr_to_uintptr(
        &mut self,
        _span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "funptrToUIntPtr HIR lowering");

        let v = self.codegen_expr(expr)?;
        let (raw, from_ty) = v.as_int().unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "funptrToUIntPtr HIR lowering",
                "argument did not lower to integer payload",
            )
        });

        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let casted = self.cast_int(raw, from_ty, to_ty)?;
        Ok(CgValue::int(casted, to_ty))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_uintptr_to_funptr(
        &mut self,
        _span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "uintPtrToFunPtr HIR lowering");

        let v = self.codegen_expr(expr)?;
        let (raw, from_ty) = v.as_int().unwrap_or_else(|| {
            self.panic_verified_intrinsic_contract(
                "uintPtrToFunPtr HIR lowering",
                "argument did not lower to integer payload",
            )
        });

        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let casted = self.cast_int(raw, from_ty, to_ty)?;
        Ok(CgValue::int(casted, to_ty))
    }
}
