//! GC/statepoint codegen（T0102e：从 `codegen/mod.rs` 拆分）。

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn declare_dispatch_target_fun(
        &mut self,
        _at: crate::span::Span,
        target: scoopc_lir_facts::LirCallableRef,
        target_label: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let program = self.expect_active_lir_program("declare_dispatch_target_fun");
        let symbol_facts = program
            .physical_layout()
            .callable_symbols
            .get(&target.local_id().ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "dispatch target `{target_label}` uses external callable ref `{}` without local symbol facts",
                    target.display_text(),
                ),
            })?)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "dispatch target callable `{target_label}` 缺少 LIR callable symbol facts"
                ),
            })?;
        let source_types =
            self.published_late_lowered_types()
                .ok_or_else(|| LlvmEmitError::Frontend {
                    message: format!(
                        "dispatch target callable `{target_label}` 缺少 LIR TypeStore contract"
                    ),
                })?;
        let (param_tys, return_ty) = self
            .published_signature_tys_as_codegen_tys_impl(
                source_types,
                symbol_facts.param_tys.clone(),
                symbol_facts.return_ty,
            )
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "dispatch target callable `{target_label}` 的 LIR signature 无法映射到 LLVM codegen TypeStore"
                ),
            })?;
        let signature = CodegenCallableSignature {
            fqn: symbol_facts.root_fqn.clone(),
            param_names: symbol_facts.param_names.clone(),
            param_tys,
            return_ty,
        };
        let abi_symbol_fact = self.abi_symbol_for_lir_callable_ref(target);
        let llvm_name = symbol_facts
            .exported_symbol
            .as_deref()
            .or_else(|| {
                symbol_facts
                    .native
                    .as_ref()
                    .map(|native| native.symbol.as_str())
                    .or_else(|| {
                        symbol_facts
                            .extern_
                            .as_ref()
                            .map(|extern_| extern_.symbol.as_str())
                    })
                })
            .or_else(|| abi_symbol_fact.map(|fact| fact.symbol.as_str()))
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "dispatch target callable `{target_label}` 的 LIR callable ABI symbol fact 缺少 symbol"
                ),
            })?
            .to_string();
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }
        let surface = if abi_symbol_fact
            .is_some_and(|fact| matches!(fact.role.as_str(), "native_callable" | "extern_callable"))
        {
            LlvmFunctionDeclarationSurface::RuntimeOrNativeImport
        } else {
            LlvmFunctionDeclarationSurface::ExportedAbi
        };
        self.declare_lir_plain_fun_with_symbol(
            &llvm_name,
            surface,
            &signature.fqn,
            &signature.param_tys,
            signature.return_ty,
            self.types,
            false,
        )
    }

    pub(super) fn release_trampoline_fn_name(&self, class_fqn: &str) -> String {
        let stable_key = self.stable_nominal_type_key(class_fqn, "release_trampoline");
        let readable = sanitize_llvm_ident(class_fqn);
        let hash = PrivateSymbolMangler.hash_suffix("release_trampoline", &stable_key);
        format!("__scoop_release_{readable}__h{hash}")
    }

    pub(super) fn codegen_release_trampolines(&mut self) -> Result<(), LlvmEmitError> {
        let mut class_keys = self.class_inits.keys().cloned().collect::<Vec<_>>();
        class_keys.sort_by(|left, right| left.as_str().cmp(right.as_str()));

        let at = crate::span::Span::new(0, 0);
        for class_key in class_keys {
            if !self.release_hooks.contains_key(class_key.as_str()) {
                continue;
            }
            let _ = self.get_or_create_release_trampoline(at, &class_key)?;
        }
        Ok(())
    }

    pub(super) fn get_or_create_release_trampoline(
        &mut self,
        at: crate::span::Span,
        class_key: &hir::ClassInstanceKey,
    ) -> Result<Option<FunctionValue<'ctx>>, LlvmEmitError> {
        let class = self.class_init_layout(at, class_key)?;
        let Some(hook) = self.release_hooks.get(&class.fqn).cloned() else {
            return Ok(None);
        };

        self.codegen_release_trampoline_for_hook(at, &class, &hook)
            .map(Some)
    }

    fn codegen_release_trampoline_for_hook(
        &mut self,
        at: crate::span::Span,
        class: &hir::MonoClassInit,
        hook: &hir::ReleaseHook,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let name = self.release_trampoline_fn_name(&class.fqn);
        let object_param_ty = self.llvm_i8_ptr_type();
        let fn_ty = self
            .context
            .void_type()
            .fn_type(&[object_param_ty.into()], false);
        let trampoline =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);
        trampoline.set_call_conventions(0);
        self.mark_gc_leaf_function(trampoline);
        if trampoline.count_basic_blocks() > 0 {
            return Ok(trampoline);
        }

        let signature = self
            .published_codegen_callable_signature(&hook.target_fqn)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "release hook target `{}` 缺少 LIR callable signature contract",
                    hook.target_fqn
                ),
            })?;
        if signature.param_tys.len() != hook.arg_fields.len() {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "release hook `{}` 参数数量漂移：target params={} args={}",
                    class.fqn,
                    signature.param_tys.len(),
                    hook.arg_fields.len()
                ),
            });
        }

        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(trampoline, "entry");
        self.builder.position_at_end(entry);

        let object = trampoline
            .get_nth_param(0)
            .unwrap_or_else(|| panic!("release trampoline declaration must have object parameter"))
            .into_pointer_value();
        object.set_name("object");

        let mut args = Vec::<inkwell::values::BasicMetadataValueEnum<'ctx>>::with_capacity(
            hook.arg_fields.len(),
        );
        for (idx, field_name) in hook.arg_fields.iter().enumerate() {
            let param_ty = signature.param_tys[idx];
            let param_cg = self.cg_ty_of_type_id(param_ty, "release hook target param");
            let field_value = self.codegen_release_hook_field_arg(at, class, object, field_name)?;
            let coerced = self.coerce_value(at, field_value, param_cg)?;
            args.push(self.as_llvm_arg_value(at, param_cg, coerced)?);
        }

        let llvm_name = self
            .published_symbol_for_source_root_text(&hook.target_fqn)
            .unwrap_or_else(|| hook.target_fqn.clone());
        let target = self.declare_lir_plain_fun_with_symbol(
            &llvm_name,
            LlvmFunctionDeclarationSurface::ExportedAbi,
            &signature.fqn,
            &signature.param_tys,
            signature.return_ty,
            self.types,
            false,
        )?;
        let call = self
            .builder
            .build_call(target, &args, "release_hook_call")?;
        call.set_call_convention(self.llvm_call_convention_for_fqn(&hook.target_fqn));
        self.builder.build_return(None)?;

        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }
        Ok(trampoline)
    }

    fn codegen_release_hook_field_arg(
        &mut self,
        at: crate::span::Span,
        class: &hir::MonoClassInit,
        object: PointerValue<'ctx>,
        field_name: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let field_idx = self.release_hook_field_index(class, field_name)?;
        let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
            panic!("release hook verifier accepted field index outside class layout")
        });
        let field_cg = self.cg_ty_of(field.ty);
        let field_ptr = self.codegen_class_field_ptr(at, class, object, field_idx)?;
        let llvm_ty = self.llvm_basic_type_of(at, field_cg)?;
        let load_name = format!("release_hook_arg_{}", sanitize_llvm_ident(field_name));
        let loaded = self.builder.build_load(llvm_ty, field_ptr, &load_name)?;
        self.cg_value_from_loaded(at, field_cg, loaded)
    }

    fn release_hook_field_index(
        &self,
        class: &hir::MonoClassInit,
        field_name: &str,
    ) -> Result<u32, LlvmEmitError> {
        let field_fqn = if field_name.contains('.') {
            field_name.to_string()
        } else {
            format!("{}.{}", class.fqn, field_name)
        };
        if let Some(idx) = class.field_indices.get(&field_fqn).copied() {
            return Ok(idx);
        }

        class
            .fields
            .iter()
            .position(|field| field.name == field_name && field.fqn == field_fqn)
            .map(|idx| idx as u32)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "release hook `{}` references field `{}` missing from class layout",
                    class.fqn, field_name
                ),
            })
    }

    pub(super) fn try_codegen_sysroot_gc_debug_intrinsics(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        // Most runtime test helpers are ordinary extern declarations in `scoop.runtime.test`.
        // Stackmap smoke is intentionally special: its callsite must opt back into LLVM
        // statepoint lowering so the object file contains a real stackmap record.
        if fqn == "scoop.runtime.test.__scoop_stackmap_statepoint_smoke" {
            if !args.is_empty() {
                self.panic_verified_intrinsic_contract(
                    "stackmap statepoint smoke HIR lowering",
                    "argument list drift",
                );
            }

            // 重要：
            // - 默认 explicit mode 不再给托管函数统一打 `gc "statepoint-example"`；
            // - 该 helper 是保留下来的“显式 opt-in stackmap smoke”边界，因此需要仅对当前函数
            //   恢复 LLVM statepoint GC strategy，让调用点重新产出真实 statepoint/stackmap record；
            // - 这里只给显式调用该 helper 的函数开启该策略，避免把默认 explicit-root-frame 路线
            //   再次退回到隐式 stackmap 依赖。
            let current_fun =
                self.expect_current_function("stackmap statepoint smoke HIR lowering");
            current_fun.set_gc("statepoint-example");

            // 该 helper 仍必须经 ordinary managed runtime call 进入 IR；不能走 `@Extern` 的
            // native/leaf lowering，否则调用点本身不会留下 stackmap record。
            let rt = self.declare_runtime_stackmap_statepoint_smoke();
            let call = self.build_call_preserving_gc_local_roots(
                span,
                rt,
                &[],
                "stackmap_statepoint_smoke",
            )?;
            let raw = self.expect_basic_value(call, "stackmap statepoint smoke runtime return");
            let raw_int = self.expect_int_value(raw, "stackmap statepoint smoke runtime return");

            let from = IntTy {
                bits: 64,
                signed: true,
            };
            let to = IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            };
            let casted = self.cast_int(raw_int, from, to)?;
            return Ok(Some(CgValue::int(casted, to)));
        }

        Ok(None)
    }

    pub(super) fn codegen_sysroot_gc_pin(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let obj_expr = self.expect_hir_positional_intrinsic_arg(args, 1, 0, "GC.pin HIR lowering");

        let Some(CgTy::Struct(pinned_ty)) = expected else {
            self.panic_verified_intrinsic_contract(
                "GC.pin HIR lowering",
                "missing expected Pinned result type",
            );
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", callee_span)?;

        let obj_v = self.codegen_expr_in_expected_context(obj_expr, Some(field_cg_ty))?;
        let obj_v = self.coerce_value(obj_expr.span, obj_v, field_cg_ty)?;

        // 运行期 pin 需要 `void*`：统一使用 `i8*`。
        let obj_ref = self.coerce_value(obj_expr.span, obj_v, CgTy::Ref)?;
        let obj_ptr = self.expect_cg_pointer(obj_ref, "GC.pin argument");

        let rt_pin = self.declare_runtime_gc_pin();
        let call = self
            .builder
            .build_call(rt_pin, &[obj_ptr.into()], "gc_pin")?;
        let raw = self.expect_basic_value(call, "GC.pin runtime return");
        let ok_i32 = self.expect_int_value(raw, "GC.pin runtime return");

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_pin_ok",
        )?;

        let func = self.expect_current_function("GC.pin branch blocks");

        let ok_bb = self.context.append_basic_block(func, "gc_pin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "gc_pin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "gc_pin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let llvm_struct_ty = self.llvm_struct_type(span, pinned_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        let raw_field: BasicValueEnum<'ctx> = match field_cg_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            _ => self.expect_cg_value(obj_v, "GC.pin Pinned.value field"),
        };
        agg = self
            .builder
            .build_insert_value(agg, raw_field, field_idx, "pinned_value")?;
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(pinned_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(super) fn codegen_sysroot_gc_handle_new(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let obj_expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "GC.handleNew HIR lowering");

        let Some(CgTy::Struct(handle_ty)) = expected else {
            self.panic_verified_intrinsic_contract(
                "GC.handleNew HIR lowering",
                "missing expected GcHandle result type",
            );
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", callee_span)?;
        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "GC.handleNew HIR lowering",
                "GcHandle.raw field is not an integer",
            );
        };

        let obj_v = self.codegen_expr_in_expected_context(obj_expr, Some(CgTy::Ref))?;
        let obj_ref = self.coerce_value(obj_expr.span, obj_v, CgTy::Ref)?;
        let obj_ptr = self.expect_cg_pointer(obj_ref, "GC.handleNew argument");

        let rt_handle_new = self.declare_runtime_gc_handle_new();
        let call = self
            .builder
            .build_call(rt_handle_new, &[obj_ptr.into()], "gc_handle_new")?;
        let raw = self.expect_basic_value(call, "GC.handleNew runtime return");
        let handle_i64 = self.expect_int_value(raw, "GC.handleNew runtime return");

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            handle_i64,
            self.context.i64_type().const_zero(),
            "gc_handle_new_ok",
        )?;

        let func = self.expect_current_function("GC.handleNew branch blocks");

        let ok_bb = self.context.append_basic_block(func, "gc_handle_new_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "gc_handle_new_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "gc_handle_new_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let from = IntTy {
            bits: 64,
            signed: false,
        };
        let handle_word = self.cast_int(handle_i64, from, field_int_ty)?;
        let llvm_struct_ty = self.llvm_struct_type(span, handle_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        agg = self.builder.build_insert_value(
            agg,
            handle_word.as_basic_value_enum(),
            field_idx,
            "gc_handle_raw",
        )?;
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(handle_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    pub(super) fn codegen_sysroot_gc_handle_get(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        let handle_expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "GC.handleGet HIR lowering");

        let handle_v = self.codegen_expr(handle_expr)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            self.panic_verified_intrinsic_contract(
                "GC.handleGet HIR lowering",
                "argument is not a GcHandle struct",
            );
        };
        let raw = self.expect_cg_value(handle_v, "GC.handleGet argument");
        let struct_v = self.expect_struct_value(raw, "GC.handleGet argument");

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", handle_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(handle_expr.span, field_cg_ty, extracted)?;

        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "GC.handleGet HIR lowering",
                "GcHandle.raw field is not an integer",
            );
        };
        let field_raw = self.expect_cg_value(field_v, "GC.handleGet raw handle field");
        let handle_word = self.expect_int_value(field_raw, "GC.handleGet raw handle field");

        let to_i64 = IntTy {
            bits: 64,
            signed: false,
        };
        let handle_i64 = self.cast_int(handle_word, field_int_ty, to_i64)?;

        let rt_handle_get = self.declare_runtime_gc_handle_get();
        let call = self
            .builder
            .build_call(rt_handle_get, &[handle_i64.into()], "gc_handle_get")?;
        let raw = self.expect_basic_value(call, "GC.handleGet runtime return");
        let obj_ptr = self.expect_pointer_value(raw, "GC.handleGet runtime return");

        let obj_is_null = self
            .builder
            .build_is_null(obj_ptr, "gc_handle_get_is_null")?;
        let ok_cond = self.builder.build_not(obj_is_null, "gc_handle_get_ok")?;

        let func = self.expect_current_function("GC.handleGet branch blocks");

        let ok_bb = self.context.append_basic_block(func, "gc_handle_get_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "gc_handle_get_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "gc_handle_get_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_ptr.into()),
        })
    }

    pub(super) fn codegen_sysroot_gc_handle_drop(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        let handle_expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "GC.handleDrop HIR lowering");

        let handle_v = self.codegen_expr(handle_expr)?;
        let CgTy::Struct(handle_ty) = handle_v.ty else {
            self.panic_verified_intrinsic_contract(
                "GC.handleDrop HIR lowering",
                "argument is not a GcHandle struct",
            );
        };
        let raw = self.expect_cg_value(handle_v, "GC.handleDrop argument");
        let struct_v = self.expect_struct_value(raw, "GC.handleDrop argument");

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(handle_ty, "scoop.core.GcHandle.raw", handle_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "gc_handle_raw")?;
        let field_v = self.cg_value_from_loaded(handle_expr.span, field_cg_ty, extracted)?;

        let CgTy::Int(field_int_ty) = field_cg_ty else {
            self.panic_verified_intrinsic_contract(
                "GC.handleDrop HIR lowering",
                "GcHandle.raw field is not an integer",
            );
        };
        let field_raw = self.expect_cg_value(field_v, "GC.handleDrop raw handle field");
        let handle_word = self.expect_int_value(field_raw, "GC.handleDrop raw handle field");

        let to_i64 = IntTy {
            bits: 64,
            signed: false,
        };
        let handle_i64 = self.cast_int(handle_word, field_int_ty, to_i64)?;

        let rt_handle_drop = self.declare_runtime_gc_handle_drop();
        let call =
            self.builder
                .build_call(rt_handle_drop, &[handle_i64.into()], "gc_handle_drop")?;
        let raw = self.expect_basic_value(call, "GC.handleDrop runtime return");
        let ok_i32 = self.expect_int_value(raw, "GC.handleDrop runtime return");

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_handle_drop_ok",
        )?;

        let func = self.expect_current_function("GC.handleDrop branch blocks");

        let ok_bb = self
            .context
            .append_basic_block(func, "gc_handle_drop_ok_bb");
        let err_bb = self
            .context
            .append_basic_block(func, "gc_handle_drop_err_bb");
        let cont_bb = self
            .context
            .append_basic_block(func, "gc_handle_drop_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    pub(super) fn codegen_sysroot_gc_unpin(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        let pinned_expr =
            self.expect_hir_positional_intrinsic_arg(args, 1, 0, "GC.unpin HIR lowering");

        let pinned_v = self.codegen_expr(pinned_expr)?;
        let CgTy::Struct(pinned_ty) = pinned_v.ty else {
            self.panic_verified_intrinsic_contract(
                "GC.unpin HIR lowering",
                "argument is not a Pinned struct",
            );
        };
        let raw = self.expect_cg_value(pinned_v, "GC.unpin argument");
        let struct_v = self.expect_struct_value(raw, "GC.unpin argument");

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", pinned_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "pinned_value")?;
        let field_v = self.cg_value_from_loaded(pinned_expr.span, field_cg_ty, extracted)?;
        let field_ref = self.coerce_value(pinned_expr.span, field_v, CgTy::Ref)?;

        let obj_ptr = self.expect_cg_pointer(field_ref, "GC.unpin Pinned.value field");

        let rt_unpin = self.declare_runtime_gc_unpin();
        let call = self
            .builder
            .build_call(rt_unpin, &[obj_ptr.into()], "gc_unpin")?;
        let raw = self.expect_basic_value(call, "GC.unpin runtime return");
        let ok_i32 = self.expect_int_value(raw, "GC.unpin runtime return");

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_unpin_ok",
        )?;

        let func = self.expect_current_function("GC.unpin branch blocks");

        let ok_bb = self.context.append_basic_block(func, "gc_unpin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "gc_unpin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "gc_unpin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    pub(super) fn gc_address_space(&self) -> AddressSpace {
        AddressSpace::from(GC_ADDRSPACE)
    }

    pub(super) fn llvm_ptr_type(&self, address_space: AddressSpace) -> PointerType<'ctx> {
        self.context.ptr_type(address_space)
    }

    pub(super) fn llvm_ptr_sized_int_type(
        &self,
        address_space: Option<AddressSpace>,
    ) -> IntType<'ctx> {
        self.context
            .ptr_sized_int_type(self.target_data, address_space)
    }

    /// LLVM addrspace(0)：native/unsafe 指针（C ABI / malloc buffer 等）。
    pub(super) fn llvm_i8_ptr_type(&self) -> PointerType<'ctx> {
        self.llvm_ptr_type(AddressSpace::default())
    }

    /// LLVM addrspace(1)：GC-managed 引用指针（Any/class/interface/closure/...）。
    pub(super) fn llvm_gc_i8_ptr_type(&self) -> PointerType<'ctx> {
        self.llvm_ptr_type(self.gc_address_space())
    }

    pub(super) fn llvm_scoop_string_type(&self) -> StructType<'ctx> {
        // 说明：该类型名用于 LLVM module 内部复用，不应与用户类型冲突（使用 runtime 命名空间前缀）。
        const TY_NAME: &str = "scoop.runtime.ScoopString";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { ScoopGcObjectHeader hdr; uint64_t len; const uint8_t *data; } ScoopString;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let len_ty = self.context.i64_type();
        let data_ty = self.llvm_i8_ptr_type();
        ty.set_body(&[header_ty.into(), len_ty.into(), data_ty.into()], false);
        ty
    }

    pub(super) fn llvm_scoop_array_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopArray";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct ScoopArray {
        //    ScoopGcObjectHeader header;
        //    uint64_t len;
        //    uint64_t elem_size_bytes;
        //    uint64_t data_offset_bytes;
        //    const ScoopCompositeTransportDescriptor *elem_desc;
        //    uint32_t elem_kind;
        //    uint32_t _reserved_u32;
        //    uint8_t data[];
        // } ScoopArray;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        ty.set_body(
            &[
                header_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                ptr_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_scoop_mutable_array_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopMutableArray";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct ScoopMutableArray {
        //    ScoopGcObjectHeader header;
        //    uint64_t len;
        //    uint64_t cap;
        //    uint64_t elem_size_bytes;
        //    uint64_t elem_align_bytes;
        //    const ScoopCompositeTransportDescriptor *elem_desc;
        //    uint8_t *data;
        //    uint32_t elem_kind;
        //    uint32_t _reserved_u32;
        // } ScoopMutableArray;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        ty.set_body(
            &[
                header_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                ptr_ty.into(),
                ptr_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_scoop_string_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.llvm_ptr_type(self.gc_address_space())
    }

    fn local_gc_root_value_ptr_type(
        &mut self,
        at: crate::span::Span,
        local: &CgLocal<'ctx>,
    ) -> Result<Option<PointerType<'ctx>>, LlvmEmitError> {
        let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
        let BasicTypeEnum::PointerType(ptr_ty) = llvm_ty else {
            return Ok(None);
        };

        if ptr_ty.get_address_space() == self.gc_address_space() {
            Ok(Some(ptr_ty))
        } else {
            Ok(None)
        }
    }

    /// 收集当前 env 中“底层 LLVM 表示就是 GC 指针”的局部槽位。
    ///
    /// 说明：
    /// - 不能只按 `CgTy::Ref | CgTy::String` 判断；
    /// - `Option<Ref>` / `Option<Continuation>` 等 niche enum 也可能直接降成 `ptr addrspace(1)`；
    /// - statepoint 只会追踪 live SSA roots，不会自动把 `alloca ptr addrspace(1)` 当作根，
    ///   因此 ordinary 函数里的这类局部必须在 safepoint 前显式 load 成 SSA 值并在之后写回。
    pub(super) fn collect_conservative_gc_root_slots(
        &mut self,
        at: crate::span::Span,
    ) -> Result<
        Vec<(
            u32,
            PointerValue<'ctx>,
            PointerType<'ctx>,
            PointerValue<'ctx>,
        )>,
        LlvmEmitError,
    > {
        let mut locals = Vec::new();
        for frame in &self.function_cx.env.scopes {
            for (id, local) in frame {
                locals.push((id.as_u32(), *local));
            }
        }

        let mut slots = Vec::new();
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();
        for (local_id, local) in locals {
            if let Some(value_ptr_ty) = self.local_gc_root_value_ptr_type(at, &local)? {
                let slot_ptr =
                    self.local_ptr_for_use(at, local, &format!("gc_root_slot_{local_id}"))?;
                let needs_spill = self.conservative_gc_root_slot_needs_spill_writeback(slot_ptr);
                let frame_slot = if explicit_frame_enabled && needs_spill {
                    self.explicit_frame_slot_mirrors_for(local.ptr)
                        .and_then(|slots| slots.first().copied())
                        .map(|slot| {
                            self.rematerialize_ptr_in_current_block(
                                at,
                                slot,
                                &format!("explicit_gc_root_slot_{local_id}"),
                            )
                        })
                        .transpose()?
                        .unwrap_or_else(|| {
                            panic!(
                                "collect_conservative_gc_root_slots: explicit-frame verifier accepted missing local root mirror"
                            )
                        })
                } else {
                    slot_ptr
                };
                slots.push((local_id, slot_ptr, value_ptr_ty, frame_slot));
            }
        }
        for (index, extra) in self
            .function_cx
            .tracked_gc_root_slots
            .clone()
            .into_iter()
            .enumerate()
        {
            let slot = self.rematerialize_ptr_in_current_block(
                at,
                extra.slot,
                &format!("tracked_gc_root_slot_{index}"),
            )?;
            let frame_slot = if explicit_frame_enabled
                && self.conservative_gc_root_slot_needs_spill_writeback(slot)
            {
                self.rematerialize_ptr_in_current_block(
                    at,
                    extra.frame_slot,
                    &format!("tracked_explicit_gc_root_slot_{index}"),
                )?
            } else {
                slot
            };
            slots.push((
                u32::MAX - index as u32,
                slot,
                extra.value_ptr_ty,
                frame_slot,
            ));
        }
        slots.sort_by_key(|(id, _, _, _)| *id);
        Ok(slots)
    }

    fn conservative_gc_root_slot_needs_spill_writeback(&self, slot: PointerValue<'ctx>) -> bool {
        slot.get_type().get_address_space() == AddressSpace::default()
    }

    /// 在一次 ordinary safepoint 前后保守 keepalive 所有“需要手动 spill/writeback”的
    /// pointer-shaped GC locals。
    ///
    /// 做法：
    /// - 仅对 stack/native-slot 一类的 stack-backed 槽位执行 `load -> gc-live -> writeback`；
    /// - heap-backed 槽位（例如 effect frame / GC object field）本身已位于 traced heap 中，
    ///   运行时会直接更新真实槽位，不能再把“调用前旧 keepalive”写回覆盖新值；
    /// - 对 stack-backed 槽位，调用前先从局部槽位 load 出 SSA root，调用后再把
    ///   relocate 后的 SSA root store 回原槽位；
    /// - 这样 `rewrite-statepoints-for-gc` 才会把这些 stack locals 纳入 `gc-live` / stackmap。
    pub(super) fn with_conservative_gc_local_root_spills<T, F>(
        &mut self,
        at: crate::span::Span,
        f: F,
    ) -> Result<T, LlvmEmitError>
    where
        F: FnOnce(&mut Self) -> Result<T, LlvmEmitError>,
    {
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();
        let spills = self
            .collect_conservative_gc_root_slots(at)?
            .into_iter()
            .filter(|(_, slot, _, _)| self.conservative_gc_root_slot_needs_spill_writeback(*slot))
            .map(|(local_id, slot, value_ptr_ty, frame_slot)| {
                let source_slot = if explicit_frame_enabled {
                    frame_slot
                } else {
                    slot
                };
                let loaded = self
                    .builder
                    .build_load(
                        value_ptr_ty,
                        source_slot,
                        &format!("gc_root_keepalive_{local_id}"),
                    )?
                    .into_pointer_value();
                Ok((slot, frame_slot, loaded, value_ptr_ty))
            })
            .collect::<Result<Vec<_>, LlvmEmitError>>()?;

        let result = f(self)?;

        let Some(insert_block) = self.builder.get_insert_block() else {
            return Ok(result);
        };
        if let Some(term) = insert_block.get_terminator() {
            let builder = self.context.create_builder();
            builder.position_before(&term);
            for (slot, frame_slot, value, value_ptr_ty) in spills {
                if explicit_frame_enabled {
                    let reloaded = builder
                        .build_load(value_ptr_ty, frame_slot, "gc_root_keepalive_reload")?
                        .into_pointer_value();
                    let _ = builder.build_store(slot, reloaded)?;
                } else {
                    let _ = builder.build_store(slot, value)?;
                }
            }
            return Ok(result);
        }

        for (slot, frame_slot, value, value_ptr_ty) in spills {
            if explicit_frame_enabled {
                let reloaded = self
                    .builder
                    .build_load(value_ptr_ty, frame_slot, "gc_root_keepalive_reload")?
                    .into_pointer_value();
                let _ = self.builder.build_store(slot, reloaded)?;
            } else {
                let _ = self.builder.build_store(slot, value)?;
            }
        }

        Ok(result)
    }

    pub(super) fn build_call_preserving_gc_local_roots(
        &mut self,
        at: crate::span::Span,
        callee: FunctionValue<'ctx>,
        args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
        name: &str,
    ) -> Result<CallSiteValue<'ctx>, LlvmEmitError> {
        self.with_conservative_gc_local_root_spills(at, |cg| {
            Ok(cg.builder.build_call(callee, args, name)?)
        })
    }

    pub(super) fn llvm_gc_object_header_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_gc.h` 的 `ScoopGcObjectHeader`（TODO T0908）；
        // - 当前阶段用 `i8*` 作为 `next` 与 `type_desc` 的承载类型（不暴露具体指针类型）；
        // - 布局必须与 C runtime 一致，否则 `scoop_alloc` 初始化的对象头会被错误解释。
        const TY_NAME: &str = "scoop.runtime.ScoopGcObjectHeader";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { void* next; void* type_desc; uint64_t size_bytes; uint32_t flags; uint32_t mark; } ScoopGcObjectHeader;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        ty.set_body(
            &[
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
            ],
            false,
        );
        ty
    }

    pub(super) fn llvm_scoop_type_descriptor_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_gc.h` 的 `ScoopTypeDescriptor`（ABI 已在 T1501 固化）；
        // - 这里只需要保证字段顺序与大小匹配；具体偏移由 runtime 的 `_Static_assert` 与
        //   `crates/scoop_runtime/tests/object_model_abi.rs` 双向约束。
        const TY_NAME: &str = "scoop.runtime.ScoopTypeDescriptor";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let u64_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        // self-referential：parent_type_desc 指向同一 struct 类型。
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        // 字段顺序必须与 C 定义一致（见 `runtime/c/scoop_gc.h`）。
        ty.set_body(
            &[
                i32_ty.into(),      // abi_version
                i32_ty.into(),      // flags
                i64_ty.into(),      // size_bytes
                i64_ty.into(),      // align_bytes
                i64_ty.into(),      // trace_start_offset_bytes
                i32_ty.into(),      // trace_bitmap_u64_len
                i32_ty.into(),      // _reserved_u32
                u64_ptr_ty.into(),  // trace_bitmap (const uint64_t*)
                i8_ptr_ty.into(),   // trace_fn
                i8_ptr_ty.into(),   // release_fn
                i64_ty.into(),      // type_id
                desc_ptr_ty.into(), // parent_type_desc
                i8_ptr_ty.into(),   // itable
                i8_ptr_ty.into(),   // vtable
            ],
            false,
        );

        ty
    }

    pub(super) fn collect_gc_ptr_offsets_in_basic_type(
        &self,
        _at: crate::span::Span,
        ty: BasicTypeEnum<'ctx>,
        base_off: u64,
        out: &mut Vec<u64>,
    ) -> Result<(), LlvmEmitError> {
        match ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push(base_off);
                }
            }
            BasicTypeEnum::StructType(st) => {
                if st.is_opaque() {
                    return Ok(());
                }
                let fields = st.get_field_types();
                for (idx, field_ty) in fields.into_iter().enumerate() {
                    let off = self
                        .target_data
                        .offset_of_element(&st, idx as u32)
                        .unwrap_or_else(|| {
                            panic!(
                                "collect_gc_ptr_offsets_in_basic_type: target data omitted struct field offset"
                            )
                        });
                    self.collect_gc_ptr_offsets_in_basic_type(_at, field_ty, base_off + off, out)?;
                }
            }
            BasicTypeEnum::ArrayType(arr) => {
                let elem = arr.get_element_type();
                let stride = self.target_data.get_store_size(&elem);
                let len = arr.len() as u64;
                for i in 0..len {
                    let elem_off = base_off + i.saturating_mul(stride);
                    self.collect_gc_ptr_offsets_in_basic_type(_at, elem, elem_off, out)?;
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(())
    }

    pub(super) fn trace_bitmap_words_for_struct(
        &self,
        at: crate::span::Span,
        obj_ty: StructType<'ctx>,
        trace_start_offset_bytes: u64,
    ) -> Result<Vec<u64>, LlvmEmitError> {
        if obj_ty.is_opaque() {
            return Ok(Vec::new());
        }

        let ptr_size = self.target_layout().pointer_size.max(1);
        let size_bytes = self.target_data.get_store_size(&obj_ty);
        if trace_start_offset_bytes >= size_bytes {
            return Ok(Vec::new());
        }
        if !trace_start_offset_bytes.is_multiple_of(ptr_size) {
            return Ok(Vec::new());
        }

        let mut ptr_offsets: Vec<u64> = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(at, obj_ty.into(), 0, &mut ptr_offsets)?;
        ptr_offsets.sort();
        ptr_offsets.dedup();

        let mut word_indices: Vec<u64> = Vec::new();
        for off in ptr_offsets {
            if off < trace_start_offset_bytes {
                continue;
            }
            let rel = off - trace_start_offset_bytes;
            if !rel.is_multiple_of(ptr_size) {
                continue;
            }
            word_indices.push(rel / ptr_size);
        }

        word_indices.sort();
        word_indices.dedup();
        let Some(&max_idx) = word_indices.last() else {
            return Ok(Vec::new());
        };

        let len_u64 = (max_idx / 64) + 1;
        let mut words = vec![0u64; len_u64 as usize];
        for idx in word_indices {
            let wi = (idx / 64) as usize;
            let bit = (idx % 64) as u32;
            words[wi] |= 1u64 << bit;
        }
        Ok(words)
    }

    pub(super) fn get_or_create_trace_bitmap_global(
        &mut self,
        name: &str,
        words: &[u64],
    ) -> GlobalValue<'ctx> {
        if let Some(existing) = self.module.get_global(name) {
            return existing;
        }

        let i64_ty = self.context.i64_type();
        let arr_ty = i64_ty.array_type(words.len() as u32);
        let gv = self.module.add_global(arr_ty, None, name);

        let mut inits: Vec<IntValue<'ctx>> = Vec::with_capacity(words.len());
        for &w in words {
            inits.push(i64_ty.const_int(w, false));
        }

        gv.set_initializer(&i64_ty.const_array(&inits));
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        gv
    }

    fn type_descriptor_release_fn_ptr(
        &mut self,
        at: crate::span::Span,
        type_id_key: &str,
        i8_ptr_ty: PointerType<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if !self.release_hooks.contains_key(type_id_key) {
            return Ok(i8_ptr_ty.const_null());
        }

        let class_key = self.registered_class_instance_key(type_id_key).ok_or_else(|| {
            LlvmEmitError::Frontend {
                message: format!(
                    "type descriptor `{type_id_key}` has a release hook but no class layout metadata"
                ),
            }
        })?;
        let trampoline = self
            .get_or_create_release_trampoline(at, &class_key)?
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "type descriptor `{type_id_key}` has a release hook but no release trampoline"
                ),
            })?;
        Ok(trampoline
            .as_global_value()
            .as_pointer_value()
            .const_cast(i8_ptr_ty))
    }

    pub(super) fn get_or_create_type_descriptor_global(
        &mut self,
        spec: TypeDescriptorSpec<'ctx, '_>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let TypeDescriptorSpec {
            at,
            global_name,
            type_id_key,
            obj_ty,
            trace_start_offset_bytes,
            parent,
            itable,
            vtable,
        } = spec;
        if let Some(existing) = self.module.get_global(global_name) {
            return Ok(existing);
        }

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let u64_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        let size_bytes = self.target_data.get_store_size(&obj_ty);
        let align_bytes = self.target_layout().pointer_align.max(1);

        let bitmap_words =
            self.trace_bitmap_words_for_struct(at, obj_ty, trace_start_offset_bytes)?;
        let (bitmap_len_u32, bitmap_ptr) = if bitmap_words.is_empty() {
            (0u32, u64_ptr_ty.const_null())
        } else {
            let bitmap_name = format!("{global_name}__trace_bitmap");
            let bitmap_gv = self.get_or_create_trace_bitmap_global(&bitmap_name, &bitmap_words);
            let ptr = bitmap_gv.as_pointer_value().const_cast(u64_ptr_ty);
            (bitmap_words.len() as u32, ptr)
        };

        let parent_ptr = parent
            .map(|p| p.as_pointer_value())
            .unwrap_or_else(|| desc_ptr_ty.const_null());

        let itable_ptr = itable.unwrap_or_else(|| i8_ptr_ty.const_null());
        let vtable_ptr = vtable.unwrap_or_else(|| i8_ptr_ty.const_null());
        let release_fn_ptr = self.type_descriptor_release_fn_ptr(at, type_id_key, i8_ptr_ty)?;

        let values: [BasicValueEnum<'ctx>; 14] = [
            i32_ty.const_zero().into(), // abi_version
            i32_ty.const_zero().into(), // flags
            i64_ty.const_int(size_bytes, false).into(),
            i64_ty.const_int(align_bytes, false).into(),
            i64_ty.const_int(trace_start_offset_bytes, false).into(),
            i32_ty.const_int(bitmap_len_u32 as u64, false).into(),
            i32_ty.const_zero().into(), // _reserved_u32
            bitmap_ptr.into(),
            i8_ptr_ty.const_null().into(), // trace_fn
            release_fn_ptr.into(),         // release_fn
            i64_ty
                .const_int(stable_rtti_type_id(type_id_key), false)
                .into(),
            parent_ptr.into(),
            itable_ptr.into(),
            vtable_ptr.into(),
        ];

        let init = desc_ty.const_named_struct(&values);
        let gv = self.module.add_global(desc_ty, None, global_name);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(gv)
    }

    pub(super) fn basic_type_contains_gc_ptrs(
        &self,
        at: crate::span::Span,
        ty: BasicTypeEnum<'ctx>,
    ) -> Result<bool, LlvmEmitError> {
        let mut ptr_offsets = Vec::new();
        self.collect_gc_ptr_offsets_in_basic_type(at, ty, 0, &mut ptr_offsets)?;
        Ok(!ptr_offsets.is_empty())
    }

    pub(super) fn get_or_create_global_root_type_desc_global(
        &mut self,
        at: crate::span::Span,
        global_name: &str,
        storage_ty: BasicTypeEnum<'ctx>,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if !self.basic_type_contains_gc_ptrs(at, storage_ty)? {
            return Ok(None);
        }

        let wrapper_ty = self.context.struct_type(&[storage_ty], false);
        let desc_name = format!("{global_name}__global_root_type_desc");
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &desc_name,
            type_id_key: global_name,
            obj_ty: wrapper_ty,
            trace_start_offset_bytes: 0,
            parent: None,
            itable: None,
            vtable: None,
        })
        .map(Some)
    }

    pub(super) fn get_or_create_class_type_desc_global(
        &mut self,
        at: crate::span::Span,
        class_key: &hir::ClassInstanceKey,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let stable_key = self.stable_nominal_type_key(class_key.as_str(), "class_type_desc");
        let global_name = PrivateSymbolMangler.mangle("class_type_desc", &stable_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let class = self.class_init_layout(at, class_key)?;
        let parent = if let Some(super_fqn) = class.super_class_fqn.as_deref() {
            let super_key = self.registered_class_instance_key(super_fqn).ok_or_else(|| {
                LlvmEmitError::Frontend {
                    message: format!(
                        "class type descriptor `{class_key}` references superclass `{super_fqn}` without ClassInstanceKey metadata"
                    ),
                }
            })?;
            Some(self.get_or_create_class_type_desc_global(at, &super_key)?)
        } else {
            None
        };

        let obj_ty = self.llvm_class_object_type(at, &class)?;
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);

        let itable_ptr = self
            .get_or_create_class_itable_global(at, class_key.as_str())?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));

        let vtable_ptr = self
            .get_or_create_class_vtable_global(at, class_key.as_str())?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));

        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: &class.fqn,
            obj_ty,
            trace_start_offset_bytes,
            parent,
            itable: itable_ptr,
            vtable: vtable_ptr,
        })
    }

    pub(super) fn llvm_object_singleton_type(&self, object_fqn: &str) -> StructType<'ctx> {
        let stable_key = self.stable_nominal_type_key(object_fqn, "object_singleton_type");
        let name =
            PrivateSymbolMangler.type_name("ObjectSingleton", "object_singleton_type", &stable_key);
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into()], false);
        ty
    }

    pub(super) fn get_or_create_object_singleton_type_desc_global(
        &mut self,
        at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let stable_key = self.stable_nominal_type_key(object_fqn, "object_singleton_type_desc");
        let global_name = PrivateSymbolMangler.mangle("object_type_desc", &stable_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_object_singleton_type(object_fqn);
        let itable_ptr = self
            .get_or_create_class_itable_global(at, object_fqn)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));
        let vtable_ptr = self
            .get_or_create_class_vtable_global(at, object_fqn)?
            .map(|gv| gv.as_pointer_value().const_cast(self.llvm_i8_ptr_type()));

        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: object_fqn,
            obj_ty,
            trace_start_offset_bytes: 0,
            parent: None,
            itable: itable_ptr,
            vtable: vtable_ptr,
        })
    }

    pub(super) fn get_or_create_class_itable_global(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        let Some(itable) = self
            .expect_active_lir_program("get_or_create_class_itable_global")
            .physical_layout()
            .class_itables
            .get(class_fqn)
        else {
            return Ok(None);
        };
        let entries = itable.entries.as_slice();
        if entries.is_empty() {
            return Ok(None);
        }

        let owner_key = self.stable_nominal_type_key(class_fqn, "itable_owner");
        self.get_or_create_itable_global_from_entries(at, &owner_key, entries)
    }

    pub(super) fn get_or_create_itable_global_from_entries<K>(
        &mut self,
        at: crate::span::Span,
        owner_key: &K,
        entries: &[LirClassItableEntryFacts],
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError>
    where
        K: StableCanonicalKey + ?Sized,
    {
        if entries.is_empty() {
            return Ok(None);
        }

        let global_name = PrivateSymbolMangler.mangle("itable", owner_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(Some(existing));
        }

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // itable entry：
        // {
        //   interface_id: u64,
        //   match_len: u32,
        //   _reserved: u32,
        //   match_ids: i8*,
        //   methods: i8*,
        //   receiver_type_ids: i8*
        // }
        //
        // 说明：
        // - `methods` 指向一个 `i8*[]`（函数指针数组），按 interface slot 顺序排列；
        // - `match_ids` 指向一个 `u64[]`，保存该具体 interface 实例在运行期可匹配的 target type ids。
        // - `receiver_type_ids` 指向一个 `u64[]`，按 slot 发布 receiver marshal metadata：
        //   `0` 表示继续按 ref/object ptr 传递；非 `0` 表示需要从 value-box payload 里按该
        //   stable type id 对应的 concrete value nominal 形状重建 receiver。
        let entry_ty = self.context.struct_type(
            &[
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
            ],
            false,
        );

        let mut entry_inits: Vec<inkwell::values::StructValue<'ctx>> =
            Vec::with_capacity(entries.len());

        for entry in entries {
            // 1) 生成 method table：`i8*[]`。
            let methods_key = CanonicalTextKey::new(canonical_record(
                "itable_methods",
                [
                    owner_key.canonical_text(),
                    format!("{:016x}", entry.interface_id),
                    format!("{:016x}", entry.interface_type_id),
                ],
            ));
            let methods_gv_name = PrivateSymbolMangler.mangle("itable_methods", &methods_key);

            let methods_gv = if let Some(existing) = self.module.get_global(&methods_gv_name) {
                existing
            } else {
                let arr_ty = i8_ptr_ty.array_type(entry.method_impl_targets.len() as u32);
                let gv = self.module.add_global(arr_ty, None, &methods_gv_name);

                let mut inits: Vec<PointerValue<'ctx>> =
                    Vec::with_capacity(entry.method_impl_targets.len());
                for target in &entry.method_impl_targets {
                    let Some(target) = *target else {
                        inits.push(i8_ptr_ty.const_null());
                        continue;
                    };
                    let target_label = target.display_text();
                    let llvm_fun = self.declare_dispatch_target_fun(at, target, &target_label)?;
                    let key = callable_carrier_target_key_for_ref(
                        self.expect_active_lir_program("class itable carrier target"),
                        CallableCarrierKind::InterfaceItable,
                        target,
                        "class itable carrier target",
                    )?;

                    let fn_ptr = self.callable_carrier_target_fn_ptr(
                        key,
                        &target_label,
                        llvm_fun.as_global_value().as_pointer_value(),
                    )?;
                    inits.push(fn_ptr.const_cast(i8_ptr_ty));
                }

                gv.set_initializer(&i8_ptr_ty.const_array(&inits));
                gv.set_constant(true);
                gv.set_linkage(Linkage::Internal);
                gv
            };

            let methods_ptr_i8 = methods_gv.as_pointer_value().const_cast(i8_ptr_ty).into();

            let receiver_type_ids_ptr_i8 = if entry
                .method_receiver_type_ids
                .iter()
                .all(|id| *id == crate::itable::ITABLE_RECEIVER_REF_TYPE_ID)
            {
                i8_ptr_ty.const_null().into()
            } else {
                let receiver_types_key = CanonicalTextKey::new(canonical_record(
                    "itable_receiver_type_ids",
                    [
                        owner_key.canonical_text(),
                        format!("{:016x}", entry.interface_id),
                        format!("{:016x}", entry.interface_type_id),
                    ],
                ));
                let receiver_types_gv_name =
                    PrivateSymbolMangler.mangle("itable_receiver_type_ids", &receiver_types_key);
                let receiver_types_gv =
                    if let Some(existing) = self.module.get_global(&receiver_types_gv_name) {
                        existing
                    } else {
                        let arr_ty = i64_ty.array_type(entry.method_impl_targets.len() as u32);
                        let gv = self
                            .module
                            .add_global(arr_ty, None, &receiver_types_gv_name);
                        let inits = entry
                            .method_receiver_type_ids
                            .iter()
                            .copied()
                            .chain(std::iter::repeat(
                                crate::itable::ITABLE_RECEIVER_REF_TYPE_ID,
                            ))
                            .take(entry.method_impl_targets.len())
                            .map(|id| i64_ty.const_int(id, false))
                            .collect::<Vec<_>>();
                        gv.set_initializer(&i64_ty.const_array(&inits));
                        gv.set_constant(true);
                        gv.set_linkage(Linkage::Internal);
                        gv
                    };
                receiver_types_gv
                    .as_pointer_value()
                    .const_cast(i8_ptr_ty)
                    .into()
            };

            let match_ids_ptr_i8 = if entry.runtime_match_type_ids.is_empty() {
                i8_ptr_ty.const_null().into()
            } else {
                let match_key = CanonicalTextKey::new(canonical_record(
                    "itable_match_ids",
                    [
                        owner_key.canonical_text(),
                        format!("{:016x}", entry.interface_id),
                        format!("{:016x}", entry.interface_type_id),
                    ],
                ));
                let match_gv_name = PrivateSymbolMangler.mangle("itable_match_ids", &match_key);
                let match_gv = if let Some(existing) = self.module.get_global(&match_gv_name) {
                    existing
                } else {
                    let arr_ty = i64_ty.array_type(entry.runtime_match_type_ids.len() as u32);
                    let gv = self.module.add_global(arr_ty, None, &match_gv_name);
                    let inits = entry
                        .runtime_match_type_ids
                        .iter()
                        .map(|id| i64_ty.const_int(*id, false))
                        .collect::<Vec<_>>();
                    gv.set_initializer(&i64_ty.const_array(&inits));
                    gv.set_constant(true);
                    gv.set_linkage(Linkage::Internal);
                    gv
                };
                match_gv.as_pointer_value().const_cast(i8_ptr_ty).into()
            };

            let init = entry_ty.const_named_struct(&[
                i64_ty.const_int(entry.interface_id, false).into(),
                i32_ty
                    .const_int(entry.runtime_match_type_ids.len() as u64, false)
                    .into(),
                i32_ty.const_zero().into(),
                match_ids_ptr_i8,
                methods_ptr_i8,
                receiver_type_ids_ptr_i8,
            ]);
            entry_inits.push(init);
        }

        let entries_arr_ty = entry_ty.array_type(entry_inits.len() as u32);
        let entries_arr_init = entry_ty.const_array(&entry_inits);

        // itable：{ len: i32, _reserved: i32, entries: [N x Entry] }
        let itable_ty = self.context.struct_type(
            &[i32_ty.into(), i32_ty.into(), entries_arr_ty.into()],
            false,
        );
        let itable_init = itable_ty.const_named_struct(&[
            i32_ty.const_int(entries.len() as u64, false).into(),
            i32_ty.const_zero().into(),
            entries_arr_init.into(),
        ]);

        let gv = self.module.add_global(itable_ty, None, &global_name);
        gv.set_initializer(&itable_init);
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(Some(gv))
    }

    pub(super) fn get_or_create_class_vtable_global(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        let Some(slots) = self
            .expect_active_lir_program("get_or_create_class_vtable_global")
            .physical_layout()
            .class_vtables
            .get(class_fqn)
        else {
            return Ok(None);
        };
        if slots.is_empty() {
            return Ok(None);
        }

        let stable_key = self.stable_nominal_type_key(class_fqn, "class_vtable");
        let global_name = PrivateSymbolMangler.mangle("class_vtable", &stable_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(Some(existing));
        }

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let arr_ty = i8_ptr_ty.array_type(slots.len() as u32);
        let gv = self.module.add_global(arr_ty, None, &global_name);

        let mut inits: Vec<PointerValue<'ctx>> = Vec::with_capacity(slots.len());
        for slot in slots {
            let target = slot.impl_member_target;
            let target_label = target.display_text();
            let llvm_fun = self.declare_dispatch_target_fun(at, target, &target_label)?;
            let key = callable_carrier_target_key_for_ref(
                self.expect_active_lir_program("class vtable carrier target"),
                CallableCarrierKind::ClassVtable,
                target,
                "class vtable carrier target",
            )?;

            let fn_ptr = self.callable_carrier_target_fn_ptr(
                key,
                &target_label,
                llvm_fun.as_global_value().as_pointer_value(),
            )?;
            inits.push(fn_ptr.const_cast(i8_ptr_ty));
        }

        gv.set_initializer(&i8_ptr_ty.const_array(&inits));
        gv.set_constant(true);
        gv.set_linkage(Linkage::Internal);
        Ok(Some(gv))
    }

    fn get_or_create_closure_runtime_type_desc_global(
        &mut self,
        at: crate::span::Span,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        const GLOBAL_NAME: &str = "__scoop_type_desc_runtime__ScoopClosure";
        if let Some(existing) = self.module.get_global(GLOBAL_NAME) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_closure_object_type();
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: GLOBAL_NAME,
            type_id_key: "scoop.runtime.ScoopClosure",
            obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn get_or_create_closure_object_type_desc_global(
        &mut self,
        at: crate::span::Span,
        fun_ty: TypeId,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let (receiver, params, return_ty) = match self.types.kind(fun_ty) {
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                (fun.receiver, fun.params.clone(), fun.return_ty)
            }
            _ => {
                panic!(
                    "get_or_create_closure_object_type_desc_global: expected function target type"
                );
            }
        };
        self.get_or_create_closure_object_type_desc_for_signature(at, receiver, &params, return_ty)
    }

    pub(super) fn get_or_create_closure_object_type_desc_for_signature(
        &mut self,
        at: crate::span::Span,
        receiver: Option<TypeId>,
        params: &[TypeId],
        return_ty: TypeId,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let base_type_key = self.closure_runtime_signature_type_key(receiver, params, return_ty)?;
        let key = CanonicalTextKey::new(base_type_key.clone());
        let global_name = PrivateSymbolMangler.mangle("closure_type_desc", &key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_closure_object_type();
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        let parent = self.get_or_create_closure_runtime_type_desc_global(at)?;
        let type_id_key = stable_rtti_derived_type_key("closure_type_desc", &base_type_key);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: type_id_key.as_str(),
            obj_ty,
            trace_start_offset_bytes,
            parent: Some(parent),
            itable: None,
            vtable: None,
        })
    }

    fn closure_runtime_signature_type_key(
        &self,
        receiver: Option<TypeId>,
        params: &[TypeId],
        return_ty: TypeId,
    ) -> Result<String, LlvmEmitError> {
        let mut key = String::from("closure.fn(");
        if let Some(receiver) = receiver {
            key.push_str("receiver=");
            key.push_str(&self.canonical_type_key_text_for_codegen(
                receiver,
                "closure runtime receiver signature",
            )?);
            key.push(';');
        }
        key.push_str("params=[");
        for (idx, param) in params.iter().copied().enumerate() {
            if idx != 0 {
                key.push(',');
            }
            key.push_str(
                &self.canonical_type_key_text_for_codegen(
                    param,
                    "closure runtime param signature",
                )?,
            );
        }
        key.push_str("];return=");
        key.push_str(
            &self.canonical_type_key_text_for_codegen(
                return_ty,
                "closure runtime return signature",
            )?,
        );
        key.push(')');
        Ok(key)
    }

    pub(super) fn get_or_create_closure_env_type_desc_global(
        &mut self,
        at: crate::span::Span,
        closure_key: &StableClosureKey,
        env_ty: StructType<'ctx>,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = private_closure_env_type_desc_name(closure_key);
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let trace_start_offset_bytes = self.target_data.offset_of_element(&env_ty, 1).unwrap_or(0);
        let canonical_name = closure_key.env_canonical_name();
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: &canonical_name,
            obj_ty: env_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn get_or_create_string_type_desc_global(
        &mut self,
        _at: crate::span::Span,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        // String 类型描述符由 C runtime 唯一定义（见
        // `runtime/c/scoop_runtime.c::__scoop_type_desc_runtime__ScoopString`）。
        // codegen 端只声明 extern：字面量分配（写入对象头）与 `as? String`
        // 检查（沿 parent 链 pointer-equality）必须用同一指针，否则 cast 会
        // 因为指针地址不同而误判 fail。
        const GLOBAL_NAME: &str = "__scoop_type_desc_runtime__ScoopString";
        if let Some(existing) = self.module.get_global(GLOBAL_NAME) {
            return Ok(existing);
        }

        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let gv = self.module.add_global(desc_ty, None, GLOBAL_NAME);
        gv.set_constant(true);
        gv.set_linkage(Linkage::External);
        Ok(gv)
    }

    pub(super) fn llvm_boxed_unit_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.BoxedUnit";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into()], false);
        ty
    }

    pub(super) fn get_or_create_boxed_unit_type_desc_global(
        &mut self,
        at: crate::span::Span,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        const GLOBAL_NAME: &str = "__scoop_type_desc_runtime__BoxedUnit";
        if let Some(existing) = self.module.get_global(GLOBAL_NAME) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_boxed_unit_type();
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: GLOBAL_NAME,
            type_id_key: "scoop.runtime.BoxedUnit",
            obj_ty,
            trace_start_offset_bytes: 0,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn get_or_create_boxed_int_type_desc_global(
        &mut self,
        at: crate::span::Span,
        payload: IntTy,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!(
            "__scoop_type_desc_runtime__boxed_int{}_{}",
            payload.bits,
            if payload.signed { "i" } else { "u" }
        );
        if let Some(existing) = self.module.get_global(&global_name) {
            return Ok(existing);
        }

        let obj_ty = self.llvm_boxed_int_type(payload);
        let trace_start_offset_bytes = self.target_data.offset_of_element(&obj_ty, 1).unwrap_or(0);
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: &format!(
                "scoop.runtime.BoxedInt{}_{}",
                payload.bits,
                if payload.signed { "i" } else { "u" }
            ),
            obj_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn llvm_boxed_int_type(&self, payload: IntTy) -> StructType<'ctx> {
        // 说明：box 类型目前只用于 `Int/UInt/... -> Any` 的最小装箱（T0817）。
        // 未来会扩展为统一的对象头 + type descriptor（T0907+）；当前已接入最小对象头（T0908）。
        let name = format!(
            "scoop.runtime.BoxedInt{}_{}",
            payload.bits,
            if payload.signed { "i" } else { "u" }
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        // `{ ScoopGcObjectHeader header, <int> payload }`
        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), self.int_type(payload).into()], false);
        ty
    }

    pub(super) fn llvm_closure_object_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型是 early stage 的函数值/闭包运行期表示（T0710/T1307b）。
        // - env 指针指向一个 GC-managed 的 closure env heap object（无捕获时为 NULL）。
        //
        // 布局（与 GC 对象头兼容）：
        // `{ header: ScoopGcObjectHeader, env_ptr: i8 addrspace(1)*, fn_ptr: i8* }`
        const TY_NAME: &str = "scoop.runtime.ScoopClosure";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        ty.set_body(
            &[header_ty.into(), gc_i8_ptr_ty.into(), i8_ptr_ty.into()],
            false,
        );
        ty
    }

    pub(super) fn store_local_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 说明：当前阶段 locals 允许：
        // - 标量：`Unit/Bool/Int*`
        // - struct/enum（值类型）：以 LLVM struct by-value 形式存入栈 slot（`alloca`）
        let v = self.coerce_value(at, value, ty)?;
        self.store_local_value_exact(at, ptr, ty, v)
    }

    pub(super) fn store_local_value_exact(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // safepoint 是 GC ref 的 clobber 边界：若 `ptr` 由某个可被 moving GC 更新的
        // base 指针（例如 explicit-frame-backed 的 heap frame ptr load）推导而来，
        // 则不能继续信任旧 SSA/GEP；在真正写入前需要在当前 block 里重建指针链。
        let ptr = self.rematerialize_ptr_in_current_block(at, ptr, "store_local_slot")?;

        if value.ty != ty && value.ty != CgTy::Never {
            panic!("store_local_value_exact: verifier accepted store value type drift");
        }

        match ty {
            // T1612: Nothing/Never has no runtime value; storing is a no-op (unreachable path).
            CgTy::Never => return Ok(CgValue::never()),
            CgTy::Unit => {
                let zero = self.context.i8_type().const_int(0, false);
                let _ = self.builder.build_store(ptr, zero)?;
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_) => {
                let raw = self.expect_cg_value(value, "store_local_value_exact scalar/aggregate");
                // T1412d：当写入目标位于 GC heap（addrspace(1)）且写入值为 GC ref 时，
                // 必须走统一写屏障 hook，避免形成 old→nursery 指针（minor GC 的关键前置条件）。
                //
                // 注意：locals/alloca 在 addrspace(0)，因此不会触发该分支。
                if ptr.get_type().get_address_space() == self.gc_address_space()
                    && needs_write_barrier_for_value_ty(self, at, ty)?
                {
                    let value_ptr = self.expect_pointer_value(raw, "write barrier value");

                    self.store_gc_pointer_slot_with_write_barrier(at, ptr, value_ptr)?;
                } else {
                    let store_inst = self.builder.build_store(ptr, raw)?;
                    // T0119: `@CLayout(packed = N)` — aggregate store 到 alloca 时，
                    // store alignment 降到 packed value（与 load 路径保持一致）。
                    // packed=1 时 alignment=1，packed>1 时 alignment=min(struct_natural, N)。
                    if let CgTy::Struct(struct_ty) = ty
                        && let Some(pack_n) = self.struct_clayout(struct_ty).and_then(|c| c.packed)
                    {
                        // For whole-aggregate store, use pack_n as alignment
                        // (the struct is packed, so its overall alignment is at most pack_n).
                        store_inst.set_alignment(pack_n)?;
                    }
                }
                if ptr.get_type().get_address_space() == AddressSpace::default() {
                    let storage_ty = self.llvm_basic_type_of(at, ty)?;
                    self.sync_basic_value_into_explicit_frame(
                        at,
                        ptr,
                        raw,
                        storage_ty,
                        "store_local",
                    )?;
                }
            }
            CgTy::Enum(enum_ty) => {
                let raw = self.expect_cg_value(value, "store_local_value_exact enum");

                if self.try_store_heap_tagged_union_enum_exact(at, ptr, enum_ty, raw)? {
                    return Ok(value);
                }

                if ptr.get_type().get_address_space() == self.gc_address_space()
                    && needs_write_barrier_for_value_ty(self, at, ty)?
                {
                    let value_ptr = self.expect_pointer_value(raw, "enum write barrier value");

                    self.store_gc_pointer_slot_with_write_barrier(at, ptr, value_ptr)?;
                } else {
                    let _ = self.builder.build_store(ptr, raw)?;
                }
                if ptr.get_type().get_address_space() == AddressSpace::default() {
                    let storage_ty = self.llvm_basic_type_of(at, ty)?;
                    self.sync_basic_value_into_explicit_frame(
                        at,
                        ptr,
                        raw,
                        storage_ty,
                        "store_enum",
                    )?;
                }
            }
        }
        Ok(value)
    }

    pub(super) fn store_gc_pointer_slot_with_write_barrier(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        value_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let wb = self.declare_runtime_gc_write_barrier();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // `slot_addr`：传入“slot 的地址”即可；runtime 用 memcpy 写回（避免 strict alias UB）。
        //
        // 注意：该地址只是 native 指针（C ABI `void*`），不应位于 GC address space；
        // 否则会被 statepoint/stackmap 当作 GC root，产生 derived/non-header roots。
        let slot_addr_i8_gc =
            self.builder
                .build_pointer_cast(ptr, gc_i8_ptr_ty, "gc_wb_slot_addr_i8_gc")?;
        let slot_addr =
            self.builder
                .build_address_space_cast(slot_addr_i8_gc, i8_ptr_ty, "gc_wb_slot_addr")?;
        let value_i8 =
            self.builder
                .build_pointer_cast(value_ptr, gc_i8_ptr_ty, "gc_wb_value_i8")?;

        let _ = self.build_call_preserving_gc_local_roots(
            at,
            wb,
            &[slot_addr.into(), value_i8.into()],
            "gc_write_barrier",
        )?;
        Ok(())
    }

    pub(super) fn promote_gc_pointer_with_write_barrier(
        &mut self,
        at: crate::span::Span,
        value_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let wb = self.declare_runtime_gc_write_barrier();
        let slot_addr = self.llvm_i8_ptr_type().const_null();
        let value_i8 = self.builder.build_pointer_cast(
            value_ptr,
            self.llvm_gc_i8_ptr_type(),
            "gc_wb_value_i8",
        )?;

        let _ = self.build_call_preserving_gc_local_roots(
            at,
            wb,
            &[slot_addr.into(), value_i8.into()],
            "gc_write_barrier",
        )?;
        Ok(())
    }

    fn try_store_heap_tagged_union_enum_exact(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        enum_ty: crate::ty::MonoTypeId,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<bool, LlvmEmitError> {
        if ptr.get_type().get_address_space() != self.gc_address_space() {
            return Ok(false);
        }

        let layout = self.cg_enum_layout(at, enum_ty)?;
        if !matches!(layout.repr, CgEnumRepr::TaggedUnion) {
            return Ok(false);
        }

        let enum_raw = self.expect_struct_value(raw, "tagged union enum heap store");

        let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
        let gc_ptr_slot = self
            .builder
            .build_struct_gep(llvm_enum_ty, ptr, 2, "enum_gc_ptr_gep")?;
        let word_slot =
            self.builder
                .build_struct_gep(llvm_enum_ty, ptr, 1, "enum_payload_word_gep")?;
        let tag_slot = self
            .builder
            .build_struct_gep(llvm_enum_ty, ptr, 0, "enum_tag_gep")?;

        // 先写 GC pointer 槽位：若写屏障内部触发 GC，GC 至少还能通过静态 layout 看到新 payload。
        let gc_ptr = self
            .builder
            .build_extract_value(enum_raw, 2, "enum_payload_ptr")?;
        let gc_ptr = self.expect_pointer_value(gc_ptr, "tagged union enum GC payload field");
        self.store_gc_pointer_slot_with_write_barrier(at, gc_ptr_slot, gc_ptr)?;

        let word = self
            .builder
            .build_extract_value(enum_raw, 1, "enum_payload_word")?;
        let word = self.expect_int_value(word, "tagged union enum word payload field");
        let _ = self.builder.build_store(word_slot, word)?;

        let tag = self.builder.build_extract_value(enum_raw, 0, "enum_tag")?;
        let tag = self.expect_int_value(tag, "tagged union enum tag field");
        let _ = self.builder.build_store(tag_slot, tag)?;

        Ok(true)
    }
}

