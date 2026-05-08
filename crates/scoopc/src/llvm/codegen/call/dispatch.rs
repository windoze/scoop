//! Call dispatch and indirect-call lowering helpers.

use super::super::*;

/// direct-call target 已在 HIR 中物化为 `foo::<Bar>` 时，返回其模板 FQN `foo`。
///
/// 说明：
/// - 普通静态 direct-call 仍应继续使用完整实例 FQN 命中 `fun_index`；
/// - 只有 sysroot/builtin special-case dispatch、vtable/itable slot 识别等仍按模板名建模的
///   路径需要看这个“base/template FQN”。
fn direct_call_dispatch_fqn(fqn: &str) -> &str {
    if let Some((base, _)) = fqn.rsplit_once("::<") {
        return base;
    }
    fqn.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(fqn)
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn dispatch_call_kind_for_receiver(
        &self,
        span: crate::span::Span,
        receiver_ty: TypeId,
    ) -> Result<Option<hir::DispatchCallKind>, LlvmEmitError> {
        let source = self.current_source()?;
        Ok(self
            .dispatch_call_sites
            .get(&hir::DispatchCallSite::new(
                source.path().to_path_buf(),
                span,
                receiver_ty,
            ))
            .copied())
    }

    pub(in crate::llvm::codegen) fn codegen_call_impl(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self
            .continuation_resume_call_sites
            .contains(&self.current_call_site(span)?)
        {
            return self
                .codegen_continuation_resume_builtin(span, callee, args, expected, result_ty);
        }

        enum CallableCallee {
            FunctionValue(crate::ty::FunctionType),
            FunPtr(crate::ty::FunctionType),
        }

        let callable_callee = self
            .resolve_expr_concrete_type(callee)
            .and_then(|callee_hir_ty| match self.types.kind(callee_hir_ty) {
                TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                    Some(CallableCallee::FunctionValue(fun_ty.clone()))
                }
                TypeKind::Value(ValueTypeKind::Nominal(nominal))
                    if nominal.fqn == "scoop.unsafe.FunPtr" =>
                {
                    let sig_ty = nominal.args.first().copied()?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
                    else {
                        return None;
                    };
                    Some(CallableCallee::FunPtr(fun_ty.clone()))
                }
                _ => None,
            });
        if let Some(callable_callee) = callable_callee {
            match callable_callee {
                CallableCallee::FunctionValue(fun_ty) => {
                    let call_may_suspend = self
                        .function_value_expr_body_may_outward_effect_when_called_for_local(callee);
                    let callee_value = self.codegen_expr(callee)?;
                    let callee_v = self.coerce_value(callee.span, callee_value, CgTy::Ref)?;
                    let Some(BasicValueEnum::PointerValue(closure_obj_i8)) = callee_v.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "callable callee value type",
                            at: callee.span.into(),
                        });
                    };
                    return self.codegen_function_value_call_from_closure_obj(
                        closure_obj_i8,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend,
                            fun_ty: &fun_ty,
                            args,
                        },
                    );
                }
                CallableCallee::FunPtr(fun_ty) => {
                    let call_may_suspend = !fun_ty.effects.is_pure();
                    let callee_v = self.codegen_expr(callee)?;
                    let (funptr_addr, funptr_int_ty) =
                        callee_v
                            .as_int()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr callee value type",
                                at: callee.span.into(),
                            })?;
                    return self.codegen_funptr_value_call(
                        funptr_addr,
                        funptr_int_ty,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend,
                            fun_ty: &fun_ty,
                            args,
                        },
                    );
                }
            }
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind {
            let local = self.function_cx.env.get(*id).ok_or_else(|| {
                LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown local value",
                    at: callee.span.into(),
                }
            })?;

            if let Some(hir_ty) = local.hir_ty {
                if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(hir_ty) {
                    return self.codegen_function_value_call(
                        &local,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend: local.call_may_suspend,
                            fun_ty,
                            args,
                        },
                    );
                }

                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty)
                    && nominal.fqn == "scoop.unsafe.FunPtr"
                {
                    let sig_ty = nominal.args.first().copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature type",
                            at: callee.span.into(),
                        },
                    )?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature kind",
                            at: callee.span.into(),
                        });
                    };

                    let CgTy::Int(int_ty) = local.ty else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr local cg type",
                            at: callee.span.into(),
                        });
                    };
                    let local_ptr =
                        self.local_ptr_for_use(callee.span, local, "load_funptr_slot")?;
                    let loaded = self
                        .builder
                        .build_load(
                            self.llvm_basic_type_of(callee.span, local.ty)?,
                            local_ptr,
                            "load_funptr",
                        )?
                        .into_int_value();

                    return self.codegen_funptr_value_call(
                        loaded,
                        int_ty,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend: local.call_may_suspend,
                            fun_ty,
                            args,
                        },
                    );
                }
            }
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
            let dispatch_fqn = direct_call_dispatch_fqn(fqn);

            if dispatch_fqn == "scoop.unsafe.invoke" {
                return self.codegen_sysroot_funptr_invoke(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.unsafe.funPtrToUIntPtr" {
                return self.codegen_sysroot_funptr_to_uintptr(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.unsafe.uintPtrToFunPtr" {
                return self.codegen_sysroot_uintptr_to_funptr(span, callee.span, args);
            }

            if let Some(v) = self.try_codegen_class_vtable_call(span, callee.span, fqn, args)? {
                return Ok(v);
            }

            if dispatch_fqn == "scoop.core.ToString.toString"
                && let Some(v) = self.try_codegen_tostring_iface_builtin(span, callee.span, args)?
            {
                return Ok(v);
            }

            if let Some(v) = self.try_codegen_interface_itable_call(span, callee.span, fqn, args)? {
                return Ok(v);
            }
            if let Some(v) =
                self.try_codegen_sysroot_gc_debug_intrinsics(span, dispatch_fqn, args)?
            {
                return Ok(v);
            }
            if dispatch_fqn.starts_with("scoop.core.__scoop_effect_") {
                return self.codegen_sysroot_effect_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }
            if dispatch_fqn == "scoop.core.sizeOf" {
                return self.codegen_sysroot_size_of(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.getPlatform" {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "getPlatform intrinsic arity",
                        at: span.into(),
                    });
                }
                let target_cg = expected
                    .or_else(|| result_ty.and_then(|ty| self.cg_ty_of(ty)))
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "getPlatform intrinsic return type",
                        at: span.into(),
                    })?;
                return self.codegen_platform_literal(span, target_cg);
            }
            if dispatch_fqn == "scoop.core.panic" {
                return self.codegen_sysroot_panic(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.print" || dispatch_fqn == "scoop.core.println" {
                return self.codegen_sysroot_print_like(span, callee.span, dispatch_fqn, args);
            }
            if dispatch_fqn == "scoop.core.__scoop_print_string"
                || dispatch_fqn == "scoop.core.__scoop_println_string"
            {
                return self.codegen_sysroot_internal_print_string(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }
            if dispatch_fqn == "scoop.core.toString" {
                return self.codegen_sysroot_to_string_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.concat" {
                return self.codegen_sysroot_string_concat_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.compareTo" {
                return self.codegen_sysroot_string_compare_to_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.byteLength" {
                return self.codegen_sysroot_string_byte_length_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.getByte" {
                return self.codegen_sysroot_string_get_byte_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.unsafeSliceBytes" {
                return self.codegen_sysroot_string_unsafe_slice_bytes_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.toInt" {
                return self.codegen_sysroot_to_int_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.hash" {
                return self.codegen_sysroot_hash_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.abs"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(
                            self.resolve_expr_cg_ty(expr),
                            Some(CgTy::Float64 | CgTy::Float32)
                        )
                )
            {
                return self.codegen_sysroot_abs_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.isNaN"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(
                            self.resolve_expr_cg_ty(expr),
                            Some(CgTy::Float64 | CgTy::Float32)
                        )
                )
            {
                return self.codegen_sysroot_is_nan_ext(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.isInfinite"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(
                            self.resolve_expr_cg_ty(expr),
                            Some(CgTy::Float64 | CgTy::Float32)
                        )
                )
            {
                return self.codegen_sysroot_is_infinite_ext(span, callee.span, args);
            }

            if dispatch_fqn == "scoop.sync.mutexCreate" {
                return self.codegen_sysroot_sync_mutex_create(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.lock" {
                return self.codegen_sysroot_sync_mutex_lock(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.unlock" {
                return self.codegen_sysroot_sync_mutex_unlock(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.condVarCreate" {
                return self.codegen_sysroot_sync_condvar_create(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.wait" {
                return self.codegen_sysroot_sync_condvar_wait(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.notifyOne" {
                return self.codegen_sysroot_sync_condvar_notify_one(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.notifyAll" {
                return self.codegen_sysroot_sync_condvar_notify_all(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.onceCreate" {
                return self.codegen_sysroot_sync_once_create(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.isDone" {
                return self.codegen_sysroot_sync_once_is_done(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.run" {
                return self.codegen_sysroot_sync_once_run(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.sync.destroy" {
                return self.codegen_sysroot_sync_destroy(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.threadSpawn" {
                return self.codegen_sysroot_thread_spawn(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.join" {
                return self.codegen_sysroot_thread_join(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.sleepMillis" {
                return self.codegen_sysroot_thread_sleep_millis(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.yield" {
                return self.codegen_sysroot_thread_yield(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.thread.currentId" {
                return self.codegen_sysroot_thread_current_id(span, callee.span, args);
            }
            if dispatch_fqn == "scoop.core.__task_transport_pack"
                || dispatch_fqn == "scoop.core.__task_transport_unpack"
            {
                return self.codegen_sysroot_task_transport_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                    result_ty,
                );
            }
            if dispatch_fqn.starts_with("scoop.unsafe.__atomicInt") {
                return self.codegen_sysroot_atomic_int_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }
            if dispatch_fqn == "scoop.core.size"
                || dispatch_fqn == "scoop.core.get"
                || dispatch_fqn == "scoop.core.set"
            {
                return self.codegen_sysroot_array_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                    expected,
                );
            }
            if dispatch_fqn.starts_with("scoop.core.__scoop_array_builder_") {
                return self.codegen_sysroot_array_builder_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }
            if dispatch_fqn == "scoop.core.__scoop_thread_spawn_join_resume_u64" {
                return self.codegen_sysroot_thread_intrinsics(
                    span,
                    callee.span,
                    dispatch_fqn,
                    args,
                );
            }

            if let Some(callee_hir_ty) = self.top_level_value_ty(fqn) {
                if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(callee_hir_ty)
                {
                    let call_may_suspend = self
                        .function_value_expr_body_may_outward_effect_when_called_for_local(callee);
                    let callee_value = self.codegen_top_level_value_ref(callee.span, fqn)?;
                    let CgValue {
                        ty: CgTy::Ref,
                        value: Some(BasicValueEnum::PointerValue(closure_obj_i8)),
                    } = callee_value
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "function value top-level type",
                            at: callee.span.into(),
                        });
                    };
                    return self.codegen_function_value_call_from_closure_obj(
                        closure_obj_i8,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend,
                            fun_ty,
                            args,
                        },
                    );
                }

                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) =
                    self.types.kind(callee_hir_ty)
                    && nominal.fqn == "scoop.unsafe.FunPtr"
                {
                    let sig_ty = nominal.args.first().copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature type",
                            at: callee.span.into(),
                        },
                    )?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature kind",
                            at: callee.span.into(),
                        });
                    };

                    let callee_value = self.codegen_top_level_value_ref(callee.span, fqn)?;
                    let (funptr_addr, funptr_int_ty) =
                        callee_value
                            .as_int()
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr top-level cg type",
                                at: callee.span.into(),
                            })?;
                    return self.codegen_funptr_value_call(
                        funptr_addr,
                        funptr_int_ty,
                        CallableValueCallSpec {
                            span,
                            callee_span: callee.span,
                            call_may_suspend: !fun_ty.effects.is_pure(),
                            fun_ty,
                            args,
                        },
                    );
                }
            }
            return self.codegen_top_level_fun_call(span, callee.span, fqn, args);
        }

        if let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind {
            if let Some(hir::MemberRef::Fun { fqn, .. }) = member.resolved.as_ref() {
                if fqn == "scoop.core.GC.handleNew" {
                    return self.codegen_sysroot_gc_handle_new(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.handleGet" {
                    return self.codegen_sysroot_gc_handle_get(span, member.span, args);
                }
                if fqn == "scoop.core.GC.handleDrop" {
                    return self.codegen_sysroot_gc_handle_drop(span, member.span, args);
                }

                if fqn == "scoop.core.GC.pin" {
                    return self.codegen_sysroot_gc_pin(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.unpin" {
                    return self.codegen_sysroot_gc_unpin(span, member.span, args);
                }
            }

            if member.name == "trimIndent" {
                return self.codegen_string_trim_indent(span, receiver, args);
            }
            if member.name == "toInt" {
                let recv_ty = match &receiver.kind {
                    hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                        .function_cx
                        .env
                        .get(*id)
                        .and_then(|l| l.hir_ty)
                        .unwrap_or(receiver.ty),
                    _ => receiver.ty,
                };
                if matches!(
                    self.types.kind(recv_ty),
                    TypeKind::Value(ValueTypeKind::Char)
                ) {
                    return self.codegen_char_method_to_int(receiver);
                }
                if matches!(
                    self.types.kind(recv_ty),
                    TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32)
                ) {
                    let recv = self.codegen_expr(receiver)?;
                    return self.codegen_float_to_int_value(span, receiver.span, recv);
                }
                return self.codegen_string_method(span, receiver, &member.name, args);
            }
            if matches!(
                member.name.as_str(),
                "length"
                    | "concat"
                    | "isEmpty"
                    | "replace"
                    | "charAt"
                    | "repeat"
                    | "compareTo"
                    | "byteLength"
                    | "getByte"
                    | "unsafeSliceBytes"
            ) {
                return self.codegen_string_method(span, receiver, &member.name, args);
            }
            if member.name == "toString" {
                return self.codegen_to_string_method(span, receiver);
            }
            if member.name == "hash" {
                let recv_ty = match &receiver.kind {
                    hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                        .function_cx
                        .env
                        .get(*id)
                        .and_then(|l| l.hir_ty)
                        .unwrap_or(receiver.ty),
                    _ => receiver.ty,
                };
                match self.types.kind(recv_ty) {
                    TypeKind::Value(ValueTypeKind::Char) => {
                        return self.codegen_char_method_hash(span, receiver);
                    }
                    TypeKind::Value(ValueTypeKind::Int) => {
                        return self.codegen_int_method_hash(span, receiver);
                    }
                    TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32) => {
                        let recv = self.codegen_expr(receiver)?;
                        return self.codegen_float_hash_value(receiver.span, recv);
                    }
                    _ => {
                        return self.codegen_string_method(span, receiver, "hash", args);
                    }
                }
            }
            if matches!(member.name.as_str(), "abs" | "isNaN" | "isInfinite") {
                let recv = self.codegen_expr(receiver)?;
                return match member.name.as_str() {
                    "abs" => self.codegen_float_abs_value(receiver.span, recv),
                    "isNaN" => self.codegen_float_is_nan_value(receiver.span, recv),
                    "isInfinite" => self.codegen_float_is_infinite_value(receiver.span, recv),
                    _ => unreachable!("filtered by matches!"),
                };
            }

            if let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref()
                && let Some((_owner_fqn, variant_name)) = fqn.rsplit_once('.')
                && let Some(CgTy::Enum(enum_ty)) = expected
            {
                let layout = self.cg_enum_layout(span, enum_ty)?;
                if layout
                    .variants
                    .iter()
                    .any(|variant| variant.name == variant_name)
                {
                    return self.codegen_enum_variant_ctor_call(span, enum_ty, variant_name, args);
                }
            }
        }

        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind {
            let call_site = self.current_call_site(span)?;
            if let Some(site) = self.ctor_call_sites.get(&call_site) {
                return self.codegen_class_ctor_call(
                    span,
                    callee.span,
                    name,
                    args,
                    site,
                    result_ty,
                );
            }

            let Some(CgTy::Enum(enum_ty)) = expected else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum variant ctor call without expected enum type",
                    at: callee.span.into(),
                });
            };
            return self.codegen_enum_variant_ctor_call(span, enum_ty, name, args);
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "call callee",
            at: callee.span.into(),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_fun_call_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let is_extern = self.extern_funs.contains_key(fqn);

        let sig_fun =
            self.fun_index
                .get(fqn)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call callee type",
                    at: callee_span.into(),
                })?;
        let call_may_suspend = self.known_fun_body_may_outward_effect(fqn, sig_fun.ty);
        let explicit_effect_call = call_may_suspend && !is_extern;
        let uses_hidden_incoming_resume_token =
            self.top_level_fun_uses_hidden_incoming_resume_token(sig_fun);

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call arity mismatch",
                at: span.into(),
            });
        }

        let param_names: Vec<String> = sig_fun
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let param_tys: Vec<TypeId> = sig_fun.params.iter().map(|param| param.ty).collect();
        let ret_cg = if let Some(ret_cg) = self.cg_ty_of(sig_fun.return_ty) {
            ret_cg
        } else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call return type",
                at: span.into(),
            });
        };
        let hidden_sret_result_ty = if is_extern {
            None
        } else {
            self.hidden_sret_result_ty(callee_span, ret_cg)?
        };
        let evaluated_args = self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind: "call arg binding",
                abi_mode: if is_extern {
                    CallArgAbiMode::Native
                } else {
                    CallArgAbiMode::Ordinary
                },
            },
            &param_names,
            &param_tys,
            args,
        )?;
        let (effect_ctx_slot, effect_outcome_slot): (
            Option<PointerValue<'ctx>>,
            Option<PointerValue<'ctx>>,
        ) = if explicit_effect_call {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "direct_call")?;
            (Some(ctx_slot), Some(outcome_slot))
        } else {
            (None, None)
        };
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(!explicit_effect_call && uses_hidden_incoming_resume_token)
                + usize::from(explicit_effect_call) * 3,
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        if !explicit_effect_call && uses_hidden_incoming_resume_token {
            llvm_args.push(self.null_effect_resume_token().into());
        }
        if let (Some(ctx_slot), Some(outcome_slot)) = (effect_ctx_slot, effect_outcome_slot) {
            llvm_args.push(ctx_slot.into());
            llvm_args.push(self.null_effect_resume_token().into());
            llvm_args.push(outcome_slot.into());
        }
        llvm_args.extend(evaluated_args.iter().map(|slot| slot.value));

        let llvm_name = self
            .extern_funs
            .get(fqn)
            .map(|e| e.symbol.as_str())
            .unwrap_or(fqn);

        let llvm_fun = if explicit_effect_call {
            self.ensure_top_level_fun_effect_call_wrapper_defined(sig_fun)?
        } else {
            match self.module.get_function(llvm_name) {
                Some(f) => f,
                None => self.declare_top_level_fun(sig_fun)?,
            }
        };
        if !is_extern
            && !explicit_effect_call
            && llvm_fun.count_basic_blocks() == 0
            && sig_fun.body.is_some()
            && self
                .materialized_pass_view()
                .is_some_and(|pass_view| pass_view.callable(fqn).is_none())
        {
            let restore_block = self.builder.get_insert_block();
            self.fresh_child_codegen()
                .codegen_top_level_fun(sig_fun, llvm_fun)?;
            if let Some(block) = restore_block {
                self.builder.position_at_end(block);
            }
        }

        let call_site_result = if is_extern {
            self.emit_extern_native_call(span, fqn, llvm_fun, &llvm_args)
        } else {
            self.with_conservative_gc_local_root_spills(span, |cg| {
                let call_site = cg.builder.build_call(llvm_fun, &llvm_args, "call")?;
                if let Some(result_ty) = hidden_sret_result_ty {
                    cg.add_sret_attribute_to_call(call_site, 0, result_ty);
                }
                call_site.set_call_convention(cg.llvm_call_convention_for_fqn(fqn));
                Ok(call_site)
            })
        };
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "call_direct_result")?
        } else {
            None
        };
        if let Some(outcome_slot) = effect_outcome_slot {
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "direct_call_effect",
            )?;
        } else if call_may_suspend && !is_extern {
            self.emit_ordinary_call_effect_propagation_check(span, "direct_call_effect")?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(span, ret_cg, result_ptr, "call_sret")
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "call_direct_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "call deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    pub(in crate::llvm::codegen) fn emit_enter_native_for_extern_call_impl(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let slot_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let slots_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();

        let slots = self
            .collect_conservative_gc_root_slots(at)?
            .into_iter()
            .map(|(id, slot, _, frame_slot)| {
                let root_slot = if explicit_frame_enabled {
                    frame_slot
                } else {
                    slot
                };
                (id, root_slot)
            })
            .collect::<Vec<_>>();

        let (slots_base, slots_len) = if slots.is_empty() {
            (slots_ptr_ty.const_null(), i32_ty.const_zero())
        } else {
            let arr_ty = slot_ptr_ty.array_type(slots.len() as u32);
            let arr_ptr = self.create_entry_alloca_raw(at, "native_root_slots", arr_ty.into())?;
            let base =
                self.builder
                    .build_pointer_cast(arr_ptr, slots_ptr_ty, "native_root_slots_base")?;

            for (idx, (_id, local_ptr)) in slots.iter().enumerate() {
                let slot_ptr = self.builder.build_pointer_cast(
                    *local_ptr,
                    slot_ptr_ty,
                    "native_root_slot_cast",
                )?;
                let idx_v = i32_ty.const_int(idx as u64, false);
                let elem_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        slot_ptr_ty,
                        base,
                        &[idx_v],
                        &format!("native_root_slot_gep_{idx}"),
                    )?
                };
                let _ = self.builder.build_store(elem_ptr, slot_ptr)?;
            }

            (base, i32_ty.const_int(slots.len() as u64, false))
        };

        let enter = self.declare_runtime_enter_native();
        let enter_args: [inkwell::values::BasicMetadataValueEnum<'ctx>; 2] =
            [slots_base.into(), slots_len.into()];
        let _ = self
            .builder
            .build_call(enter, &enter_args, "enter_native")?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_extern_native_call_impl(
        &mut self,
        at: crate::span::Span,
        fqn: &str,
        llvm_fun: FunctionValue<'ctx>,
        llvm_args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<CallSiteValue<'ctx>, LlvmEmitError> {
        self.emit_enter_native_for_extern_call(at)?;

        let call_site = self.builder.build_call(llvm_fun, llvm_args, "call")?;
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(fqn));

        let leave = self.declare_runtime_leave_native();
        let _ = self.builder.build_call(leave, &[], "leave_native")?;
        Ok(call_site)
    }

    pub(in crate::llvm::codegen) fn try_codegen_class_vtable_call_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let dispatch_fqn = direct_call_dispatch_fqn(fqn);
        let Some((owner_fqn, method_name)) = dispatch_fqn.rsplit_once('.') else {
            return Ok(None);
        };

        let Some(slots) = self.class_vtables.get(owner_fqn) else {
            return Ok(None);
        };
        if slots.is_empty() {
            return Ok(None);
        }

        let Some((receiver_arg, _call_args)) = args.split_first() else {
            return Ok(None);
        };
        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Ok(None);
        };
        if !matches!(
            self.dispatch_call_kind_for_receiver(span, receiver_expr.ty)?,
            Some(hir::DispatchCallKind::Virtual)
        ) {
            return Ok(None);
        }

        let explicit_params_len = args.len().saturating_sub(1) as u32;
        let slot = slots
            .iter()
            .find(|s| s.name == method_name && s.params_len == explicit_params_len)
            .map(|s| s.slot);

        let Some(slot) = slot else {
            return Ok(None);
        };

        let sig_fun = self
            .fun_index
            .get(fqn)
            .or_else(|| self.fun_index.get(dispatch_fqn))
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call callee type",
                at: callee_span.into(),
            })?;
        let call_may_suspend = self.hir_ty_declared_effectful(Some(sig_fun.ty));

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "vtable call return type",
                    at: span.into(),
                })?;

        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            sig_fun.params.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        }
        for p in &sig_fun.params {
            llvm_param_tys.push(self.ordinary_param_abi(callee_span, p.ty)?.llvm_param_ty());
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let param_names: Vec<String> = sig_fun
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let param_tys: Vec<TypeId> = sig_fun.params.iter().map(|param| param.ty).collect();
        let evaluated_args = self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind: "vtable call arg binding",
                abi_mode: CallArgAbiMode::Ordinary,
            },
            &param_names,
            &param_tys,
            args,
        )?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call receiver type",
                at: callee_span.into(),
            })?;
        let deferred_receiver =
            self.defer_gc_ref_pointer(callee_span, "vtable_call_receiver", receiver_ptr)?;
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "vtable_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let incoming_resume_token = self.null_effect_resume_token();
        if call_may_suspend {
            llvm_args.push(incoming_resume_token.into());
        }
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));
        let effect_boundary = if call_may_suspend {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "vtable_call")?;
            let installed_top =
                self.load_effect_ctx_handler_top_from_slot(span, ctx_slot, "vtable_call")?;
            let saved_top =
                self.swap_effect_handler_stack_top(span, installed_top, "vtable_call")?;
            self.publish_incoming_resume_token(span, incoming_resume_token, "vtable_call")?;
            Some((outcome_slot, saved_top))
        } else {
            None
        };

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            callee_span,
            "vtable_call_receiver_reload",
            &deferred_receiver,
        )?;

        let fn_i8 = self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, slot)?;
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "vtable_fn_typed",
        )?;

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "call_vtable",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(cg.llvm_call_convention_for_fqn(fqn));
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "vtable_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "vtable_call_direct_result")?
        } else {
            None
        };
        if let Some((outcome_slot, saved_top)) = effect_boundary {
            self.consume_current_effect_outcome_into(span, outcome_slot, "vtable_call")?;
            self.clear_incoming_resume_token(span, "vtable_call")?;
            let _ = self.swap_effect_handler_stack_top(span, saved_top, "vtable_call_restore")?;
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "vtable_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => Ok(Some(if let Some(result_ptr) = sret_result_slot {
                self.load_hidden_sret_result_from_ptr(span, ret_cg, result_ptr, "vtable_call_sret")?
            } else {
                self.materialize_deferred_cg_value(
                    span,
                    "vtable_call_direct_result_reload",
                    deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "vtable call deferred return value",
                        at: span.into(),
                    })?,
                )?
            })),
        }
    }

    pub(in crate::llvm::codegen) fn try_codegen_interface_itable_call_impl(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let dispatch_fqn = direct_call_dispatch_fqn(fqn);
        let Some((owner_fqn, method_name)) = dispatch_fqn.rsplit_once('.') else {
            return Ok(None);
        };

        let Some(iface) = self.interfaces.get(owner_fqn) else {
            return Ok(None);
        };

        if args.is_empty() {
            return Ok(None);
        }

        let Some((receiver_arg, _call_args)) = args.split_first() else {
            return Ok(None);
        };
        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Ok(None);
        };
        if !matches!(
            self.dispatch_call_kind_for_receiver(span, receiver_expr.ty)?,
            Some(hir::DispatchCallKind::Interface)
        ) {
            return Ok(None);
        }

        let explicit_params_len = args.len().saturating_sub(1) as u32;
        let mut candidates = iface
            .method_slots
            .iter()
            .filter(|s| s.name == method_name && s.params_len == explicit_params_len);
        let Some(first) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call slot ambiguous",
                at: callee_span.into(),
            });
        }
        let slot = first.slot;

        let sig_fun = self
            .fun_index
            .get(fqn)
            .or_else(|| self.fun_index.get(dispatch_fqn))
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call callee type",
                at: callee_span.into(),
            })?;
        let call_may_suspend = self.hir_ty_declared_effectful(Some(sig_fun.ty));

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "itable call return type",
                    at: span.into(),
                })?;

        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            sig_fun.params.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        if hidden_sret_result_ty.is_some() {
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        }
        for p in &sig_fun.params {
            llvm_param_tys.push(self.ordinary_param_abi(callee_span, p.ty)?.llvm_param_ty());
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let param_names: Vec<String> = sig_fun
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect();
        let param_tys: Vec<TypeId> = sig_fun.params.iter().map(|param| param.ty).collect();
        let evaluated_args = self.codegen_bound_call_args(
            BoundCallArgsSpec {
                span,
                callee_span,
                kind: "itable call arg binding",
                abi_mode: CallArgAbiMode::Ordinary,
            },
            &param_names,
            &param_tys,
            args,
        )?;
        let receiver_ptr = evaluated_args
            .first()
            .and_then(|arg| arg.pointer_value)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call receiver type",
                at: callee_span.into(),
            })?;
        let deferred_receiver =
            self.defer_gc_ref_pointer(callee_span, "itable_call_receiver", receiver_ptr)?;
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            evaluated_args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "itable_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let incoming_resume_token = self.null_effect_resume_token();
        if call_may_suspend {
            llvm_args.push(incoming_resume_token.into());
        }
        llvm_args.extend(evaluated_args.iter().map(|arg| arg.value));
        let effect_boundary = if call_may_suspend {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "itable_call")?;
            let installed_top =
                self.load_effect_ctx_handler_top_from_slot(span, ctx_slot, "itable_call")?;
            let saved_top =
                self.swap_effect_handler_stack_top(span, installed_top, "itable_call")?;
            self.publish_incoming_resume_token(span, incoming_resume_token, "itable_call")?;
            Some((outcome_slot, saved_top))
        } else {
            None
        };

        let receiver_ptr = self.reload_deferred_gc_ref_without_clearing(
            callee_span,
            "itable_call_receiver_reload",
            &deferred_receiver,
        )?;

        let fn_i8 = self.load_interface_itable_slot_fn_ptr_i8(
            span,
            receiver_ptr,
            iface.interface_id,
            slot,
        )?;
        let fn_is_null = self.builder.build_is_null(fn_i8, "itable_fn_is_null")?;
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
        let ok_bb = self.context.append_basic_block(func, "itable_fn_ok");
        let bad_bb = self.context.append_basic_block(func, "itable_fn_null");
        self.builder
            .build_conditional_branch(fn_is_null, bad_bb, ok_bb)?;
        self.builder.position_at_end(bad_bb);
        let exit = self.declare_libc_exit();
        let code = self.context.i32_type().const_int(7, false);
        let _ = self
            .builder
            .build_call(exit, &[code.into()], "itable_fn_null_exit")?;
        self.builder.build_unreachable()?;
        self.builder.position_at_end(ok_bb);

        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "itable_fn_typed",
        )?;

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "call_itable",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(cg.llvm_call_convention_for_fqn(fqn));
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "itable_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "itable_call_direct_result")?
        } else {
            None
        };
        if let Some((outcome_slot, saved_top)) = effect_boundary {
            self.consume_current_effect_outcome_into(span, outcome_slot, "itable_call")?;
            self.clear_incoming_resume_token(span, "itable_call")?;
            let _ = self.swap_effect_handler_stack_top(span, saved_top, "itable_call_restore")?;
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "itable_call_effect",
            )?;
        }

        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => Ok(Some(if let Some(result_ptr) = sret_result_slot {
                self.load_hidden_sret_result_from_ptr(span, ret_cg, result_ptr, "itable_call_sret")?
            } else {
                self.materialize_deferred_cg_value(
                    span,
                    "itable_call_direct_result_reload",
                    deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "itable call deferred return value",
                        at: span.into(),
                    })?,
                )?
            })),
        }
    }

    pub(in crate::llvm::codegen) fn load_class_vtable_slot_fn_ptr_i8_impl(
        &mut self,
        _at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(receiver, header_ptr_ty, "vtable_hdr_ptr")?;

        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "vtable_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "load_type_desc")?
            .into_pointer_value();

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr = self
            .builder
            .build_pointer_cast(type_desc_i8, desc_ptr_ty, "type_desc")?;
        let vtable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 13, "type_desc_vtable_gep")?;
        let vtable_i8 = self
            .builder
            .build_load(i8_ptr_ty, vtable_field_ptr, "load_vtable")?
            .into_pointer_value();

        let vtable_entries_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let vtable_entries =
            self.builder
                .build_pointer_cast(vtable_i8, vtable_entries_ptr_ty, "vtable_entries")?;
        let slot_idx = i32_ty.const_int(slot as u64, false);
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ptr_ty,
                vtable_entries,
                &[slot_idx],
                "vtable_slot_ptr",
            )?
        };
        let fn_i8 = self
            .builder
            .build_load(i8_ptr_ty, slot_ptr, "load_vtable_fn")?
            .into_pointer_value();

        Ok(fn_i8)
    }

    pub(in crate::llvm::codegen) fn llvm_scoop_itable_entry_type_impl(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopItableEntry";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        ty.set_body(
            &[
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
            ],
            false,
        );
        ty
    }

    pub(in crate::llvm::codegen) fn llvm_scoop_itable_type_impl(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopItable";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_ty = entry_ty.array_type(0);
        ty.set_body(&[i32_ty.into(), i32_ty.into(), entries_ty.into()], false);
        ty
    }

    pub(in crate::llvm::codegen) fn load_interface_itable_slot_fn_ptr_i8_impl(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(receiver, header_ptr_ty, "itable_hdr_ptr")?;

        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "itable_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "load_type_desc")?
            .into_pointer_value();

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr = self
            .builder
            .build_pointer_cast(type_desc_i8, desc_ptr_ty, "type_desc")?;
        let itable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 12, "type_desc_itable_gep")?;
        let itable_i8 = self
            .builder
            .build_load(i8_ptr_ty, itable_field_ptr, "load_itable")?
            .into_pointer_value();

        let itable_is_null = self.builder.build_is_null(itable_i8, "itable_is_null")?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let null_bb = self.context.append_basic_block(func, "itable_null");
        let lookup_bb = self.context.append_basic_block(func, "itable_lookup");
        let done_bb = self.context.append_basic_block(func, "itable_done");
        self.builder
            .build_conditional_branch(itable_is_null, null_bb, lookup_bb)?;

        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(lookup_bb);
        let itable_ty = self.llvm_scoop_itable_type();
        let itable_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let itable_ptr = self
            .builder
            .build_pointer_cast(itable_i8, itable_ptr_ty, "itable_ptr")?;

        let len_ptr = self
            .builder
            .build_struct_gep(itable_ty, itable_ptr, 0, "itable_len_gep")?;
        let len_i32 = self
            .builder
            .build_load(i32_ty, len_ptr, "itable_len")?
            .into_int_value();

        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_field_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 2, "itable_entries_gep")?;
        let entry_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let entries_base =
            self.builder
                .build_pointer_cast(entries_field_ptr, entry_ptr_ty, "itable_entries")?;

        let loop_bb = self.context.append_basic_block(func, "itable_loop");
        let found_bb = self.context.append_basic_block(func, "itable_found");
        let not_found_bb = self.context.append_basic_block(func, "itable_not_found");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let idx_phi = self.builder.build_phi(i32_ty, "itable_idx")?;
        idx_phi.add_incoming(&[(&i32_ty.const_zero(), lookup_bb)]);
        let idx_i32 = idx_phi.as_basic_value().into_int_value();

        let cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            idx_i32,
            len_i32,
            "itable_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(cond, found_bb, not_found_bb)?;

        self.builder.position_at_end(found_bb);
        let entry_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                entry_ty,
                entries_base,
                &[idx_i32],
                "itable_entry_ptr",
            )?
        };
        let id_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 0, "itable_entry_id_gep")?;
        let id_i64 = self
            .builder
            .build_load(i64_ty, id_ptr, "itable_entry_id")?
            .into_int_value();

        let target_id = i64_ty.const_int(interface_id, false);
        let id_ok =
            self.builder
                .build_int_compare(IntPredicate::EQ, id_i64, target_id, "itable_id_eq")?;

        let hit_bb = self.context.append_basic_block(func, "itable_hit");
        let miss_bb = self.context.append_basic_block(func, "itable_miss");
        self.builder
            .build_conditional_branch(id_ok, hit_bb, miss_bb)?;

        self.builder.position_at_end(miss_bb);
        let next =
            self.builder
                .build_int_add(idx_i32, i32_ty.const_int(1, false), "itable_idx_next")?;
        idx_phi.add_incoming(&[(&next, miss_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        self.builder.position_at_end(hit_bb);
        let methods_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 4, "itable_entry_methods_gep")?;
        let methods_i8 = self
            .builder
            .build_load(i8_ptr_ty, methods_ptr, "itable_entry_methods")?
            .into_pointer_value();
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(not_found_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        let methods_phi = self.builder.build_phi(i8_ptr_ty, "itable_methods")?;
        methods_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), null_bb),
            (&i8_ptr_ty.const_null(), not_found_bb),
            (&methods_i8, hit_bb),
        ]);
        let methods_i8 = methods_phi.as_basic_value().into_pointer_value();

        let methods_is_null = self
            .builder
            .build_is_null(methods_i8, "itable_methods_is_null")?;
        let slot_null_bb = self.context.append_basic_block(func, "itable_slot_null");
        let slot_ok_bb = self.context.append_basic_block(func, "itable_slot_ok");
        let slot_done_bb = self.context.append_basic_block(func, "itable_slot_done");
        self.builder
            .build_conditional_branch(methods_is_null, slot_null_bb, slot_ok_bb)?;

        self.builder.position_at_end(slot_null_bb);
        self.builder.build_unconditional_branch(slot_done_bb)?;

        self.builder.position_at_end(slot_ok_bb);
        let methods_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let methods_entries = self.builder.build_pointer_cast(
            methods_i8,
            methods_ptr_ty,
            "itable_methods_entries",
        )?;
        let slot_idx = i32_ty.const_int(slot as u64, false);
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ptr_ty,
                methods_entries,
                &[slot_idx],
                "itable_slot_ptr",
            )?
        };
        let fn_i8 = self
            .builder
            .build_load(i8_ptr_ty, slot_ptr, "load_itable_fn")?
            .into_pointer_value();
        self.builder.build_unconditional_branch(slot_done_bb)?;

        self.builder.position_at_end(slot_done_bb);
        let fn_phi = self.builder.build_phi(i8_ptr_ty, "itable_fn_i8")?;
        fn_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), slot_null_bb),
            (&fn_i8, slot_ok_bb),
        ]);
        let fn_i8 = fn_phi.as_basic_value().into_pointer_value();

        Ok(fn_i8)
    }

    pub(in crate::llvm::codegen) fn codegen_funptr_value_call_impl(
        &mut self,
        funptr_addr: inkwell::values::IntValue<'ctx>,
        funptr_int_ty: IntTy,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CallableValueCallSpec {
            span,
            callee_span,
            call_may_suspend,
            fun_ty,
            args,
        } = call;
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr call arity mismatch",
                at: span.into(),
            });
        }

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr call return type",
                at: callee_span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;

        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            expected_arity
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            llvm_param_tys.push(self.llvm_gc_i8_ptr_type().into());
        }
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, receiver_ty)?);
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, *ty)?);
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        let fun_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let casted_addr = if funptr_int_ty.bits == self.host.word_bit_width() {
            funptr_addr
        } else {
            let from = funptr_int_ty;
            let to = IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            };
            self.cast_int(funptr_addr, from, to)?
        };
        let typed_fn_ptr =
            self.builder
                .build_int_to_ptr(casted_addr, fun_ptr_ty, "funptr_typed")?;

        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "funptr_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let incoming_resume_token = self.null_effect_resume_token();
        if call_may_suspend {
            llvm_args.push(incoming_resume_token.into());
        }
        let evaluated_args = self.codegen_callable_value_args(
            span,
            callee_span,
            fun_ty,
            args,
            "funptr call arg binding",
            CallArgAbiMode::Native,
        )?;
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let effect_boundary = if call_may_suspend {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "funptr_call")?;
            let installed_top =
                self.load_effect_ctx_handler_top_from_slot(span, ctx_slot, "funptr_call")?;
            let saved_top =
                self.swap_effect_handler_stack_top(span, installed_top, "funptr_call")?;
            self.publish_incoming_resume_token(span, incoming_resume_token, "funptr_call")?;
            Some((outcome_slot, saved_top))
        } else {
            None
        };

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "call_funptr",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            call_site.set_call_convention(0);
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "funptr_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "funptr_call_direct_result")?
        } else {
            None
        };
        if let Some((outcome_slot, saved_top)) = effect_boundary {
            self.consume_current_effect_outcome_into(span, outcome_slot, "funptr_call")?;
            self.clear_incoming_resume_token(span, "funptr_call")?;
            let _ = self.swap_effect_handler_stack_top(span, saved_top, "funptr_call_restore")?;
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "funptr_call_effect",
            )?;
        } else if call_may_suspend {
            self.emit_ordinary_call_effect_propagation_check(span, "funptr_call_effect")?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "funptr_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "funptr_call_direct_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr call deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }

    pub(in crate::llvm::codegen) fn codegen_function_value_call_impl(
        &mut self,
        local: &CgLocal<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let callee_span = call.callee_span;
        let CgTy::Ref = local.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function value local type",
                at: callee_span.into(),
            });
        };

        let llvm_local_ty = self.llvm_basic_type_of(callee_span, local.ty)?;
        let local_ptr = self.local_ptr_for_use(callee_span, *local, "load_closure_obj_slot")?;
        let closure_obj_i8 = self
            .builder
            .build_load(llvm_local_ty, local_ptr, "load_closure_obj")?
            .into_pointer_value();

        self.codegen_function_value_call_from_closure_obj(closure_obj_i8, call)
    }

    pub(in crate::llvm::codegen) fn codegen_function_value_call_from_closure_obj_impl(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let CallableValueCallSpec {
            span,
            callee_span,
            call_may_suspend,
            fun_ty,
            args,
        } = call;
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call arity mismatch",
                at: span.into(),
            });
        }
        let deferred_closure =
            self.defer_gc_ref_pointer(callee_span, "closure_call_obj", closure_obj_i8)?;

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call return type",
                at: callee_span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;

        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
            1 + expected_arity
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if call_may_suspend {
            llvm_param_tys.push(gc_i8_ptr_ty.into());
        }
        llvm_param_tys.push(gc_i8_ptr_ty.into());
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(
                self.ordinary_param_abi(callee_span, receiver_ty)?
                    .llvm_param_ty(),
            );
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.ordinary_param_abi(callee_span, *ty)?.llvm_param_ty());
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, CgTy::Bool) => self.context.bool_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float64) => self.context.f64_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float32) => self.context.f32_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Int(int_ty)) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
            (None, CgTy::String) => self
                .llvm_scoop_string_ptr_type()
                .fn_type(&llvm_param_tys, false),
            (None, CgTy::Ref) => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(
                "aggregate function-value returns should have been lowered through hidden sret"
            ),
        };

        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(
            1 + args.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + usize::from(call_may_suspend),
        );
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        let incoming_resume_token = self.null_effect_resume_token();
        if call_may_suspend {
            llvm_args.push(incoming_resume_token.into());
        }
        let evaluated_args = self.codegen_callable_value_args(
            span,
            callee_span,
            fun_ty,
            args,
            "function value call arg binding",
            CallArgAbiMode::Ordinary,
        )?;

        let effect_boundary = if call_may_suspend {
            let (ctx_slot, outcome_slot) =
                self.prepare_current_effect_call_contract(span, "closure_call")?;
            let installed_top =
                self.load_effect_ctx_handler_top_from_slot(span, ctx_slot, "closure_call")?;
            let saved_top =
                self.swap_effect_handler_stack_top(span, installed_top, "closure_call")?;
            self.publish_incoming_resume_token(span, incoming_resume_token, "closure_call")?;
            Some((outcome_slot, saved_top))
        } else {
            None
        };

        let closure_obj_i8 = self.reload_deferred_gc_ref_without_clearing(
            callee_span,
            "closure_call_obj_reload",
            &deferred_closure,
        )?;
        let closure_ptr =
            self.builder
                .build_pointer_cast(closure_obj_i8, closure_ptr_ty, "closure_obj_ptr")?;
        let env_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "closure_env_gep")?;
        let fn_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "closure_fn_gep")?;
        let env_ptr = self
            .builder
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "closure_fn")?
            .into_pointer_value();
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "closure_fn_typed",
        )?;
        llvm_args.push(env_ptr.into());
        for arg in &evaluated_args {
            llvm_args.push(arg.value);
        }

        let call_site_result = self.with_conservative_gc_local_root_spills(span, |cg| {
            let call_site = cg.builder.build_indirect_call(
                llvm_fun_ty,
                typed_fn_ptr,
                &llvm_args,
                "call_closure",
            )?;
            if let Some(result_ty) = hidden_sret_result_ty {
                cg.add_sret_attribute_to_call(call_site, 0, result_ty);
            }
            Ok(call_site)
        });
        self.release_evaluated_call_arg_roots(&evaluated_args);
        let call_site = call_site_result?;
        if let Some(result_ptr) = sret_result_slot {
            self.sync_hidden_sret_result_roots(span, ret_cg, result_ptr, "closure_call_sret")?;
        }
        let deferred_direct_result = if sret_result_slot.is_none() {
            self.defer_direct_call_result(span, ret_cg, call_site, "closure_call_direct_result")?
        } else {
            None
        };
        if let Some((outcome_slot, saved_top)) = effect_boundary {
            self.consume_current_effect_outcome_into(span, outcome_slot, "closure_call")?;
            self.clear_incoming_resume_token(span, "closure_call")?;
            let _ = self.swap_effect_handler_stack_top(span, saved_top, "closure_call_restore")?;
            self.maybe_record_active_suspend_site_effect_outcome(span, outcome_slot);
            self.emit_ordinary_call_effect_propagation_check_from_outcome(
                span,
                outcome_slot,
                "closure_call_effect",
            )?;
        } else if call_may_suspend {
            self.emit_ordinary_call_effect_propagation_check(span, "closure_call_effect")?;
        }

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_hidden_sret_result_from_ptr(
                        span,
                        ret_cg,
                        result_ptr,
                        "closure_call_sret",
                    )
                } else {
                    self.materialize_deferred_cg_value(
                        span,
                        "closure_call_direct_result_reload",
                        deferred_direct_result.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "function value deferred return value",
                            at: span.into(),
                        })?,
                    )
                }
            }
        }
    }
}
