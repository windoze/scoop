//! Top-level function declaration: declare_top_level_fun_*, sret hidden return, native parameter ABI, libc declarations.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn declare_top_level_fun(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let callable_abi = self.direct_call_abi_identity(&fun.fqn);
        let llvm_name = if callable_abi.is_extern() {
            self.extern_funs
                .get(&fun.fqn)
                .map(|e| e.symbol.clone())
                .unwrap_or_else(|| fun.fqn.clone())
        } else {
            self.exported_abi_symbol_for_hir_fun(fun)?
        };
        self.declare_top_level_fun_with_symbol(fun, llvm_name)
    }

    pub(crate) fn declare_top_level_fun_with_symbol(
        &mut self,
        fun: &hir::FunDecl,
        llvm_name: String,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }

        let callable_abi = self.direct_call_abi_identity(&fun.fqn);
        let native_abi = if callable_abi.uses_native_abi() {
            let param_tys = fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
            Some(self.classify_direct_extern_native_callable(
                fun.span,
                &fun.fqn,
                &param_tys,
                fun.return_ty,
            )?)
        } else {
            None
        };

        // `@Extern` 调用点会在进入 native 前把 managed roots 暴露为 `native_roots` slots；
        // 从 LLVM GC/statepoint 的视角看，这些调用必须视作 leaf：
        // - native 内部即使触发 GC，也应以 slots 更新为准；
        // - 不能再依赖 caller frame 上的 SSA `gc.relocate` / stackmap 结果。
        //
        // 历史上我们还会把“返回 GC-free aggregate 的普通函数”标成 leaf，以绕开
        // `gc.result` 不能承载多寄存器 aggregate 的 LLVM 限制；但现在 ordinary path
        // 已统一把 aggregate return 改成 hidden sret，这类函数不应再被视作 leaf，
        // 否则它们内部的 managed calls 会被错误跳过 statepoint rewrite。
        let returns_gc_free_aggregate = self.returns_gc_free_aggregate(fun.return_ty);

        let return_cg = native_abi
            .as_ref()
            .map(|abi| abi.return_abi.cg_ty)
            .or_else(|| self.try_cg_ty_of_type_id(fun.return_ty))
            .unwrap_or_else(|| {
                tracing::warn!(
                    "declare_top_level_fun: unsupported return type for {} -> {}",
                    fun.fqn,
                    self.types.display(fun.return_ty)
                );
                panic!(
                    "declare_top_level_fun: MIR signature verifier accepted unsupported return type"
                )
            });

        let hidden_sret_result_ty = if native_abi.is_some() {
            None
        } else {
            self.hidden_sret_result_ty(fun.span, return_cg)?
        };
        let uses_explicit_effect_hidden_abi = callable_abi.uses_effect_bridge_abi();
        let is_gc_leaf = native_abi
            .as_ref()
            .map(|abi| abi.gc_leaf_function)
            .unwrap_or(returns_gc_free_aggregate && hidden_sret_result_ty.is_none());

        let mut llvm_params = Vec::with_capacity(
            fun.params.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);
        }
        if let Some(native_abi) = native_abi.as_ref() {
            llvm_params.extend(native_abi.param_abis.iter().map(|abi| abi.llvm_param_ty));
        } else {
            for param in &fun.params {
                let llvm_param_ty = self
                    .ordinary_param_abi(param.span, param.ty)
                    .map(OrdinaryParamAbi::llvm_param_ty);
                match llvm_param_ty {
                    Ok(ty) => llvm_params.push(ty),
                    Err(err) => {
                        tracing::warn!(
                            "declare_top_level_fun: unsupported param type for {} param {} -> {}",
                            fun.fqn,
                            param.name,
                            self.types.display(param.ty)
                        );
                        return Err(err);
                    }
                }
            }
        }

        let fn_ty = if let Some(native_abi) = native_abi.as_ref() {
            native_abi.fn_ty
        } else {
            match (hidden_sret_result_ty, return_cg) {
                (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                    self.context.void_type().fn_type(&llvm_params, false)
                }
                (None, other) => self
                    .llvm_basic_type_of(fun.span, other)?
                    .fn_type(&llvm_params, false),
            }
        };

        let llvm_fun = if callable_abi.is_extern() {
            self.declare_runtime_or_native_import_function(&llvm_name, fn_ty)
        } else {
            self.declare_exported_abi_function(&llvm_name, fn_ty)
        };
        // `@CallingConvention(...)`：native surface 仍通过统一 classifier 产出 callconv。
        llvm_fun.set_call_conventions(
            native_abi
                .as_ref()
                .map(|abi| abi.call_convention)
                .unwrap_or_else(|| self.llvm_call_convention_for_fqn(&fun.fqn)),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        if is_gc_leaf {
            self.mark_gc_leaf_function(llvm_fun);
        }
        Ok(llvm_fun)
    }

    pub(crate) fn declare_top_level_fun_with_signature_override(
        &mut self,
        fun: &hir::FunDecl,
        llvm_name: &str,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let callable_abi = self.direct_call_abi_identity(&fun.fqn);
        let native_abi =
            if callable_abi.uses_native_abi() {
                Some(self.classify_direct_extern_native_callable(
                    fun.span, &fun.fqn, param_tys, return_ty,
                )?)
            } else {
                None
            };
        let llvm_name = if callable_abi.is_extern() {
            llvm_name.to_string()
        } else if self.lir_callable_symbol_facts(llvm_name).is_some() {
            self.exported_abi_symbol_for_lir_callable(llvm_name)?
        } else {
            self.exported_abi_symbol_for_hir_fun_with_signature_override(
                fun, llvm_name, param_tys, return_ty,
            )?
        };
        if let Some(existing) = self.module.get_function(&llvm_name) {
            return Ok(existing);
        }
        let returns_gc_free_aggregate = self.returns_gc_free_aggregate(return_ty);

        let return_cg = native_abi
            .as_ref()
            .map(|abi| abi.return_abi.cg_ty)
            .or_else(|| self.try_cg_ty_of_type_id(return_ty))
            .unwrap_or_else(|| {
                panic!("declare_top_level_fun_with_signature_override: MIR signature verifier accepted unsupported return type")
            });
        if param_tys.len() != fun.params.len() {
            panic!(
                "declare_top_level_fun_with_signature_override: MIR signature verifier accepted param arity drift"
            );
        }

        let hidden_sret_result_ty = if native_abi.is_some() {
            None
        } else {
            self.hidden_sret_result_ty(fun.span, return_cg)?
        };
        let uses_explicit_effect_hidden_abi = callable_abi.uses_effect_bridge_abi();
        let is_gc_leaf = native_abi
            .as_ref()
            .map(|abi| abi.gc_leaf_function)
            .unwrap_or(returns_gc_free_aggregate && hidden_sret_result_ty.is_none());

        let mut llvm_params = Vec::with_capacity(
            param_tys.len()
                + usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(uses_explicit_effect_hidden_abi)
                    as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        if uses_explicit_effect_hidden_abi {
            self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);
        }
        if let Some(native_abi) = native_abi.as_ref() {
            llvm_params.extend(native_abi.param_abis.iter().map(|abi| abi.llvm_param_ty));
        } else {
            for (param, param_ty) in fun.params.iter().zip(param_tys.iter().copied()) {
                llvm_params.push(
                    self.ordinary_param_abi(param.span, param_ty)
                        .map(OrdinaryParamAbi::llvm_param_ty)?,
                );
            }
        }

        let fn_ty = if let Some(native_abi) = native_abi.as_ref() {
            native_abi.fn_ty
        } else {
            match (hidden_sret_result_ty, return_cg) {
                (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                    self.context.void_type().fn_type(&llvm_params, false)
                }
                (None, other) => self
                    .llvm_basic_type_of(fun.span, other)?
                    .fn_type(&llvm_params, false),
            }
        };

        let llvm_fun = if callable_abi.is_extern() {
            self.declare_runtime_or_native_import_function(&llvm_name, fn_ty)
        } else {
            self.declare_exported_abi_function(&llvm_name, fn_ty)
        };
        llvm_fun.set_call_conventions(
            native_abi
                .as_ref()
                .map(|abi| abi.call_convention)
                .unwrap_or_else(|| self.llvm_call_convention_for_fqn(&fun.fqn)),
        );
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        if is_gc_leaf {
            self.mark_gc_leaf_function(llvm_fun);
        }
        Ok(llvm_fun)
    }

    pub(in crate::llvm::codegen) fn declare_callee_resume_entry_function(
        &mut self,
        at: crate::span::Span,
        name: &str,
        return_cg: CgTy,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(name) {
            return Ok(existing);
        }

        let hidden_sret_result_ty = self.hidden_sret_result_ty(at, return_cg)?;
        let mut llvm_params = Vec::with_capacity(
            usize::from(hidden_sret_result_ty.is_some())
                + self.explicit_effect_hidden_abi_param_count(true) as usize,
        );
        if hidden_sret_result_ty.is_some() {
            llvm_params.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        self.push_explicit_effect_hidden_abi_param_tys(&mut llvm_params);

        let fn_ty = match (hidden_sret_result_ty, return_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_params, false)
            }
            (None, other) => self
                .llvm_basic_type_of(at, other)?
                .fn_type(&llvm_params, false),
        };
        let llvm_fun =
            self.declare_compiler_private_helper_function(name, fn_ty, Linkage::Internal);
        llvm_fun.set_call_conventions(0);
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
        }
        Ok(llvm_fun)
    }

    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn declare_top_level_fun_callee_resume_entry(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let return_cg = self.try_cg_ty_of_type_id(fun.return_ty).unwrap_or_else(|| {
            panic!("declare_top_level_fun_callee_resume_entry: MIR signature verifier accepted unsupported return type")
        });
        self.declare_callee_resume_entry_function(
            fun.span,
            &top_level_callee_resume_entry_fn_name(&fun.fqn),
            return_cg,
        )
    }

    pub(in crate::llvm::codegen) fn mark_gc_leaf_function(&self, function: FunctionValue<'ctx>) {
        let attr = self.context.create_string_attribute("gc-leaf-function", "");
        function.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
    }

    pub(in crate::llvm::codegen) fn llvm_call_convention_for_fqn(&self, fqn: &str) -> u32 {
        let Some(extern_fun) = self.extern_funs.get(fqn) else {
            return 0;
        };
        if !extern_fun.callable_abi_identity().uses_native_abi() {
            return 0;
        }
        let Some(name) = extern_fun.calling_convention.as_deref() else {
            return 0;
        };

        match name.trim().to_ascii_lowercase().as_str() {
            "c" | "cdecl" => 0,
            // 其它 calling convention 名称留到后续任务再补齐（spec §15.5.4）。
            _ => 0,
        }
    }

    pub(in crate::llvm::codegen) fn llvm_type_needs_sret(ty: BasicTypeEnum<'ctx>) -> bool {
        matches!(
            ty,
            BasicTypeEnum::StructType(_)
                | BasicTypeEnum::ArrayType(_)
                | BasicTypeEnum::VectorType(_)
                | BasicTypeEnum::ScalableVectorType(_)
        )
    }

    pub(in crate::llvm::codegen) fn hidden_sret_result_ty(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
    ) -> Result<Option<BasicTypeEnum<'ctx>>, LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        Ok(Self::llvm_type_needs_sret(llvm_ret_ty).then_some(llvm_ret_ty))
    }

    pub(in crate::llvm::codegen) fn sret_type_attribute(
        &self,
        result_ty: BasicTypeEnum<'ctx>,
    ) -> Attribute {
        let kind_id = Attribute::get_named_enum_kind_id("sret");
        self.context
            .create_type_attribute(kind_id, result_ty.as_any_type_enum())
    }

    pub(in crate::llvm::codegen) fn add_sret_attribute_to_function(
        &self,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        result_ty: BasicTypeEnum<'ctx>,
    ) {
        llvm_fun.add_attribute(
            AttributeLoc::Param(param_index),
            self.sret_type_attribute(result_ty),
        );
    }

    pub(in crate::llvm::codegen) fn add_sret_attribute_to_call(
        &self,
        call_site: CallSiteValue<'ctx>,
        param_index: u32,
        result_ty: BasicTypeEnum<'ctx>,
    ) {
        call_site.add_attribute(
            AttributeLoc::Param(param_index),
            self.sret_type_attribute(result_ty),
        );
    }

    pub(in crate::llvm::codegen) fn track_gc_root_slots_for_spill_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let slot =
            self.rematerialize_ptr_in_current_block(at, slot, &format!("{name_prefix}_slot"))?;
        let mut gc_leaf_slots = Vec::new();
        self.collect_gc_ptr_leaf_slots_in_spill(slot, value_ty, name_prefix, &mut gc_leaf_slots)?;
        let explicit_frame_enabled = self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_some();
        let frame_slots = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec());
        if explicit_frame_enabled && frame_slots.is_none() {
            panic!(
                "track_gc_root_slots_for_spill_slot: explicit frame verifier accepted missing spill slot mirrors"
            );
        }
        let frame_slots = frame_slots.unwrap_or_default();
        if explicit_frame_enabled && frame_slots.len() != gc_leaf_slots.len() {
            panic!(
                "track_gc_root_slots_for_spill_slot: explicit frame verifier accepted spill/frame slot count mismatch"
            );
        }
        for (index, (slot, value_ptr_ty)) in gc_leaf_slots.into_iter().enumerate() {
            let frame_slot = frame_slots.get(index).copied().unwrap_or(slot);
            self.function_cx
                .tracked_gc_root_slots
                .push(TrackedGcRootSlot {
                    slot,
                    value_ptr_ty,
                    frame_slot,
                });
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn sync_hidden_sret_result_roots(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        result_ptr: PointerValue<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        if !self.basic_type_contains_gc_ptrs(at, llvm_ret_ty)? {
            return Ok(());
        }
        self.sync_storage_slot_into_explicit_frame(at, result_ptr, llvm_ret_ty, name_prefix)
    }

    pub(in crate::llvm::codegen) fn load_hidden_sret_result_from_ptr(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        result_ptr: PointerValue<'ctx>,
        name_prefix: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        self.sync_hidden_sret_result_roots(at, ret_cg, result_ptr, name_prefix)?;
        let reload_slot = self.storage_slot_for_use(at, result_ptr, ret_cg, name_prefix)?;
        let loaded = self
            .builder
            .build_load(llvm_ret_ty, reload_slot, "sret_result")?;
        let result = self.cg_value_from_loaded(at, ret_cg, loaded)?;
        self.clear_spill_slot_root_homes(at, result_ptr, llvm_ret_ty, name_prefix)?;
        Ok(result)
    }

    pub(in crate::llvm::codegen) fn defer_direct_call_result(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        call_site: CallSiteValue<'ctx>,
        name: &str,
    ) -> Result<Option<DeferredCgValue<'ctx>>, LlvmEmitError> {
        match ret_cg {
            CgTy::Unit | CgTy::Never => Ok(None),
            _ => {
                let raw = call_site
                    .try_as_basic_value()
                    .basic()
                    .expect("non-void direct call must yield a value");
                let value = self.cg_value_from_loaded(at, ret_cg, raw)?;
                Ok(Some(self.defer_gc_sensitive_cg_value(at, name, value)?))
            }
        }
    }

    pub(in crate::llvm::codegen) fn clear_gc_locals_in_current_scope(
        &mut self,
        at: crate::span::Span,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(scope) = self.function_cx.env.scopes.last() else {
            return Ok(());
        };

        let locals: Vec<CgLocal<'ctx>> = scope.values().copied().collect();
        for local in locals {
            let llvm_ty = self.llvm_basic_type_of(at, local.ty)?;
            if !self.basic_type_contains_gc_ptrs(at, llvm_ty)? {
                continue;
            }
            self.clear_spill_slot_root_homes(at, local.ptr, llvm_ty, name_prefix)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn llvm_call_convention_for_name(&self, name: &str) -> u32 {
        match name.trim().to_ascii_lowercase().as_str() {
            "c" | "cdecl" => 0,
            // 其它 calling convention 名称留到后续任务再补齐（spec §15.5.4）。
            _ => 0,
        }
    }
}
