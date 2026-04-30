//! Synchronization intrinsics lowering.

use super::super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_mutex_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.mutexCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_sync_mutex_create();
        let call = self.builder.build_call(rt, &[], "sync_mutex_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.mutexCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.mutexCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_mutex_lock(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_mutex_lock();
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_mutex_lock")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_mutex_unlock(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_mutex_unlock();
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_mutex_unlock")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_condvar_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.condVarCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_sync_condvar_create();
        let call = self.builder.build_call(rt, &[], "sync_condvar_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.condVarCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.condVarCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_condvar_wait(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(cv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(mutex_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait named arg (mutex)",
                at: span.into(),
            });
        };

        let cv_v = self.codegen_expr_in_expected_context(cv_expr, Some(CgTy::Ref))?;
        let cv_v = self.coerce_value(cv_expr.span, cv_v, CgTy::Ref)?;
        let Some(cv_raw) = cv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait receiver value",
                at: cv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cv_ptr) = cv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait receiver type",
                at: cv_expr.span.into(),
            });
        };

        let m_v = self.codegen_expr_in_expected_context(mutex_expr, Some(CgTy::Ref))?;
        let m_v = self.coerce_value(mutex_expr.span, m_v, CgTy::Ref)?;
        let Some(m_raw) = m_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait mutex value",
                at: mutex_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(m_ptr) = m_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait mutex type",
                at: mutex_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_condvar_wait();
        let _ = self
            .builder
            .build_call(rt, &[cv_ptr.into(), m_ptr.into()], "sync_condvar_wait")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_condvar_notify_one(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(cv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne named arg (receiver)",
                at: span.into(),
            });
        };

        let cv_v = self.codegen_expr_in_expected_context(cv_expr, Some(CgTy::Ref))?;
        let cv_v = self.coerce_value(cv_expr.span, cv_v, CgTy::Ref)?;
        let Some(cv_raw) = cv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne receiver value",
                at: cv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cv_ptr) = cv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne receiver type",
                at: cv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_condvar_notify_one();
        let _ = self
            .builder
            .build_call(rt, &[cv_ptr.into()], "sync_condvar_notify_one")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_condvar_notify_all(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(cv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll named arg (receiver)",
                at: span.into(),
            });
        };

        let cv_v = self.codegen_expr_in_expected_context(cv_expr, Some(CgTy::Ref))?;
        let cv_v = self.coerce_value(cv_expr.span, cv_v, CgTy::Ref)?;
        let Some(cv_raw) = cv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll receiver value",
                at: cv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cv_ptr) = cv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll receiver type",
                at: cv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_condvar_notify_all();
        let _ = self
            .builder
            .build_call(rt, &[cv_ptr.into()], "sync_condvar_notify_all")?;
        Ok(CgValue::unit())
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_once_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.onceCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_sync_once_create();
        let call = self.builder.build_call(rt, &[], "sync_once_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.onceCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.onceCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_once_is_done(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_once_is_done();
        let call = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_once_is_done")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(done_i1) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone return type",
                at: span.into(),
            });
        };

        Ok(CgValue::bool(done_i1))
    }

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_once_run(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // `fun Once.run(block: () -> Unit): Unit`：`args = [once, block]`
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(once_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(block_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run named arg (block)",
                at: span.into(),
            });
        };

        let once_v = self.codegen_expr_in_expected_context(once_expr, Some(CgTy::Ref))?;
        let once_v = self.coerce_value(once_expr.span, once_v, CgTy::Ref)?;
        let Some(once_raw) = once_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run receiver value",
                at: once_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(once_ptr) = once_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run receiver type",
                at: once_expr.span.into(),
            });
        };
        let deferred_once = self.defer_gc_ref_pointer(once_expr.span, "sync_once_run_receiver", once_ptr)?;

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                // 说明：
                // - `Once.run` 的参数类型在 sysroot 中固定为 `() -> Unit`；
                // - 但 early stage 的 `fun_index` 只包含"本编译单元内有 body 的函数"，不含 sysroot 声明；
                // - 同时 HIR v0 对 closure expr 的 `ty` 也不总是可用作 expected type（需要 MIR/CFG 才能更稳）。
                //
                // 因此这里从 `TypeStore` 中查找一个"无参、返回 Unit、Pure"的函数类型作为 expected context。
                let expected_fun_ty = self.lookup_pure_unit_closure_type().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.Once.run block fun type",
                        at: block_expr.span.into(),
                    },
                )?;
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let Some(block_raw) = block_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run block value",
                at: block_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(block_obj_i8) = block_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run block type",
                at: block_expr.span.into(),
            });
        };

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

    pub(in crate::llvm::codegen) fn codegen_sysroot_sync_destroy(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy named arg (receiver)",
                at: span.into(),
            });
        };

        // `destroy` 为 overload set：根据 receiver 的名义类型分派到不同 runtime 符号。
        // generic helper/method 体内的 `carrier.lock.destroy()` 往往以 member access 作为
        // receiver，因此这里需要和其它 builtin/member call 一样优先恢复 concrete type，
        // 不能只依赖局部 VarRef 或宽化后的 `expr.ty`。
        let recv_hir_ty = self.resolve_expr_concrete_type(recv_expr).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver hir type",
                at: recv_expr.span.into(),
            },
        )?;

        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(recv_hir_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver type kind",
                at: recv_expr.span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = match nominal.fqn.as_str() {
            "scoop.sync.Mutex" => self.declare_runtime_sync_mutex_destroy(),
            "scoop.sync.CondVar" => self.declare_runtime_sync_condvar_destroy(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sync.destroy receiver nominal",
                    at: recv_expr.span.into(),
                });
            }
        };
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_destroy")?;
        Ok(CgValue::unit())
    }
}
