//! Explicit frame layout: leaf slot allocation, root frame setup/teardown, storage-slot mirroring, GC ptr leaf collection, and helper function declarations.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn declare_exported_abi_function(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        declare_exported_abi_function(self.module, name, fn_ty)
    }

    pub(crate) fn declare_runtime_or_native_import_function(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        declare_runtime_or_native_import_function(self.module, name, fn_ty)
    }

    pub(crate) fn declare_compiler_private_helper_function(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
        linkage: Linkage,
    ) -> FunctionValue<'ctx> {
        declare_compiler_private_helper_function(self.module, name, fn_ty, linkage)
    }

    pub(in crate::llvm::codegen) fn enable_callable_carrier_contract(&self) {
        self.shared_caches
            .callable_carrier_contract_enabled
            .set(true);
    }

    pub(in crate::llvm::codegen) fn callable_carrier_contract_enabled(&self) -> bool {
        self.shared_caches.callable_carrier_contract_enabled.get()
    }

    pub(in crate::llvm::codegen) fn register_callable_carrier_entry_symbol(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
        symbol_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let mut symbols = self
            .shared_caches
            .callable_carrier_entry_symbols
            .borrow_mut();
        let key = (kind, callable_fqn.to_string());
        if self
            .shared_caches
            .plain_callable_carrier_fallback_targets
            .borrow()
            .contains(&key)
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 同时把 {} `{}` 发布为 plain fallback 和 effect-step target",
                    kind.label(),
                    callable_fqn,
                ),
            });
        }
        if let Some(existing) = symbols.get(&key) {
            if existing == symbol_name {
                return Ok(());
            }
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 为 {} `{}` 重复发布了不同 target：已有 `{}`，新值 `{}`",
                    kind.label(),
                    callable_fqn,
                    existing,
                    symbol_name,
                ),
            });
        }
        symbols.insert(key, symbol_name.to_string());
        Ok(())
    }

    pub(in crate::llvm::codegen) fn register_plain_callable_carrier_fallback(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<(), LlvmEmitError> {
        let key = (kind, callable_fqn.to_string());
        if self
            .shared_caches
            .callable_carrier_entry_symbols
            .borrow()
            .contains_key(&key)
        {
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 同时把 {} `{}` 发布为 effect-step target 和 plain fallback",
                    kind.label(),
                    callable_fqn,
                ),
            });
        }
        self.shared_caches
            .plain_callable_carrier_fallback_targets
            .borrow_mut()
            .insert(key);
        Ok(())
    }

    pub(in crate::llvm::codegen) fn callable_carrier_entry_symbol(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<Option<String>, LlvmEmitError> {
        if let Some(symbol) = self
            .shared_caches
            .callable_carrier_entry_symbols
            .borrow()
            .get(&(kind, callable_fqn.to_string()))
            .cloned()
        {
            return Ok(Some(symbol));
        }
        if self.callable_carrier_contract_enabled() {
            if self
                .shared_caches
                .plain_callable_carrier_fallback_targets
                .borrow()
                .contains(&(kind, callable_fqn.to_string()))
            {
                return Ok(None);
            }
            return Err(LlvmEmitError::Frontend {
                message: format!(
                    "callable carrier contract 缺少 {} `{}` 的 published target entry",
                    kind.label(),
                    callable_fqn,
                ),
            });
        }
        Ok(None)
    }

    pub(in crate::llvm::codegen) fn plain_callable_carrier_fallback_allowed(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> bool {
        self.shared_caches
            .plain_callable_carrier_fallback_targets
            .borrow()
            .contains(&(kind, callable_fqn.to_string()))
    }

    pub(in crate::llvm::codegen) fn callable_carrier_target_fn_ptr(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
        fallback_target: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let Some(symbol_name) = self.callable_carrier_entry_symbol(kind, callable_fqn)? else {
            return Ok(fallback_target);
        };
        let function = self
            .module
            .get_function(&symbol_name)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "refactor callable carrier contract 为 {} `{}` 发布了 target `{symbol_name}`，但 LLVM module 中缺少对应 function shell",
                    kind.label(),
                    callable_fqn,
                ),
            })?;
        Ok(function.as_global_value().as_pointer_value())
    }

    pub(crate) fn begin_function_explicit_frame_layout(
        &mut self,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let entry = llvm_fun
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: crate::span::Span::new(0, 0).into(),
            })?;
        let entry_builder = self.context.create_builder();
        match entry.get_first_instruction() {
            Some(inst) => entry_builder.position_before(&inst),
            None => entry_builder.position_at_end(entry),
        }

        let storage = entry_builder.build_array_alloca(
            self.llvm_ptr_type(AddressSpace::default()),
            self.context.i32_type().const_int(2, false),
            "explicit_root_frame_storage",
        )?;
        self.function_cx.explicit_frame_layout = ExplicitFrameLayoutPlan {
            function_symbol: Some(
                llvm_fun
                    .get_name()
                    .to_str()
                    .unwrap_or("anonymous")
                    .to_string(),
            ),
            frame_storage: Some(storage),
            slot_tys: Vec::new(),
        };
        Ok(())
    }

    pub(crate) fn finish_function_explicit_frame_layout(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let plan = std::mem::take(&mut self.function_cx.explicit_frame_layout);
        let Some(ref function_symbol) = plan.function_symbol else {
            return Ok(());
        };

        let slot_count = plan.slot_tys.len();
        let frame_ty_name = explicit_root_frame_type_name(function_symbol);
        let frame_ty = self
            .context
            .get_struct_type(&frame_ty_name)
            .unwrap_or_else(|| self.context.opaque_struct_type(&frame_ty_name));
        let header_ty = self.llvm_explicit_root_frame_header_type();

        let gc_slot_ty = self.llvm_gc_i8_ptr_type();
        let mut field_tys: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + slot_count);
        field_tys.push(header_ty.into());
        field_tys.extend((0..slot_count).map(|_| BasicTypeEnum::PointerType(gc_slot_ty)));
        frame_ty.set_body(&field_tys, false);

        let ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let offset_ptr = if slot_count == 0 {
            ptr_ty.const_null()
        } else {
            let offset_global_name = explicit_root_frame_offsets_global_name(function_symbol);
            let offsets_gv = if let Some(existing) = self.module.get_global(&offset_global_name) {
                existing
            } else {
                let mut offsets = Vec::with_capacity(slot_count);
                for field_index in 0..slot_count {
                    let offset = self.explicit_root_frame_slot_offset_bytes(field_index)?;
                    offsets.push(i32_ty.const_int(offset, false));
                }

                let arr_ty = i32_ty.array_type(slot_count as u32);
                let gv = self.module.add_global(arr_ty, None, &offset_global_name);
                gv.set_initializer(&i32_ty.const_array(&offsets));
                gv.set_constant(true);
                gv.set_linkage(Linkage::Internal);
                gv
            };
            offsets_gv.as_pointer_value().const_cast(ptr_ty)
        };

        let desc_global_name = explicit_root_frame_desc_global_name(function_symbol);
        if self.module.get_global(&desc_global_name).is_none() {
            let desc_ty = self.llvm_explicit_root_frame_desc_type();
            let init = desc_ty.const_named_struct(&[
                i32_ty.const_int(slot_count as u64, false).into(),
                offset_ptr.into(),
            ]);
            let gv = self.module.add_global(desc_ty, None, &desc_global_name);
            gv.set_initializer(&init);
            gv.set_constant(true);
            gv.set_linkage(Linkage::Internal);
        }

        // 即使当前函数没有显式 GC leaf slots，也必须把 zero-slot frame 挂到 TLS：
        // verify-roots / moving GC 需要一个统一的 managed root source，不能退回到
        // 已不再作为普通托管函数真源的 stackmap 路径。
        self.finalize_function_explicit_frame_lifecycle(at, &plan, &desc_global_name)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn reserve_explicit_frame_leaf_slots_for_storage_type(
        &mut self,
        at: crate::span::Span,
        storage_ty: BasicTypeEnum<'ctx>,
    ) -> Result<Vec<PointerValue<'ctx>>, LlvmEmitError> {
        if self
            .function_cx
            .explicit_frame_layout
            .function_symbol
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut leaf_tys = Vec::new();
        self.collect_gc_ptr_leaf_pointer_types_in_basic_type(at, storage_ty, &mut leaf_tys)?;

        let Some(frame_storage) = self.function_cx.explicit_frame_layout.frame_storage else {
            return Ok(Vec::new());
        };

        let mut frame_slots = Vec::with_capacity(leaf_tys.len());
        for leaf_ty in leaf_tys {
            let slot_index = self.function_cx.explicit_frame_layout.slot_tys.len();
            self.function_cx
                .explicit_frame_layout
                .slot_tys
                .push(leaf_ty);
            frame_slots.push(self.explicit_root_frame_slot_pointer(
                at,
                frame_storage,
                slot_index,
                leaf_ty,
                &format!("explicit_root_frame_slot_{slot_index}"),
            )?);
        }
        Ok(frame_slots)
    }

    pub(in crate::llvm::codegen) fn explicit_root_frame_header_size_bytes(
        &self,
    ) -> Result<u64, LlvmEmitError> {
        let header_ty = self.llvm_explicit_root_frame_header_type();
        let size = self.target_data.get_store_size(&header_ty);
        if size == 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame header size",
                at: crate::span::Span::new(0, 0).into(),
            });
        }
        Ok(size)
    }

    pub(in crate::llvm::codegen) fn explicit_root_frame_slot_offset_bytes(
        &self,
        slot_index: usize,
    ) -> Result<u64, LlvmEmitError> {
        Ok(self.explicit_root_frame_header_size_bytes()?
            + (slot_index as u64 * self.target_layout().pointer_size.max(1)))
    }

    pub(in crate::llvm::codegen) fn explicit_root_frame_slot_pointer(
        &self,
        at: crate::span::Span,
        frame_storage: PointerValue<'ctx>,
        slot_index: usize,
        _slot_ty: PointerType<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let entry = frame_storage
            .as_instruction_value()
            .and_then(|inst| inst.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame entry block",
                at: at.into(),
            })?;
        let builder = self.context.create_builder();
        let mut cursor = entry.get_first_instruction();
        while let Some(inst) = cursor {
            if inst.get_opcode() != inkwell::values::InstructionOpcode::Alloca {
                builder.position_before(&inst);
                break;
            }
            cursor = inst.get_next_instruction();
        }
        if cursor.is_none() {
            builder.position_at_end(entry);
        }

        let frame_i8 = builder.build_pointer_cast(
            frame_storage,
            self.llvm_i8_ptr_type(),
            &format!("{name}_base"),
        )?;
        let i64_ty = self.context.i64_type();
        let offset = self.explicit_root_frame_slot_offset_bytes(slot_index)?;
        let slot_addr = unsafe {
            builder.build_in_bounds_gep(
                self.context.i8_type(),
                frame_i8,
                &[i64_ty.const_int(offset, false)],
                name,
            )?
        };
        Ok(builder.build_pointer_cast(
            slot_addr,
            self.llvm_ptr_type(AddressSpace::default()),
            &format!("{name}_slot"),
        )?)
    }

    pub(in crate::llvm::codegen) fn record_explicit_frame_slot_mirrors(
        &mut self,
        slot: PointerValue<'ctx>,
        frame_slots: Vec<PointerValue<'ctx>>,
    ) {
        if frame_slots.is_empty() {
            return;
        }
        self.function_cx
            .explicit_frame_slot_mirrors
            .insert(pointer_value_key(slot), frame_slots);
    }

    pub(in crate::llvm::codegen) fn explicit_frame_slot_mirrors_for(
        &self,
        slot: PointerValue<'ctx>,
    ) -> Option<&[PointerValue<'ctx>]> {
        self.function_cx
            .explicit_frame_slot_mirrors
            .get(&pointer_value_key(slot))
            .map(Vec::as_slice)
    }

    pub(in crate::llvm::codegen) fn explicit_frame_leaf_slot_pairs_for_storage_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<Vec<(PointerValue<'ctx>, PointerType<'ctx>, PointerValue<'ctx>)>, LlvmEmitError>
    {
        if self
            .function_cx
            .explicit_frame_layout
            .frame_storage
            .is_none()
        {
            return Ok(Vec::new());
        }

        let slot =
            self.rematerialize_ptr_in_current_block(at, slot, &format!("{name_prefix}_slot"))?;
        let Some(frame_slots) = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec())
        else {
            return Ok(Vec::new());
        };

        let mut gc_leaf_slots = Vec::new();
        self.collect_gc_ptr_leaf_slots_in_spill(slot, value_ty, name_prefix, &mut gc_leaf_slots)?;
        if frame_slots.len() != gc_leaf_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "spill slot/frame slot count mismatch",
                at: at.into(),
            });
        }

        Ok(gc_leaf_slots
            .into_iter()
            .zip(frame_slots)
            .map(|((leaf_slot, value_ptr_ty), frame_slot)| (leaf_slot, value_ptr_ty, frame_slot))
            .collect())
    }

    /// 对单个 pointer-shaped GC 值，返回 post-safepoint 应优先 reload 的 explicit-frame home slot。
    ///
    /// aggregate / multi-leaf 值仍交给后续 refresh/rebuild contract 处理；这里先收紧 direct
    /// ref / string / niche-pointer 这类“单槽 GC 值”的 reload source-of-truth。
    pub(in crate::llvm::codegen) fn explicit_frame_single_gc_ptr_reload_slot_for_storage_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<Option<PointerValue<'ctx>>, LlvmEmitError> {
        let BasicTypeEnum::PointerType(ptr_ty) = value_ty else {
            return Ok(None);
        };
        if ptr_ty.get_address_space() != self.gc_address_space() {
            return Ok(None);
        }

        let mut pairs =
            self.explicit_frame_leaf_slot_pairs_for_storage_slot(at, slot, value_ty, name_prefix)?;
        if pairs.is_empty() {
            return Ok(None);
        }
        if pairs.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "single gc ptr explicit frame reload slot",
                at: at.into(),
            });
        }

        let (_, _, frame_slot) = pairs.remove(0);
        Ok(Some(frame_slot))
    }

    pub(in crate::llvm::codegen) fn rebuild_value_from_storage_slot_with_explicit_frame(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        frame_slots: &[PointerValue<'ctx>],
        frame_index: &mut usize,
        name_prefix: &str,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        Ok(match value_ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    let frame_slot = frame_slots.get(*frame_index).copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "aggregate explicit frame rebuild slot",
                            at: at.into(),
                        },
                    )?;
                    *frame_index += 1;
                    self.builder.build_load(
                        ptr_ty,
                        frame_slot,
                        &format!("{name_prefix}_frame_reload"),
                    )?
                } else {
                    self.builder.build_load(
                        ptr_ty,
                        slot,
                        &format!("{name_prefix}_scalar_reload"),
                    )?
                }
            }
            BasicTypeEnum::StructType(struct_ty) => {
                if struct_ty.is_opaque() {
                    self.builder.build_load(
                        struct_ty,
                        slot,
                        &format!("{name_prefix}_opaque_reload"),
                    )?
                } else {
                    let mut agg = struct_ty.get_undef();
                    for (idx, field_ty) in struct_ty.get_field_types().into_iter().enumerate() {
                        let field_slot = self.builder.build_struct_gep(
                            struct_ty,
                            slot,
                            idx as u32,
                            &format!("{name_prefix}_field_gep_{idx}"),
                        )?;
                        let field = self.rebuild_value_from_storage_slot_with_explicit_frame(
                            at,
                            field_slot,
                            field_ty,
                            frame_slots,
                            frame_index,
                            name_prefix,
                        )?;
                        agg = self
                            .builder
                            .build_insert_value(
                                agg,
                                field,
                                idx as u32,
                                &format!("{name_prefix}_field_insert_{idx}"),
                            )?
                            .into_struct_value();
                    }
                    agg.into()
                }
            }
            BasicTypeEnum::ArrayType(array_ty) => {
                let mut agg = array_ty.get_undef();
                let i32_ty = self.context.i32_type();
                let zero = i32_ty.const_zero();
                for idx in 0..array_ty.len() {
                    let elem_slot = unsafe {
                        self.builder.build_in_bounds_gep(
                            array_ty,
                            slot,
                            &[zero, i32_ty.const_int(idx as u64, false)],
                            &format!("{name_prefix}_elem_gep_{idx}"),
                        )?
                    };
                    let elem = self.rebuild_value_from_storage_slot_with_explicit_frame(
                        at,
                        elem_slot,
                        array_ty.get_element_type(),
                        frame_slots,
                        frame_index,
                        name_prefix,
                    )?;
                    agg = self
                        .builder
                        .build_insert_value(
                            agg,
                            elem,
                            idx,
                            &format!("{name_prefix}_elem_insert_{idx}"),
                        )?
                        .into_array_value();
                }
                agg.into()
            }
            BasicTypeEnum::IntType(int_ty) => {
                self.builder
                    .build_load(int_ty, slot, &format!("{name_prefix}_int_reload"))?
            }
            BasicTypeEnum::FloatType(float_ty) => {
                self.builder
                    .build_load(float_ty, slot, &format!("{name_prefix}_float_reload"))?
            }
            BasicTypeEnum::VectorType(vector_ty) => {
                self.builder
                    .build_load(vector_ty, slot, &format!("{name_prefix}_vector_reload"))?
            }
            BasicTypeEnum::ScalableVectorType(vector_ty) => self.builder.build_load(
                vector_ty,
                slot,
                &format!("{name_prefix}_scalable_vector_reload"),
            )?,
        })
    }

    pub(in crate::llvm::codegen) fn storage_slot_for_use(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        cg_ty: CgTy,
        name_prefix: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self.rematerialize_ptr_in_current_block(at, slot, name_prefix)?;
        let llvm_ty = self.llvm_basic_type_of(at, cg_ty)?;
        if let Some(frame_slot) = self.explicit_frame_single_gc_ptr_reload_slot_for_storage_slot(
            at,
            slot,
            llvm_ty,
            name_prefix,
        )? {
            return Ok(frame_slot);
        }
        if !self.basic_type_contains_gc_ptrs(at, llvm_ty)? {
            return Ok(slot);
        }
        let Some(frame_slots) = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec())
        else {
            return Ok(slot);
        };
        if frame_slots.is_empty() {
            return Ok(slot);
        }

        let scratch =
            self.create_entry_scratch_alloca_raw(at, &format!("{name_prefix}_rebuild"), llvm_ty)?;
        let mut frame_index = 0;
        let rebuilt = self.rebuild_value_from_storage_slot_with_explicit_frame(
            at,
            slot,
            llvm_ty,
            frame_slots.as_slice(),
            &mut frame_index,
            name_prefix,
        )?;
        if frame_index != frame_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "aggregate explicit frame rebuild arity",
                at: at.into(),
            });
        }
        let _ = self.builder.build_store(scratch, rebuilt)?;
        self.apply_alloca_alignment_for_ty(at, scratch, cg_ty)?;
        Ok(scratch)
    }

    pub(in crate::llvm::codegen) fn sync_storage_slot_into_explicit_frame(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        for (leaf_slot, value_ptr_ty, frame_slot) in
            self.explicit_frame_leaf_slot_pairs_for_storage_slot(at, slot, value_ty, name_prefix)?
        {
            let loaded = self
                .builder
                .build_load(value_ptr_ty, leaf_slot, &format!("{name_prefix}_reload"))?
                .into_pointer_value();
            let _ = self.builder.build_store(frame_slot, loaded)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn sync_basic_value_into_explicit_frame(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        raw: BasicValueEnum<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
    ) -> Result<(), LlvmEmitError> {
        let slot =
            self.rematerialize_ptr_in_current_block(at, slot, &format!("{name_prefix}_slot"))?;
        let Some(frame_slots) = self
            .explicit_frame_slot_mirrors_for(slot)
            .map(|slots| slots.to_vec())
        else {
            return Ok(());
        };
        if frame_slots.is_empty() {
            return Ok(());
        }

        let mut leaves = Vec::new();
        if !self.collect_gc_ptr_leaf_values_in_basic_value(
            raw,
            value_ty,
            name_prefix,
            &mut leaves,
        )? {
            return self.sync_storage_slot_into_explicit_frame(at, slot, value_ty, name_prefix);
        }
        if leaves.len() != frame_slots.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value/frame slot count mismatch",
                at: at.into(),
            });
        }
        for ((leaf, _leaf_ty), frame_slot) in leaves.into_iter().zip(frame_slots) {
            let ptr = leaf.into_pointer_value();
            let _ = self.builder.build_store(frame_slot, ptr)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn collect_gc_ptr_leaf_values_in_basic_value(
        &mut self,
        raw: BasicValueEnum<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        name_prefix: &str,
        out: &mut Vec<(BasicValueEnum<'ctx>, PointerType<'ctx>)>,
    ) -> Result<bool, LlvmEmitError> {
        match value_ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if !matches!(raw, BasicValueEnum::PointerValue(_)) {
                    return Ok(false);
                }
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push((raw, ptr_ty));
                }
            }
            BasicTypeEnum::StructType(struct_ty) => {
                if struct_ty.is_opaque() {
                    return Ok(true);
                }
                let BasicValueEnum::StructValue(raw) = raw else {
                    return Ok(false);
                };
                for (idx, field_ty) in struct_ty.get_field_types().into_iter().enumerate() {
                    let field = self.builder.build_extract_value(
                        raw,
                        idx as u32,
                        &format!("{name_prefix}_leaf_value_{idx}"),
                    )?;
                    if !self.collect_gc_ptr_leaf_values_in_basic_value(
                        field,
                        field_ty,
                        name_prefix,
                        out,
                    )? {
                        return Ok(false);
                    }
                }
            }
            BasicTypeEnum::ArrayType(array_ty) => {
                let BasicValueEnum::ArrayValue(raw) = raw else {
                    return Ok(false);
                };
                for idx in 0..array_ty.len() {
                    let field = self.builder.build_extract_value(
                        raw,
                        idx,
                        &format!("{name_prefix}_leaf_array_value_{idx}"),
                    )?;
                    if !self.collect_gc_ptr_leaf_values_in_basic_value(
                        field,
                        array_ty.get_element_type(),
                        name_prefix,
                        out,
                    )? {
                        return Ok(false);
                    }
                }
            }
            BasicTypeEnum::IntType(_)
            | BasicTypeEnum::FloatType(_)
            | BasicTypeEnum::VectorType(_)
            | BasicTypeEnum::ScalableVectorType(_) => {}
        }
        Ok(true)
    }

    pub(in crate::llvm::codegen) fn finalize_function_explicit_frame_lifecycle(
        &mut self,
        at: crate::span::Span,
        plan: &ExplicitFrameLayoutPlan<'ctx>,
        desc_global_name: &str,
    ) -> Result<(), LlvmEmitError> {
        let Some(frame_storage) = plan.frame_storage else {
            return Ok(());
        };
        let frame_storage_inst =
            frame_storage
                .as_instruction_value()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "explicit root frame storage alloca",
                    at: at.into(),
                })?;
        let total_words = self
            .context
            .i32_type()
            .const_int((2 + plan.slot_tys.len()) as u64, false);
        unsafe {
            llvm_sys::core::LLVMSetOperand(
                frame_storage_inst.as_value_ref(),
                0,
                total_words.as_value_ref(),
            );
        }

        let desc_global =
            self.module
                .get_global(desc_global_name)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "explicit root frame descriptor global",
                    at: at.into(),
                })?;
        self.emit_explicit_root_frame_entry_setup(
            at,
            frame_storage,
            plan.slot_tys.len(),
            desc_global,
        )?;
        self.emit_explicit_root_frame_return_pops(at, frame_storage, plan.slot_tys.as_slice())?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_explicit_root_frame_entry_setup(
        &self,
        at: crate::span::Span,
        frame_storage: PointerValue<'ctx>,
        slot_count: usize,
        desc_global: GlobalValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let frame_header_ty = self.llvm_explicit_root_frame_header_type();
        let insert_block = frame_storage
            .as_instruction_value()
            .and_then(|inst| inst.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame entry block",
                at: at.into(),
            })?;
        let builder = self.context.create_builder();
        let mut cursor = insert_block.get_first_instruction();
        while let Some(inst) = cursor {
            if inst.get_opcode() != inkwell::values::InstructionOpcode::Alloca {
                builder.position_before(&inst);
                break;
            }
            cursor = inst.get_next_instruction();
        }
        if cursor.is_none() {
            builder.position_at_end(insert_block);
        }

        let top_tls = self.declare_runtime_explicit_root_frame_top_tls();
        let frame_header = builder.build_pointer_cast(
            frame_storage,
            self.llvm_ptr_type(AddressSpace::default()),
            "explicit_root_frame_header",
        )?;
        let prev_ptr = builder.build_struct_gep(
            frame_header_ty,
            frame_header,
            0,
            "explicit_root_frame_prev_ptr",
        )?;
        let desc_ptr = builder.build_struct_gep(
            frame_header_ty,
            frame_header,
            1,
            "explicit_root_frame_desc_ptr",
        )?;
        let prev = builder.build_load(
            self.llvm_ptr_type(AddressSpace::default()),
            top_tls.as_pointer_value(),
            "explicit_root_frame_prev",
        )?;
        builder.build_store(prev_ptr, prev)?;
        builder.build_store(desc_ptr, desc_global.as_pointer_value())?;

        let null_gc = self.llvm_gc_i8_ptr_type().const_null();
        let frame_i8 = builder.build_pointer_cast(
            frame_storage,
            self.llvm_i8_ptr_type(),
            "explicit_root_frame_i8",
        )?;
        let i64_ty = self.context.i64_type();
        for slot_index in 0..slot_count {
            let offset = self.explicit_root_frame_slot_offset_bytes(slot_index)?;
            let slot_addr = unsafe {
                builder.build_in_bounds_gep(
                    self.context.i8_type(),
                    frame_i8,
                    &[i64_ty.const_int(offset, false)],
                    &format!("explicit_root_frame_init_slot_{slot_index}"),
                )?
            };
            let slot_ptr = builder.build_pointer_cast(
                slot_addr,
                self.llvm_ptr_type(AddressSpace::default()),
                &format!("explicit_root_frame_init_slot_ptr_{slot_index}"),
            )?;
            builder.build_store(slot_ptr, null_gc)?;
        }
        builder.build_store(top_tls.as_pointer_value(), frame_header)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn emit_explicit_root_frame_return_pops(
        &self,
        at: crate::span::Span,
        frame_storage: PointerValue<'ctx>,
        slot_tys: &[PointerType<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        let frame_header_ty = self.llvm_explicit_root_frame_header_type();
        let function = frame_storage
            .as_instruction_value()
            .and_then(|inst| inst.get_parent())
            .and_then(|bb| bb.get_parent())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "explicit root frame parent function",
                at: at.into(),
            })?;
        let top_tls = self.declare_runtime_explicit_root_frame_top_tls();
        let null_gc = self.llvm_gc_i8_ptr_type().const_null();

        for bb in function.get_basic_blocks() {
            let Some(term) = bb.get_terminator() else {
                continue;
            };
            let opcode = term.get_opcode();
            if opcode != inkwell::values::InstructionOpcode::Return
                && opcode != inkwell::values::InstructionOpcode::Unreachable
            {
                continue;
            }
            let builder = self.context.create_builder();
            builder.position_before(&term);
            let frame_header = builder.build_pointer_cast(
                frame_storage,
                self.llvm_ptr_type(AddressSpace::default()),
                "explicit_root_frame_pop_header",
            )?;
            let prev_ptr = builder.build_struct_gep(
                frame_header_ty,
                frame_header,
                0,
                "explicit_root_frame_pop_prev_ptr",
            )?;
            let prev = builder.build_load(
                self.llvm_ptr_type(AddressSpace::default()),
                prev_ptr,
                "explicit_root_frame_pop_prev",
            )?;
            let frame_i8 = builder.build_pointer_cast(
                frame_storage,
                self.llvm_i8_ptr_type(),
                "explicit_root_frame_pop_i8",
            )?;
            let i64_ty = self.context.i64_type();
            for (slot_index, _slot_ty) in slot_tys.iter().enumerate() {
                let offset = self.explicit_root_frame_slot_offset_bytes(slot_index)?;
                let slot_addr = unsafe {
                    builder.build_in_bounds_gep(
                        self.context.i8_type(),
                        frame_i8,
                        &[i64_ty.const_int(offset, false)],
                        &format!("explicit_root_frame_pop_slot_{slot_index}"),
                    )?
                };
                let slot_ptr = builder.build_pointer_cast(
                    slot_addr,
                    self.llvm_ptr_type(AddressSpace::default()),
                    &format!("explicit_root_frame_pop_slot_ptr_{slot_index}"),
                )?;
                builder.build_store(slot_ptr, null_gc)?;
            }
            builder.build_store(top_tls.as_pointer_value(), prev)?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn collect_gc_ptr_leaf_pointer_types_in_basic_type(
        &self,
        _at: crate::span::Span,
        ty: BasicTypeEnum<'ctx>,
        out: &mut Vec<PointerType<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        match ty {
            BasicTypeEnum::PointerType(ptr_ty) => {
                if ptr_ty.get_address_space() == self.gc_address_space() {
                    out.push(ptr_ty);
                }
            }
            BasicTypeEnum::StructType(st) => {
                if st.is_opaque() {
                    return Ok(());
                }
                for field_ty in st.get_field_types() {
                    self.collect_gc_ptr_leaf_pointer_types_in_basic_type(_at, field_ty, out)?;
                }
            }
            BasicTypeEnum::ArrayType(arr) => {
                for _ in 0..arr.len() {
                    self.collect_gc_ptr_leaf_pointer_types_in_basic_type(
                        _at,
                        arr.get_element_type(),
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
}
