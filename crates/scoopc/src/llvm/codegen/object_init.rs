//! Object singleton, property access, and init-body lowering split out of `codegen/mod.rs`.

use super::*;
impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn lookup_object_property_by_fqn(
        &self,
        prop_fqn: &str,
    ) -> Option<(&hir::ObjectInit, &hir::ObjectProperty)> {
        let (owner, name) = prop_fqn.rsplit_once('.')?;
        if let Some(obj) = self.object_inits.get(owner)
            && let Some(prop) = obj.properties.get(name)
        {
            return Some((obj, prop));
        }
        self.object_inits.values().find_map(|obj| {
            (obj.fqn.ends_with(owner))
                .then(|| obj.properties.get(name).map(|prop| (obj, prop)))
                .flatten()
        })
    }

    pub(in crate::llvm::codegen) fn codegen_object_property_access(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (object_fqn, prop) = match self.lookup_object_property_by_fqn(prop_fqn) {
            Some((obj, prop)) => (obj.fqn.clone(), prop.clone()),
            None => {
                panic!(
                    "codegen_object_property_access: resolver accepted object property without metadata"
                );
            }
        };

        if !prop.has_init {
            panic!(
                "codegen_object_property_access: verifier accepted object property without initializer"
            );
        }

        let prop_cg = self.expect_cg_ty_of(prop.ty, "object property type");

        let init_fn = self.ensure_object_init_function_defined(&object_fqn)?;
        self.with_conservative_gc_local_root_spills(at, |cg| {
            let _ = cg.builder.build_call(init_fn, &[], "obj_init")?;
            Ok(())
        })?;

        if prop_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_object_property_global(at, prop_fqn, prop_cg)? else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_obj_prop")?;
        self.cg_value_from_loaded(at, prop_cg, loaded)
    }

    pub(in crate::llvm::codegen) fn load_initialized_object_property_value(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some((_obj, prop)) = self.lookup_object_property_by_fqn(prop_fqn) else {
            panic!(
                "load_initialized_object_property_value: resolver accepted object property without metadata"
            );
        };
        let prop_cg = self.expect_cg_ty_of(prop.ty, "initialized object property type");
        if prop_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }
        let Some(global) = self.declare_object_property_global(at, prop_fqn, prop_cg)? else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_obj_prop")?;
        self.cg_value_from_loaded(at, prop_cg, loaded)
    }

    pub(in crate::llvm::codegen) fn ensure_object_init_bridge_defined(
        &mut self,
        object_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(obj) = self.object_inits.get(object_fqn) else {
            panic!(
                "ensure_object_init_bridge_defined: verifier accepted missing object init metadata"
            );
        };

        let stable_key = self.stable_object_init_key(object_fqn);
        let name = private_object_init_bridge_fn_name(&stable_key);
        let fn_ty = self.llvm_effect_outcome_struct_type().fn_type(&[], false);
        let llvm_fun =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);

        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        let saved_block = self.builder.get_insert_block();

        let mut bridge_codegen = self.fresh_child_codegen();
        bridge_codegen.codegen_object_init_bridge_body(obj, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    fn codegen_object_init_bridge_body(
        &mut self,
        obj: &hir::ObjectInit,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = obj
            .steps
            .first()
            .map(|step| match step {
                hir::ObjectInitStep::PropertyInit { init, .. } => init.span,
                hir::ObjectInitStep::InitBlock { block } => block.span,
            })
            .unwrap_or(crate::span::Span::new(0, 0));
        self.current_source_id = self.source_id_for_path(obj.source_path.as_path(), err_span)?;

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;

        let init_fn = self.ensure_object_init_function_defined(&obj.fqn)?;
        self.with_conservative_gc_local_root_spills(err_span, |cg| {
            let _ = cg.builder.build_call(init_fn, &[], "object_init")?;
            Ok(())
        })?;
        let outcome = self.build_zero_complete_effect_outcome()?;
        self.builder.build_return(Some(&outcome))?;
        self.finish_function_explicit_frame_layout(err_span)?;
        Ok(())
    }

    fn ensure_object_init_function_defined(
        &mut self,
        object_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(obj) = self.object_inits.get(object_fqn) else {
            panic!(
                "ensure_object_init_function_defined: verifier accepted missing object init metadata"
            );
        };

        let stable_key = self.stable_object_init_key(object_fqn);
        let name = private_object_init_fn_name(&stable_key);
        let fn_ty = self.context.void_type().fn_type(&[], false);

        let llvm_fun =
            self.declare_compiler_private_helper_function(&name, fn_ty, Linkage::Internal);

        // 已有 body：无需重复生成。
        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        // 在生成 init function body 时，临时切换 builder 的插入点；结束后恢复到调用方位置。
        let saved_block = self.builder.get_insert_block();

        let mut init_codegen = self.fresh_child_codegen();
        init_codegen.codegen_object_init_fun_body(obj, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    fn codegen_object_init_fun_body(
        &mut self,
        obj: &hir::ObjectInit,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = obj
            .steps
            .first()
            .map(|step| match step {
                hir::ObjectInitStep::PropertyInit { init, .. } => init.span,
                hir::ObjectInitStep::InitBlock { block } => block.span,
            })
            .unwrap_or(crate::span::Span::new(0, 0));

        self.current_source_id = self.source_id_for_path(obj.source_path.as_path(), err_span)?;
        let stable_key = self.stable_object_init_key(&obj.fqn);
        self.enter_root_callable_identity(private_object_init_fn_name(&stable_key), stable_key);

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(llvm_fun)?;
        // object init 是一个内部 `void` 函数：设置 current_fun_return_ty 以便 codegen_return_stmt 使用正确的返回类型。
        self.function_cx.current_fun_return_ty = Some(CgTy::Unit);

        let guard = self.declare_object_init_guard(&obj.fqn);
        let once_begin = self.declare_runtime_once_begin();
        let call = self.builder.build_call(
            once_begin,
            &[guard.as_pointer_value().into()],
            "once_begin",
        )?;
        let ret = self.expect_basic_value(call, "object init once begin");
        let should_init = self.expect_int_value(ret, "object init once begin");
        let i32_ty = self.context.i32_type();
        let cond = self.builder.build_int_compare(
            IntPredicate::NE,
            should_init,
            i32_ty.const_int(0, false),
            "should_init",
        )?;
        self.builder
            .build_conditional_branch(cond, init_bb, done_bb)?;

        self.builder.position_at_end(init_bb);

        // object 单例值本身按 `Ref` ABI 表示为一个真正的 GC heap object；
        // properties 仍保存在独立的全局槽里，因此这里的实例对象只承载 header/type-desc 身份。
        let _ = self.allocate_object_singleton_instance(err_span, &obj.fqn)?;
        let instance_global = self.declare_object_instance_global(&obj.fqn);
        let instance_name =
            private_object_instance_global_name(&self.stable_object_init_key(&obj.fqn));
        self.register_global_root_if_needed(
            err_span,
            instance_global,
            &instance_name,
            self.llvm_gc_i8_ptr_type().into(),
        )?;

        self.function_cx.env.push_scope();
        for step in &obj.steps {
            match step {
                hir::ObjectInitStep::PropertyInit { name, init } => {
                    let Some(prop) = obj.properties.get(name) else {
                        panic!(
                            "codegen_object_init_fun_body: verifier accepted object property init without property metadata"
                        );
                    };

                    let prop_cg = self.expect_cg_ty_of(prop.ty, "object property init type");

                    let v = self.codegen_expr_in_expected_context(init, Some(prop_cg))?;

                    // Unit：只执行副作用即可，无需 backing storage。
                    if prop_cg != CgTy::Unit {
                        let prop_fqn = format!("{}.{}", obj.fqn, name);
                        let Some(global) =
                            self.declare_object_property_global(init.span, &prop_fqn, prop_cg)?
                        else {
                            continue;
                        };
                        let _ = self.store_local_value(
                            init.span,
                            global.as_pointer_value(),
                            prop_cg,
                            v,
                        )?;
                        let storage_ty = self.llvm_basic_type_of(init.span, prop_cg)?;
                        let global_name = private_object_prop_global_name(
                            &self.stable_object_property_key(&prop_fqn),
                        );
                        self.register_global_root_if_needed(
                            init.span,
                            global,
                            &global_name,
                            storage_ty,
                        )?;
                    }
                }
                hir::ObjectInitStep::InitBlock { block } => {
                    let _ = self.codegen_block_value(block)?;
                }
            }
        }
        self.function_cx.env.pop_scope();

        let once_end = self.declare_runtime_once_end();
        let _ =
            self.builder
                .build_call(once_end, &[guard.as_pointer_value().into()], "once_end")?;
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        self.builder.build_return(None)?;
        self.finish_function_explicit_frame_layout(err_span)?;
        Ok(())
    }

    fn declare_object_init_guard(&self, object_fqn: &str) -> GlobalValue<'ctx> {
        let stable_key = self.stable_object_init_key(object_fqn);
        let name = private_object_guard_global_name(&stable_key);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        // 说明：
        // - 该 guard 由 object-only runtime `scoop_once_begin/end` 维护；
        // - 布局约定：单个 `uint64_t` word（低 2 bit 状态 + 其余 bit 为 owner thread id）。
        let gv = self.module.add_global(self.context.i64_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.i64_type().const_int(0, false));
        gv
    }

    fn declare_object_instance_global(&self, object_fqn: &str) -> GlobalValue<'ctx> {
        let stable_key = self.stable_object_init_key(object_fqn);
        let name = private_object_instance_global_name(&stable_key);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        // object 单例实例存放在一个 module-local 全局槽里：
        // - 槽本身位于默认地址空间，便于普通 LLVM global 存取；
        // - 槽里保存的值是 `ptr addrspace(1)`，与 `CgTy::Ref` ABI 对齐。
        let gv = self
            .module
            .add_global(self.llvm_gc_i8_ptr_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.llvm_gc_i8_ptr_type().const_null());
        gv
    }

    fn allocate_object_singleton_instance(
        &mut self,
        at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let obj_ty = self.llvm_object_singleton_type(object_fqn);
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        let desc = self.get_or_create_object_singleton_type_desc_global(at, object_fqn)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "object_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.build_call_preserving_gc_local_roots(
            at,
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_object",
        )?;
        let raw = self.expect_basic_value(call, "scoop_alloc_typed object allocation");
        let obj_ptr = self.expect_pointer_value(raw, "scoop_alloc_typed object allocation");

        let instance_global = self.declare_object_instance_global(object_fqn);
        let _ = self
            .builder
            .build_store(instance_global.as_pointer_value(), obj_ptr)?;
        Ok(obj_ptr)
    }

    pub(in crate::llvm::codegen) fn codegen_object_value_access(
        &mut self,
        at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let init_fn = self.ensure_object_init_function_defined(object_fqn)?;
        self.with_conservative_gc_local_root_spills(at, |cg| {
            let _ = cg.builder.build_call(init_fn, &[], "obj_init")?;
            Ok(())
        })?;

        let instance = self.declare_object_instance_global(object_fqn);
        let loaded = self.builder.build_load(
            self.llvm_gc_i8_ptr_type(),
            instance.as_pointer_value(),
            "load_object_instance",
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(loaded),
        })
    }

    pub(in crate::llvm::codegen) fn load_initialized_object_value(
        &mut self,
        _at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let instance = self.declare_object_instance_global(object_fqn);
        let loaded = self.builder.build_load(
            self.llvm_gc_i8_ptr_type(),
            instance.as_pointer_value(),
            "load_object_instance",
        )?;
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(loaded),
        })
    }

    fn stable_object_init_key(&self, object_fqn: &str) -> StableDefKey {
        if let Some(obj) = self.object_inits.get(object_fqn) {
            return self.stable_def_key_for_source_path(
                obj.source_path.as_path(),
                StableDefNamespace::ObjectInit,
                object_fqn,
                "object_init",
            );
        }
        self.stable_def_key_for_current_cone(
            StableDefNamespace::ObjectInit,
            object_fqn,
            "object_init",
        )
    }

    fn stable_object_property_key(&self, prop_fqn: &str) -> StableDefKey {
        if let Some((object_fqn, _)) = prop_fqn.rsplit_once('.')
            && let Some(obj) = self.object_inits.get(object_fqn)
        {
            return self.stable_def_key_for_source_path(
                obj.source_path.as_path(),
                StableDefNamespace::Value,
                prop_fqn,
                "object_property",
            );
        }
        self.stable_def_key_for_current_cone(StableDefNamespace::Value, prop_fqn, "object_property")
    }

    fn declare_object_property_global(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
        prop_cg: CgTy,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if prop_cg == CgTy::Unit {
            return Ok(None);
        }

        let stable_key = self.stable_object_property_key(prop_fqn);
        let name = private_object_prop_global_name(&stable_key);
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(Some(existing));
        }

        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        let init: BasicValueEnum<'ctx> = match llvm_ty {
            BasicTypeEnum::IntType(ty) => BasicValueEnum::IntValue(ty.const_int(0, false)),
            BasicTypeEnum::PointerType(ty) => BasicValueEnum::PointerValue(ty.const_null()),
            BasicTypeEnum::StructType(ty) => BasicValueEnum::StructValue(ty.const_zero()),
            BasicTypeEnum::ArrayType(ty) => BasicValueEnum::ArrayValue(ty.const_zero()),
            BasicTypeEnum::FloatType(ty) => BasicValueEnum::FloatValue(ty.const_float(0.0)),
            BasicTypeEnum::VectorType(ty) => BasicValueEnum::VectorValue(ty.const_zero()),
            BasicTypeEnum::ScalableVectorType(ty) => {
                BasicValueEnum::ScalableVectorValue(ty.const_zero())
            }
        };
        gv.set_initializer(&init);
        Ok(Some(gv))
    }
}

fn private_object_init_fn_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("object_init", stable_key)
}

fn private_object_init_bridge_fn_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("hidden_object_init_bridge", stable_key)
}

fn private_object_guard_global_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("object_guard", stable_key)
}

fn private_object_instance_global_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("object_instance", stable_key)
}

fn private_object_prop_global_name(stable_key: &StableDefKey) -> String {
    PrivateSymbolMangler.mangle("object_prop", stable_key)
}
