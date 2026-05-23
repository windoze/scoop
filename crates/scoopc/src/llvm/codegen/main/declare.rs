//! Top-level function declaration: declare_top_level_fun_*, sret hidden return, native parameter ABI, libc declarations.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
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
