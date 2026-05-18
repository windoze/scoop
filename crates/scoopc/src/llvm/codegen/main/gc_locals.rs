//! Local pointer / GC-sensitive deferral: zero init, ptr rematerialization, root tracking, deferred CG value reload.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn zero_initializer_for_basic_type(
        &self,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match llvm_ty {
            BasicTypeEnum::IntType(ty) => BasicValueEnum::IntValue(ty.const_int(0, false)),
            BasicTypeEnum::PointerType(ty) => BasicValueEnum::PointerValue(ty.const_null()),
            BasicTypeEnum::StructType(ty) => BasicValueEnum::StructValue(ty.const_zero()),
            BasicTypeEnum::ArrayType(ty) => BasicValueEnum::ArrayValue(ty.const_zero()),
            BasicTypeEnum::FloatType(ty) => BasicValueEnum::FloatValue(ty.const_float(0.0)),
            BasicTypeEnum::VectorType(ty) => BasicValueEnum::VectorValue(ty.const_zero()),
            BasicTypeEnum::ScalableVectorType(ty) => {
                BasicValueEnum::ScalableVectorValue(ty.const_zero())
            }
        }
    }

    pub(in crate::llvm::codegen) fn rematerialize_ptr_in_current_block(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let Some(inst) = ptr.as_instruction_value() else {
            return Ok(ptr);
        };

        match inst.get_opcode() {
            inkwell::values::InstructionOpcode::Load => {
                let base = inst
                    .get_operand(0)
                    .and_then(|operand| operand.value())
                    .expect("load instruction must expose its base operand");
                let BasicValueEnum::PointerValue(base_ptr) = base else {
                    std::panic::panic_any("load base operand must be a pointer");
                };
                let base_ptr =
                    self.rematerialize_ptr_in_current_block(at, base_ptr, &format!("{name}_base"))?;
                let rebuilt = self.builder.build_load(ptr.get_type(), base_ptr, name)?;
                return Ok(rebuilt.into_pointer_value());
            }
            inkwell::values::InstructionOpcode::BitCast
            | inkwell::values::InstructionOpcode::AddrSpaceCast => {
                let base = inst
                    .get_operand(0)
                    .and_then(|operand| operand.value())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "local slot cast base operand",
                        at: at.into(),
                    })?;
                let BasicValueEnum::PointerValue(base_ptr) = base else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "local slot cast base pointer type",
                        at: at.into(),
                    });
                };
                let base_ptr =
                    self.rematerialize_ptr_in_current_block(at, base_ptr, &format!("{name}_base"))?;
                let target_ty = ptr.get_type();
                return if base_ptr.get_type().get_address_space() == target_ty.get_address_space() {
                    Ok(self.builder.build_pointer_cast(base_ptr, target_ty, name)?)
                } else {
                    Ok(self
                        .builder
                        .build_address_space_cast(base_ptr, target_ty, name)?)
                };
            }
            inkwell::values::InstructionOpcode::GetElementPtr => {}
            _ => return Ok(ptr),
        }

        let base = inst
            .get_operand(0)
            .and_then(|operand| operand.value())
            .expect("GEP instruction must expose its base operand");
        let BasicValueEnum::PointerValue(base_ptr) = base else {
            std::panic::panic_any("GEP base operand must be a pointer");
        };
        let base_ptr =
            self.rematerialize_ptr_in_current_block(at, base_ptr, &format!("{name}_base"))?;

        let source_ty = inst.get_gep_source_element_type().unwrap_or_else(|_| {
            panic!("rematerialize_ptr_in_current_block: local slot GEP must publish source type")
        });
        match source_ty {
            BasicTypeEnum::StructType(struct_ty) => {
                let mut indices = inst.get_indices();
                if indices.is_empty() {
                    for operand_index in 1..inst.get_num_operands() {
                        let Some(operand) =
                            inst.get_operand(operand_index).and_then(|op| op.value())
                        else {
                            return Ok(ptr);
                        };
                        let BasicValueEnum::IntValue(index_value) = operand else {
                            return Ok(ptr);
                        };
                        let Some(index) = index_value.get_zero_extended_constant() else {
                            return Ok(ptr);
                        };
                        indices.push(index as u32);
                    }
                }
                let field_index = match indices.as_slice() {
                    [field_index] => *field_index,
                    [0, field_index] => *field_index,
                    _ => return Ok(ptr),
                };

                Ok(self
                    .builder
                    .build_struct_gep(struct_ty, base_ptr, field_index, name)?)
            }
            BasicTypeEnum::IntType(int_ty) if int_ty.get_bit_width() == 8 => {
                let mut index = None;
                for operand_index in 1..inst.get_num_operands() {
                    let Some(operand) = inst.get_operand(operand_index).and_then(|op| op.value())
                    else {
                        return Ok(ptr);
                    };
                    let BasicValueEnum::IntValue(index_value) = operand else {
                        return Ok(ptr);
                    };
                    let Some(constant) = index_value.get_zero_extended_constant() else {
                        return Ok(ptr);
                    };
                    if index.replace(constant).is_some() {
                        return Ok(ptr);
                    }
                }
                let Some(index) = index else {
                    return Ok(ptr);
                };
                let rebuilt = unsafe {
                    self.builder.build_in_bounds_gep(
                        int_ty,
                        base_ptr,
                        &[self.context.i64_type().const_int(index, false)],
                        name,
                    )?
                };
                Ok(rebuilt)
            }
            _ => Ok(ptr),
        }
    }

    pub(in crate::llvm::codegen) fn local_ptr_for_use(
        &mut self,
        at: crate::span::Span,
        local: CgLocal<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.storage_slot_for_use(at, local.ptr, local.ty, name)
    }

    pub(in crate::llvm::codegen) fn clear_spill_slot_root_homes(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        for (_, value_ptr_ty, frame_slot) in
            self.explicit_frame_leaf_slot_pairs_for_storage_slot(at, slot, value_ty, name_prefix)?
        {
            let _ = self
                .builder
                .build_store(frame_slot, value_ptr_ty.const_null())?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn collect_gc_ptr_leaf_slots_in_spill(
        &mut self,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
        out: &mut Vec<(PointerValue<'ctx>, PointerType<'ctx>)>,
    ) -> Result<(), LlvmEmitError> {
        match value_ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push((slot, ptr_ty));
                }
            }
            BasicTypeEnum::StructType(st) => {
                if st.is_opaque() {
                    return Ok(());
                }
                for (idx, field_ty) in st.get_field_types().into_iter().enumerate() {
                    let field_slot = self.builder.build_struct_gep(
                        st,
                        slot,
                        idx as u32,
                        &format!("{name_prefix}_field_{idx}"),
                    )?;
                    self.collect_gc_ptr_leaf_slots_in_spill(
                        field_slot,
                        field_ty,
                        name_prefix,
                        out,
                    )?;
                }
            }
            BasicTypeEnum::ArrayType(arr) => {
                let i32_ty = self.context.i32_type();
                let zero = i32_ty.const_zero();
                for idx in 0..arr.len() {
                    let elem_slot = unsafe {
                        self.builder.build_in_bounds_gep(
                            arr,
                            slot,
                            &[zero, i32_ty.const_int(idx as u64, false)],
                            &format!("{name_prefix}_elem_{idx}"),
                        )?
                    };
                    self.collect_gc_ptr_leaf_slots_in_spill(
                        elem_slot,
                        arr.get_element_type(),
                        name_prefix,
                        out,
                    )?;
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn defer_gc_sensitive_cg_value(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: CgValue<'ctx>,
    ) -> Result<DeferredCgValue<'ctx>, LlvmEmitError> {
        let ty = value.ty;
        let Some(raw) = value.value else {
            return Ok(DeferredCgValue {
                ty,
                immediate: None,
                spill: None,
            });
        };

        let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
        if !self.basic_type_contains_gc_ptrs(at, llvm_ty)? {
            return Ok(DeferredCgValue {
                ty,
                immediate: Some(raw),
                spill: None,
            });
        }

        let slot = self.create_entry_alloca(at, name, ty)?;
        let _ = self.store_local_value_exact(at, slot, ty, value)?;
        self.track_gc_root_slots_for_spill_slot(at, slot, llvm_ty, name)?;

        Ok(DeferredCgValue {
            ty,
            immediate: None,
            spill: Some(DeferredGcSensitiveSpill {
                slot,
                value_ty: llvm_ty,
            }),
        })
    }

    pub(in crate::llvm::codegen) fn defer_gc_ref_pointer(
        &mut self,
        at: crate::span::Span,
        name: &str,
        ptr: PointerValue<'ctx>,
    ) -> Result<DeferredCgValue<'ctx>, LlvmEmitError> {
        self.defer_gc_sensitive_cg_value(
            at,
            name,
            CgValue {
                ty: CgTy::Ref,
                value: Some(ptr.into()),
            },
        )
    }

    pub(in crate::llvm::codegen) fn reload_deferred_gc_ref_without_clearing(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: &DeferredCgValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if let Some(spill) = &value.spill {
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let loaded = self
                .builder
                .build_load(self.llvm_gc_i8_ptr_type(), reload_slot, name)?;
            return Ok(loaded.into_pointer_value());
        }

        let Some(raw) = value.immediate else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "deferred gc ref reload",
                at: at.into(),
            });
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "deferred gc ref reload type",
                at: at.into(),
            });
        };
        Ok(ptr)
    }

    pub(in crate::llvm::codegen) fn reload_deferred_cg_value_without_clearing(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: &DeferredCgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(raw) = value.immediate {
            return Ok(CgValue {
                ty: value.ty,
                value: Some(raw),
            });
        }

        if let Some(spill) = &value.spill {
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
            let loaded = self.builder.build_load(llvm_ty, reload_slot, name)?;
            return Ok(CgValue {
                ty: value.ty,
                value: Some(loaded),
            });
        }

        match value.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => std::panic::panic_any("non-unit deferred value must have immediate or spill"),
        }
    }

    pub(in crate::llvm::codegen) fn clear_deferred_cg_value_root_homes(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: &DeferredCgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(spill) = &value.spill {
            self.clear_spill_slot_root_homes(at, spill.slot, spill.value_ty, name)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn materialize_deferred_cg_value(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if let Some(raw) = value.immediate {
            return Ok(CgValue {
                ty: value.ty,
                value: Some(raw),
            });
        }

        if let Some(spill) = value.spill {
            let reload_slot = self.storage_slot_for_use(at, spill.slot, value.ty, name)?;
            let llvm_ty = self.llvm_basic_type_of(at, value.ty)?;
            let loaded = self.builder.build_load(llvm_ty, reload_slot, name)?;
            self.clear_spill_slot_root_homes(at, spill.slot, spill.value_ty, name)?;
            return Ok(CgValue {
                ty: value.ty,
                value: Some(loaded),
            });
        }

        match value.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => std::panic::panic_any("non-unit deferred value must have immediate or spill"),
        }
    }
}
