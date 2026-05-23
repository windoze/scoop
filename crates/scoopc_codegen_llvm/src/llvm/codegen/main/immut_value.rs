//! Top-level immutable values: eager-init guard, init function, code-gen access;
//! extern global access; initializer expr lowering.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_initializer_expr(
        &mut self,
        expr: &hir::Expr,
        target_ty: CgTy,
        target_hir_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Closure(closure) => {
                self.codegen_closure_expr(expr.span, closure, target_hir_ty)
            }
            // 对 call initializer 传入声明类型，避免泛型 ctor 等路径因为 HIR `expr.ty = Any`
            // 丢失结果类型信息。
            hir::ExprKind::Call { callee, args } => self.codegen_call(
                expr.span,
                callee,
                args,
                Some(target_ty),
                Some(target_hir_ty),
            ),
            _ => self.codegen_expr_in_expected_context(expr, Some(target_ty)),
        }
    }

    pub(in crate::llvm::codegen) fn codegen_decl_initializer_expr(
        &mut self,
        decl: &hir::ValDecl,
        target_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let init = decl.init.as_ref().unwrap_or_else(|| {
            panic!("codegen_decl_initializer_expr: typed HIR val must publish initializer")
        });

        self.codegen_initializer_expr(init, target_ty, decl.ty)
    }

    pub(in crate::llvm::codegen) fn declare_top_level_immutable_value_guard(
        &self,
        value_fqn: &str,
    ) -> GlobalValue<'ctx> {
        let root = self.expect_lir_global_root_kind(
            value_fqn,
            LirGlobalRootKind::TopLevelImmutableVal,
            "declare_top_level_immutable_value_guard",
        );
        let name = private_top_level_immutable_value_guard_global_name(
            &self.stable_def_key_for_lir_global_root(
                root,
                StableDefNamespace::TopLevelInit,
                "top_level_init",
            ),
        );
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        let gv = self.module.add_global(self.context.i64_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.i64_type().const_int(0, false));
        gv
    }

    pub(in crate::llvm::codegen) fn emit_top_level_immutable_value_initialized_check(
        &mut self,
        at: crate::span::Span,
        value_fqn: &str,
    ) -> Result<(), LlvmEmitError> {
        let func = self.expect_current_function("top-level immutable initialized check");

        let ready_bb = self.context.append_basic_block(func, "top_level_val_ready");
        let recursive_bb = self
            .context
            .append_basic_block(func, "top_level_val_recursive");

        let guard = self.declare_top_level_immutable_value_guard(value_fqn);
        let guard_word = self
            .builder
            .build_load(
                self.context.i64_type(),
                guard.as_pointer_value(),
                "top_level_val_guard_word",
            )?
            .into_int_value();
        let state_mask = self.context.i64_type().const_int(0x3, false);
        let guard_state =
            self.builder
                .build_and(guard_word, state_mask, "top_level_val_guard_state")?;
        let initialized_state = self.context.i64_type().const_int(2, false);
        let is_initialized = self.builder.build_int_compare(
            IntPredicate::EQ,
            guard_state,
            initialized_state,
            "top_level_val_guard_is_initialized",
        )?;
        self.builder
            .build_conditional_branch(is_initialized, ready_bb, recursive_bb)?;

        self.builder.position_at_end(recursive_bb);
        // Eager init 必须已把 guard 推进到 initialized；否则访问会把“尚未完成初始化”
        // 的零值伪装成合法结果。这里直接终止，阻止递归初始化落成静默错误值。
        self.emit_exit_with_code(at, 1)?;

        self.builder.position_at_end(ready_bb);
        Ok(())
    }

    pub(in crate::llvm::codegen) fn declare_top_level_immutable_value_global(
        &mut self,
        at: crate::span::Span,
        value: &hir::TopLevelImmutableValue,
        value_cg: CgTy,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if value_cg == CgTy::Unit {
            return Ok(None);
        }

        let name = private_top_level_immutable_value_global_name(
            &self.stable_def_key_for_lir_global_root(
                self.expect_lir_global_root_kind(
                    &value.fqn,
                    LirGlobalRootKind::TopLevelImmutableVal,
                    "declare_top_level_immutable_value_global",
                ),
                StableDefNamespace::Value,
                "top_level_value",
            ),
        );
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(Some(existing));
        }

        let llvm_ty = self.llvm_basic_type_of(at, value_cg)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.zero_initializer_for_basic_type(llvm_ty));

        if let CgTy::Struct(struct_ty) = value_cg
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
        }

        Ok(Some(gv))
    }

    pub(in crate::llvm::codegen) fn ensure_top_level_immutable_value_init_function_defined(
        &mut self,
        value_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(value) = self.top_level_immutable_values.get(value_fqn) else {
            panic!(
                "ensure_top_level_immutable_value_init_function_defined: verifier accepted missing top-level immutable metadata"
            );
        };

        let name = private_top_level_immutable_value_init_fn_name(
            &self.stable_def_key_for_lir_global_root(
                self.expect_lir_global_root_kind(
                    value_fqn,
                    LirGlobalRootKind::TopLevelImmutableVal,
                    "ensure_top_level_immutable_value_init_function_defined",
                ),
                StableDefNamespace::TopLevelInit,
                "top_level_init",
            ),
        );
        let fn_ty = self.context.void_type().fn_type(&[], false);
        let llvm_fun =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);

        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        let saved_block = self.builder.get_insert_block();

        let mut init_codegen = self.fresh_child_codegen();
        init_codegen.codegen_top_level_immutable_value_init_fun_body(value, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn ensure_top_level_immutable_value_init_bridge_defined(
        &mut self,
        value_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(value) = self.top_level_immutable_values.get(value_fqn) else {
            panic!(
                "ensure_top_level_immutable_value_init_bridge_defined: verifier accepted missing top-level immutable metadata"
            );
        };

        let name = private_top_level_immutable_value_init_bridge_fn_name(
            &self.stable_def_key_for_lir_global_root(
                self.expect_lir_global_root_kind(
                    value_fqn,
                    LirGlobalRootKind::TopLevelImmutableVal,
                    "ensure_top_level_immutable_value_init_bridge_defined",
                ),
                StableDefNamespace::TopLevelInit,
                "top_level_init",
            ),
        );
        let fn_ty = self.llvm_effect_outcome_struct_type().fn_type(&[], false);
        let llvm_fun =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);

        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        let saved_block = self.builder.get_insert_block();

        let mut bridge_codegen = self.fresh_child_codegen();
        bridge_codegen.codegen_top_level_immutable_value_init_bridge_body(value, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_immutable_value_init_bridge_body(
        &mut self,
        value: &hir::TopLevelImmutableValue,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = value
            .init
            .as_ref()
            .map(|init| init.span)
            .unwrap_or(value.span);
        self.current_source_id = self.source_id_for_path(value.source_path.as_path(), err_span)?;

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;

        let outcome = self.build_zero_complete_effect_outcome()?;
        self.builder.build_return(Some(&outcome))?;
        self.finish_function_explicit_frame_layout(err_span)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_immutable_value_init_fun_body(
        &mut self,
        value: &hir::TopLevelImmutableValue,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = value
            .init
            .as_ref()
            .map(|init| init.span)
            .unwrap_or(value.span);
        self.current_source_id = self.source_id_for_path(value.source_path.as_path(), err_span)?;
        let stable_key = self.stable_def_key_for_lir_global_root(
            self.expect_lir_global_root_kind(
                &value.fqn,
                LirGlobalRootKind::TopLevelImmutableVal,
                "codegen_top_level_immutable_value_init_fun_body",
            ),
            StableDefNamespace::TopLevelInit,
            "top_level_init",
        );
        self.enter_root_callable_identity(
            private_top_level_immutable_value_init_fn_name(&stable_key),
            stable_key,
        );

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let recursive_bb = self.context.append_basic_block(llvm_fun, "recursive");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;
        self.function_cx.current_fun_return_ty = Some(CgTy::Unit);

        let guard = self.declare_top_level_immutable_value_guard(&value.fqn);
        let i64_ty = self.context.i64_type();
        let guard_word = self
            .builder
            .build_load(i64_ty, guard.as_pointer_value(), "top_level_val_init_guard")?
            .into_int_value();
        let state_mask = i64_ty.const_int(0x3, false);
        let guard_state =
            self.builder
                .build_and(guard_word, state_mask, "top_level_val_init_state")?;
        let initialized_state = i64_ty.const_int(2, false);
        let is_initialized = self.builder.build_int_compare(
            IntPredicate::EQ,
            guard_state,
            initialized_state,
            "top_level_val_already_initialized",
        )?;
        let check_recursive_bb = self.context.append_basic_block(llvm_fun, "check_recursive");
        self.builder
            .build_conditional_branch(is_initialized, done_bb, check_recursive_bb)?;

        self.builder.position_at_end(check_recursive_bb);
        let initializing_state = i64_ty.const_int(1, false);
        let is_initializing = self.builder.build_int_compare(
            IntPredicate::EQ,
            guard_state,
            initializing_state,
            "top_level_val_is_initializing",
        )?;
        self.builder
            .build_conditional_branch(is_initializing, recursive_bb, init_bb)?;

        self.builder.position_at_end(recursive_bb);
        self.emit_exit_with_code(err_span, 1)?;

        self.builder.position_at_end(init_bb);
        self.builder
            .build_store(guard.as_pointer_value(), initializing_state)?;

        let init = value
            .init
            .as_ref()
            .unwrap_or_else(|| panic!("codegen_top_level_immutable_value_init_fun_body: verifier accepted immutable value without initializer"));
        let value_cg = self.cg_ty_of_type_id(value.ty, "top-level immutable value type");
        let init_value = self.codegen_initializer_expr(init, value_cg, value.ty)?;
        if let Some(global) =
            self.declare_top_level_immutable_value_global(init.span, value, value_cg)?
        {
            let _stored =
                self.store_local_value(init.span, global.as_pointer_value(), value_cg, init_value)?;
            let storage_ty = self.llvm_basic_type_of(init.span, value_cg)?;
            let global_name = private_top_level_immutable_value_global_name(
                &self.stable_def_key_for_lir_global_root(
                    self.expect_lir_global_root_kind(
                        &value.fqn,
                        LirGlobalRootKind::TopLevelImmutableVal,
                        "codegen_top_level_immutable_value_init_fun_body",
                    ),
                    StableDefNamespace::Value,
                    "top_level_value",
                ),
            );
            self.register_global_root_if_needed(init.span, global, &global_name, storage_ty)?;
        }

        self.builder
            .build_store(guard.as_pointer_value(), initialized_state)?;
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        self.builder.build_return(None)?;
        self.finish_function_explicit_frame_layout(err_span)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_immutable_value_access(
        &mut self,
        at: crate::span::Span,
        value: &hir::TopLevelImmutableValue,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_cg = self.cg_ty_of_type_id(value.ty, "top-level immutable value access type");
        self.emit_top_level_immutable_value_initialized_check(at, &value.fqn)?;

        if value_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_top_level_immutable_value_global(at, value, value_cg)?
        else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, value_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_top_level_val")?;
        self.cg_value_from_loaded(at, value_cg, loaded)
    }

    pub(in crate::llvm::codegen) fn load_initialized_top_level_immutable_value(
        &mut self,
        at: crate::span::Span,
        value: &hir::TopLevelImmutableValue,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_cg =
            self.cg_ty_of_type_id(value.ty, "top-level immutable initialized value type");
        self.emit_top_level_immutable_value_initialized_check(at, &value.fqn)?;

        if value_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_top_level_immutable_value_global(at, value, value_cg)?
        else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, value_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_top_level_val")?;
        self.cg_value_from_loaded(at, value_cg, loaded)
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_value_ref(
        &mut self,
        span: crate::span::Span,
        fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T1311：object/companion object 单例值在表达式位置可用：
        // - 读取单例值应触发一次初始化（init block / 属性 init）；
        // - 运行期用一个 module-local 的唯一地址作为"单例实例指针"（ref type）。
        if self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ObjectSingleton) {
            return self.codegen_object_value_access(span, fqn);
        }

        if self.lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelImmutableVal) {
            let value = self.top_level_immutable_values.get(fqn).cloned().unwrap_or_else(|| {
                panic!(
                    "codegen_top_level_value_ref: LIR facts immutable root `{fqn}` is missing body scaffold"
                )
            });
            return self.codegen_top_level_immutable_value_access(span, &value);
        }

        if self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ExternGlobal) {
            let root = self
                .expect_lir_global_root_kind(
                    fqn,
                    LirGlobalRootKind::ExternGlobal,
                    "codegen_top_level_value_ref",
                )
                .clone();
            return self.codegen_lir_extern_global_access(span, &root);
        }

        // T1023：`@ThreadLocal/@Global var` 顶层可变变量。
        if !self.lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelMutableVar) {
            panic!(
                "codegen_top_level_value_ref: resolver accepted unknown top-level value `{fqn}`"
            );
        }
        let root = self
            .expect_lir_global_root_kind(
                fqn,
                LirGlobalRootKind::TopLevelMutableVar,
                "codegen_top_level_value_ref",
            )
            .clone();

        let cg_ty = self.cg_ty_of_type_id(
            self.lir_global_root_ty(&root, "top-level value ref var type"),
            "top-level value ref var type",
        );

        if cg_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let gv = self.declare_lir_top_level_var_global(&root)?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, gv.as_pointer_value(), "load_top_level_var")?;

        Ok(match cg_ty {
            CgTy::Bool => CgValue::bool(loaded.into_int_value()),
            CgTy::Float64 | CgTy::Float32 => CgValue::float(loaded.into_float_value(), cg_ty),
            CgTy::Int(int_ty) => CgValue::int(loaded.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(loaded.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(loaded.into_pointer_value().into()),
            },
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => CgValue {
                ty: cg_ty,
                value: Some(loaded),
            },
            CgTy::Unit => CgValue::unit(),
            CgTy::Never => CgValue::never(),
        })
    }

    pub(in crate::llvm::codegen) fn codegen_lir_extern_global_access(
        &mut self,
        span: crate::span::Span,
        root: &LirGlobalRootFacts,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let cg_ty = self.cg_ty_of_type_id(
            self.lir_global_root_ty(root, "extern global access type"),
            "extern global access type",
        );
        if cg_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let gv = self.declare_lir_extern_global(root)?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, gv.as_pointer_value(), "load_extern_global")?;
        self.cg_value_from_loaded(span, cg_ty, loaded)
    }

    pub(in crate::llvm::codegen) fn register_global_root_if_needed(
        &mut self,
        at: crate::span::Span,
        global: GlobalValue<'ctx>,
        global_name: &str,
        storage_ty: BasicTypeEnum<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(type_desc) =
            self.get_or_create_global_root_type_desc_global(at, global_name, storage_ty)?
        else {
            return Ok(());
        };

        let rt_register = self.declare_runtime_gc_register_global_root();
        let _ = self.builder.build_call(
            rt_register,
            &[
                global.as_pointer_value().into(),
                type_desc.as_pointer_value().into(),
            ],
            "gc_register_global_root",
        )?;
        Ok(())
    }
}
