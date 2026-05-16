//! GC root slot management plus refactor-specific GC object allocation, type-descriptor materialization, payload zeroing, and the GC-aware value/basic-value store helpers used by the body lowerer.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(super) fn build_volatile_refactor_gc_root_store(
        &mut self,
        slot: PointerValue<'ctx>,
        value: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let store = self.builder.build_store(slot, value)?;
        store.set_volatile(true).map_err(|err| {
            frontend_error(format!("refactor GC root store 无法标记 volatile: {err}"))
        })?;
        Ok(())
    }

    pub(super) fn build_volatile_refactor_gc_root_load(
        &mut self,
        slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let loaded =
            self.builder
                .build_load(self.llvm_gc_i8_ptr_type(), slot, &format!("{name}_reload"))?;
        let Some(inst) = loaded.as_instruction_value() else {
            return Err(frontend_error(format!(
                "refactor GC root load `{name}` 缺少 instruction value"
            )));
        };
        inst.set_volatile(true).map_err(|err| {
            frontend_error(format!("refactor GC root load 无法标记 volatile: {err}"))
        })?;
        Ok(loaded.into_pointer_value())
    }

    pub(super) fn refactor_gc_root_explicit_frame_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<Option<PointerValue<'ctx>>, LlvmEmitError> {
        self.explicit_frame_single_gc_ptr_reload_slot_for_storage_slot(
            at,
            slot,
            self.llvm_gc_i8_ptr_type().into(),
            name,
        )
    }

    pub(super) fn create_refactor_gc_root_slot(
        &mut self,
        at: crate::span::Span,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();
        let slot = self.create_entry_alloca_raw(at, name, gc_ptr_ty.into())?;
        if let Some(frame_slot) = self.refactor_gc_root_explicit_frame_slot(at, slot, name)? {
            // In explicit-frame mode the mirror slot is the authoritative root home. Keep
            // compiler-generated refactor root slots out of a second stack shadow so SROA cannot
            // turn reload/store pairs on the shadow slot into reachable `ptr poison` and then
            // leak that poison back into explicit-frame roots.
            self.build_volatile_refactor_gc_root_store(frame_slot, gc_ptr_ty.const_null())?;
        } else {
            self.build_volatile_refactor_gc_root_store(slot, gc_ptr_ty.const_null())?;
            self.track_gc_root_slots_for_spill_slot(at, slot, gc_ptr_ty.into(), name)?;
        }
        Ok(slot)
    }

    pub(super) fn store_refactor_gc_root_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let value =
            self.refactor_cast_ptr(value, self.llvm_gc_i8_ptr_type(), &format!("{name}_gc"))?;
        if let Some(frame_slot) = self.refactor_gc_root_explicit_frame_slot(at, slot, name)? {
            self.build_volatile_refactor_gc_root_store(frame_slot, value)?;
            Ok(())
        } else {
            self.build_volatile_refactor_gc_root_store(slot, value)?;
            self.sync_storage_slot_into_explicit_frame(
                at,
                slot,
                self.llvm_gc_i8_ptr_type().into(),
                name,
            )
        }
    }

    pub(super) fn load_refactor_gc_root_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let load_slot = self
            .refactor_gc_root_explicit_frame_slot(at, slot, name)?
            .unwrap_or(slot);
        self.build_volatile_refactor_gc_root_load(load_slot, name)
    }

    pub(super) fn refactor_alloc_gc_struct(
        &mut self,
        at: crate::span::Span,
        struct_ty: StructType<'ctx>,
        layout_anchor_name: &str,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let desc =
            self.get_or_create_refactor_gc_type_descriptor(at, struct_ty, layout_anchor_name)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            &format!("{name}_type_desc"),
        )?;
        let alloc = self.declare_runtime_alloc_typed();
        let size = self.target_data.get_store_size(&struct_ty);
        let call = self.build_call_preserving_gc_local_roots(
            at,
            alloc,
            &[
                desc_i8.into(),
                self.context.i64_type().const_int(size, false).into(),
            ],
            &format!("rt_alloc_{name}"),
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| frontend_error("scoop_alloc_typed 未返回 pointer".to_string()))?
            .into_pointer_value();
        let ptr = self.refactor_cast_ptr(raw, self.llvm_ptr_type(self.gc_address_space()), name)?;
        self.refactor_zero_gc_object_payload(struct_ty, ptr, name)?;
        Ok(ptr)
    }

    pub(super) fn get_or_create_refactor_gc_type_descriptor(
        &mut self,
        at: crate::span::Span,
        struct_ty: StructType<'ctx>,
        layout_anchor_name: &str,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!("{layout_anchor_name}__type_desc");
        let trace_start_offset_bytes = if struct_ty.count_fields() > 1 {
            self.target_data
                .offset_of_element(&struct_ty, 1)
                .unwrap_or(0)
        } else {
            self.target_data.get_store_size(&struct_ty)
        };
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            type_id_key: layout_anchor_name,
            obj_ty: struct_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    pub(super) fn refactor_zero_gc_object_payload(
        &mut self,
        struct_ty: StructType<'ctx>,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        for field_index in 1..struct_ty.count_fields() {
            let Some(field_ty) = struct_ty.get_field_type_at_index(field_index) else {
                return Err(frontend_error(format!(
                    "refactor GC object `{name}` 缺少 field {}",
                    field_index
                )));
            };
            let field_ptr = self.builder.build_struct_gep(
                struct_ty,
                ptr,
                field_index,
                &format!("{name}_zero_field_{field_index}"),
            )?;
            self.builder.build_store(field_ptr, field_ty.const_zero())?;
        }
        Ok(())
    }

    pub(super) fn refactor_store_gc_aware_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.refactor_store_gc_aware_basic_value(at, ptr, value.get_type(), value, name)
    }

    pub(super) fn refactor_store_gc_aware_basic_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        value: BasicValueEnum<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        match value_ty {
            BasicTypeEnum::PointerType(ptr_ty)
                if ptr_ty.get_address_space() == self.gc_address_space()
                    && ptr.get_type().get_address_space() == self.gc_address_space() =>
            {
                let BasicValueEnum::PointerValue(value_ptr) = value else {
                    return Err(frontend_error(format!(
                        "refactor GC-aware store `{name}` 的值不是 pointer"
                    )));
                };
                self.store_gc_pointer_slot_with_write_barrier(at, ptr, value_ptr)
            }
            BasicTypeEnum::StructType(struct_ty)
                if ptr.get_type().get_address_space() == self.gc_address_space()
                    && self.basic_type_contains_gc_ptrs(at, value_ty)? =>
            {
                let BasicValueEnum::StructValue(struct_value) = value else {
                    return Err(frontend_error(format!(
                        "refactor GC-aware store `{name}` 的值不是 struct"
                    )));
                };
                for field_index in 0..struct_ty.count_fields() {
                    let Some(field_ty) = struct_ty.get_field_type_at_index(field_index) else {
                        return Err(frontend_error(format!(
                            "refactor GC-aware store `{name}` 缺少 field {}",
                            field_index
                        )));
                    };
                    let field_ptr = self.builder.build_struct_gep(
                        struct_ty,
                        ptr,
                        field_index,
                        &format!("{name}_field_{field_index}"),
                    )?;
                    let field_value = self.builder.build_extract_value(
                        struct_value,
                        field_index,
                        &format!("{name}_field_value_{field_index}"),
                    )?;
                    self.refactor_store_gc_aware_basic_value(
                        at,
                        field_ptr,
                        field_ty,
                        field_value,
                        name,
                    )?;
                }
                Ok(())
            }
            BasicTypeEnum::ArrayType(_) if self.basic_type_contains_gc_ptrs(at, value_ty)? => {
                Err(frontend_error(format!(
                    "refactor GC-aware store `{name}` 尚未发布 array payload root/write-barrier contract"
                )))
            }
            _ => {
                self.builder.build_store(ptr, value)?;
                Ok(())
            }
        }
    }

    pub(crate) fn refactor_cast_ptr(
        &self,
        ptr: PointerValue<'ctx>,
        target_ty: inkwell::types::PointerType<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if ptr.get_type().get_address_space() == target_ty.get_address_space() {
            Ok(self.builder.build_pointer_cast(ptr, target_ty, name)?)
        } else {
            Ok(self
                .builder
                .build_address_space_cast(ptr, target_ty, name)?)
        }
    }

    pub(crate) fn refactor_build_step_complete(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        self.refactor_build_step_variant(
            step_layout,
            step_layout.complete_variant(),
            STEP_TAG_COMPLETE as u32,
            payload,
            None,
        )
    }
}
