//! Threading intrinsics lowering.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_spawn(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let block_expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "codegen_sysroot_thread_spawn");

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                // 说明：
                // - `thread.spawn` 的参数类型在 sysroot 中固定为 `() -> Unit / Pure!`；
                // - 与 `sync.Once.run` 一致：为了在 early stage 稳定 codegen，这里从 `TypeStore` 中
                //   查找一个"无参、返回 Unit、Pure"的函数类型作为 expected context。
                let expected_fun_ty = self.lookup_pure_unit_closure_type().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "codegen_sysroot_thread_spawn",
                        "missing Pure Unit closure type",
                    )
                });
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let block_obj_i8 = self.expect_cg_pointer(block_v, "codegen_sysroot_thread_spawn block");

        // 抽取 closure object：`{ header, env_ptr, fn_ptr }`，把 env 与 typed fn 指针传给 runtime。
        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "thread_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "thread_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 2, "thread_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "thread_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "thread_fn_raw")?
            .into_pointer_value();

        let start_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let start_fn_ptr =
            self.builder
                .build_pointer_cast(fn_ptr_raw, start_fn_ptr_ty, "thread_fn_typed")?;

        let rt = self.declare_runtime_thread_spawn();
        let call = self.build_call_preserving_gc_local_roots(
            span,
            rt,
            &[env_ptr.into(), start_fn_ptr.into()],
            "thread_spawn",
        )?;
        let raw = self.expect_basic_value(call, "codegen_sysroot_thread_spawn return");
        let thread_ptr = self.expect_pointer_value(raw, "codegen_sysroot_thread_spawn return");

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(thread_ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_join(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let recv_expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "codegen_sysroot_thread_join");

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let recv_ptr = self.expect_cg_pointer(recv_v, "codegen_sysroot_thread_join receiver");

        let rt = self.declare_runtime_thread_join();
        let _ =
            self.build_call_preserving_gc_local_roots(span, rt, &[recv_ptr.into()], "thread_join")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_sleep_millis(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let ms_expr = self.expect_hir_positional_intrinsic_arg(
            args,
            1,
            0,
            "codegen_sysroot_thread_sleep_millis",
        );

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let ms_v = self.codegen_expr_in_expected_context(ms_expr, Some(CgTy::Int(value_word)))?;
        let ms_v = self.coerce_value(ms_expr.span, ms_v, CgTy::Int(value_word))?;
        let (ms_raw, ms_from) = self.expect_cg_int(ms_v, "codegen_sysroot_thread_sleep_millis ms");

        let ms_to = IntTy {
            bits: 64,
            signed: true,
        };
        let ms_i64 = self.cast_int(ms_raw, ms_from, ms_to)?;

        let rt = self.declare_runtime_thread_sleep_millis();
        let _ = self.build_call_preserving_gc_local_roots(
            span,
            rt,
            &[ms_i64.into()],
            "thread_sleep_millis",
        )?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_yield(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        assert!(
            args.is_empty(),
            "typecheck must reject thread.yield arguments before LLVM codegen"
        );

        let rt = self.declare_runtime_thread_yield();
        let _ = self.build_call_preserving_gc_local_roots(span, rt, &[], "thread_yield")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_thread_current_id(
        &mut self,
        _span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.expect_hir_intrinsic_arity(args, 0, "codegen_sysroot_thread_current_id");

        let rt = self.declare_runtime_thread_current_id();
        let call = self.builder.build_call(rt, &[], "thread_current_id")?;
        let raw = self.expect_basic_value(call, "codegen_sysroot_thread_current_id return");
        let raw_i64 = self.expect_int_value(raw, "codegen_sysroot_thread_current_id return");

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
}