fn needs_write_barrier_for_value_ty<'a, 'ctx>(
    cg: &mut MainCodegen<'a, 'ctx>,
    at: crate::span::Span,
    ty: CgTy,
) -> Result<bool, LlvmEmitError> {
    // 说明：
    // - `write_barrier(slot, value)` 语义上针对“写入 slot 的 GC-managed 指针”；
    // - 大多数情况下，这对应 `CgTy::Ref/String`；
    // - 但 `Option<Ref>` 这类 enum 可能通过 niche 优化降为“直接用 payload 指针承载 enum 值”，
    //   在 LLVM IR 侧同样表现为 `ptr addrspace(1)`；
    //   若仅按 `CgTy::Ref/String` 判断，会漏掉这类 heap field store，从而在 `--gc-stress` 下出现回归。
    // - 更复杂的 tagged union enum 会在 `store_local_value_exact` 中拆成
    //   `tag/word/gc_ptr` 三槽写回；这里保留“单指针 store”子集。
    match ty {
        CgTy::Ref | CgTy::String => Ok(true),
        CgTy::Enum(enum_ty) => {
            // 仅处理“niche pointer enum，且 payload 是 GC 指针”的子集。
            let layout = cg.cg_enum_layout(at, enum_ty)?;
            match layout.repr {
                CgEnumRepr::Niche {
                    storage: NicheStorage::Pointer,
                    ..
                } => {
                    let some_field_is_gc_ptr = layout
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter())
                        .any(|f| matches!(f, CgTy::Ref | CgTy::String));
                    Ok(some_field_is_gc_ptr)
                }
                _ => Ok(false),
            }
        }
        _ => Ok(false),
    }
}
