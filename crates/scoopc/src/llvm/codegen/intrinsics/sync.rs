//! Synchronization intrinsics lowering.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_once_run(
        &mut self,
        _span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // `fun Once.run(block: () -> Unit): Unit`：`args = [once, block]`
        let once_expr =
            self.expect_hir_positional_intrinsic_arg(args, 2, 0, "codegen_sysroot_sync_once_run");
        let block_expr =
            self.expect_hir_positional_intrinsic_arg(args, 2, 1, "codegen_sysroot_sync_once_run");

        let once_v = self.codegen_expr_in_expected_context(once_expr, Some(CgTy::Ref))?;
        let once_v = self.coerce_value(once_expr.span, once_v, CgTy::Ref)?;
        let once_ptr = self.expect_cg_pointer(once_v, "codegen_sysroot_sync_once_run receiver");
        let deferred_once =
            self.defer_gc_ref_pointer(once_expr.span, "sync_once_run_receiver", once_ptr)?;

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                // 说明：
                // - `Once.run` 的参数类型在 sysroot 中固定为 `() -> Unit`；
                // - 但 early stage 的 `fun_index` 只包含"本编译单元内有 body 的函数"，不含 sysroot 声明；
                // - 同时 HIR v0 对 closure expr 的 `ty` 也不总是可用作 expected type（需要 MIR/CFG 才能更稳）。
                //
                // 因此这里从 `TypeStore` 中查找一个"无参、返回 Unit、Pure"的函数类型作为 expected context。
                let expected_fun_ty = self.lookup_pure_unit_closure_type().unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "codegen_sysroot_sync_once_run",
                        "missing Pure Unit closure type",
                    )
                });
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let block_obj_i8 = self.expect_cg_pointer(block_v, "codegen_sysroot_sync_once_run block");

        // 抽取 closure object：`{ header, env_ptr, fn_ptr }`，把 env 与 typed fn 指针传给 runtime。
        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "once_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 1, "once_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 2, "once_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "once_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "once_fn_raw")?
            .into_pointer_value();

        let init_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let init_fn_ptr =
            self.builder
                .build_pointer_cast(fn_ptr_raw, init_fn_ptr_ty, "once_fn_typed")?;

        let once_ptr = self.reload_deferred_gc_ref_without_clearing(
            once_expr.span,
            "sync_once_run_receiver_reload",
            &deferred_once,
        )?;

        let rt = self.declare_runtime_sync_once_run();
        let _ = self.builder.build_call(
            rt,
            &[once_ptr.into(), env_ptr.into(), init_fn_ptr.into()],
            "sync_once_run",
        )?;
        self.clear_deferred_cg_value_root_homes(
            once_expr.span,
            "sync_once_run_receiver_drop",
            &deferred_once,
        )?;
        Ok(CgValue::unit())
    }
}
