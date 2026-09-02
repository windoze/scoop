use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Allocate a temporary in the entry block. Enum values are
    /// materialized through memory (see EnumWrap / EnumField); allocas
    /// must dominate every use, so they go before the first instruction
    /// of the entry block alongside the locals' slots.
    pub(super) fn entry_alloca(
        &self,
        ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let builder = self.builder;
        let current = builder
            .get_insert_block()
            .ok_or_else(|| CodegenError("builder has no insertion block".to_string()))?;
        match self.entry_block.get_first_instruction() {
            Some(first) => builder.position_at(self.entry_block, &first),
            None => builder.position_at_end(self.entry_block),
        }
        let alloca = builder.build_alloca(ty, name).map_err(|e| {
            CodegenError(format!(
                "alloca {name} @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })?;
        builder.position_at_end(current);
        Ok(alloca)
    }

    pub(super) fn statepoint_source_type(
        &self,
        source: scoop_lir::CallerRootSource,
    ) -> Result<&LirType, CodegenError> {
        match source {
            scoop_lir::CallerRootSource::Param(index) => {
                self.function.params.get(index as usize).ok_or_else(|| {
                    CodegenError(format!("statepoint param {index} is out of range"))
                })
            }
            scoop_lir::CallerRootSource::Local(id) => Ok(&self.function.locals[id].ty),
            scoop_lir::CallerRootSource::Temp(id) => Ok(&self.function.temps[id].ty),
        }
    }

    pub(super) fn root_source_storage(
        &self,
        source: scoop_lir::CallerRootSource,
    ) -> Result<RootStorage<'ctx>, CodegenError> {
        match source {
            scoop_lir::CallerRootSource::Local(id) => Ok(RootStorage {
                pointer: self.allocas[arena_index(id)],
                ty: basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    &self.function.locals[id].ty,
                )?,
            }),
            scoop_lir::CallerRootSource::Param(_) | scoop_lir::CallerRootSource::Temp(_) => self
                .root_storage
                .get(&source)
                .copied()
                .ok_or_else(|| CodegenError("root source has no canonical storage".to_string())),
        }
    }

    /// Turn the complete LIR live set into one independent AS1 SSA value per
    /// managed leaf. The post-call stores in `restore_statepoint_live` are
    /// deliberate uses: SROA exposes them to RS4GC, which rewrites each use to
    /// the corresponding `gc.relocate` result.
    pub(super) fn materialize_statepoint_live(
        &mut self,
        live: &scoop_lir::StatepointLiveSet,
        safepoint: scoop_lir::SafepointId,
    ) -> Result<MaterializedStatepointLive<'ctx>, CodegenError> {
        let mut arguments = HashMap::with_capacity(live.as_slice().len());
        let mut leaves = Vec::new();
        for item in live.as_slice() {
            let source_ty = self.statepoint_source_type(item.source)?;
            if source_ty != &item.ty {
                return Err(CodegenError(format!(
                    "statepoint {} source type is incomplete or inconsistent",
                    safepoint.get()
                )));
            }
            let llvm_ty = basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                &item.ty,
            )?;
            let storage = match item.source {
                scoop_lir::CallerRootSource::Local(id) => self.allocas[arena_index(id)],
                scoop_lir::CallerRootSource::Param(_) | scoop_lir::CallerRootSource::Temp(_) => {
                    let canonical = self.root_storage.get(&item.source).ok_or_else(|| {
                        CodegenError(format!(
                            "statepoint {} source lacks canonical root storage",
                            safepoint.get()
                        ))
                    })?;
                    if canonical.ty != llvm_ty {
                        return Err(CodegenError(format!(
                            "statepoint {} root storage disagrees with source type",
                            safepoint.get()
                        )));
                    }
                    canonical.pointer
                }
            };
            let source_size = self.target_data.get_store_size(&llvm_ty);
            for leaf in item.leaves.as_slice() {
                if leaf
                    .byte_offset
                    .checked_add(8)
                    .is_none_or(|end| end > source_size)
                {
                    return Err(CodegenError(format!(
                        "statepoint {} managed leaf offset {} is outside its concrete type",
                        safepoint.get(),
                        leaf.byte_offset
                    )));
                }
                // SAFETY: the LIR leaf path was computed from the concrete
                // physical layout and was range-checked immediately above.
                let leaf_storage = unsafe {
                    self.builder.build_gep(
                        self.context.i8_type(),
                        storage,
                        &[self.context.i64_type().const_int(leaf.byte_offset, false)],
                        &format!("statepoint_{}_leaf", safepoint.get()),
                    )
                }
                .map_err(|error| CodegenError(format!("address statepoint leaf: {error}")))?;
                let source_value = self
                    .builder
                    .build_load(
                        managed_ptr_ty(self.context, self.managed_address_space),
                        leaf_storage,
                        &format!("statepoint_{}_source", safepoint.get()),
                    )
                    .map_err(|error| CodegenError(format!("load statepoint leaf: {error}")))?
                    .into_pointer_value();
                let identity_storage = self.entry_alloca(
                    managed_ptr_ty(self.context, self.managed_address_space).into(),
                    "statepoint_root",
                )?;
                statepoint::mark_root_identity(
                    self.context,
                    identity_storage
                        .as_instruction_value()
                        .expect("entry_alloca returns an alloca instruction"),
                    safepoint,
                    item.source,
                    leaf.byte_offset,
                )?;
                self.builder
                    .build_store(identity_storage, source_value)
                    .map_err(|error| {
                        CodegenError(format!("initialize statepoint root: {error}"))
                    })?;
                let value = self
                    .builder
                    .build_load(
                        managed_ptr_ty(self.context, self.managed_address_space),
                        identity_storage,
                        &format!("statepoint_{}_live", safepoint.get()),
                    )
                    .map_err(|error| CodegenError(format!("load statepoint root: {error}")))?
                    .into_pointer_value();
                value
                    .as_instruction_value()
                    .expect("a statepoint leaf load is an instruction")
                    .set_volatile(true)
                    .map_err(|error| {
                        CodegenError(format!("make statepoint leaf load volatile: {error}"))
                    })?;
                leaves.push(StatepointLiveLeaf {
                    storage: leaf_storage,
                    value,
                });
            }
            let value = match llvm_ty {
                BasicTypeEnum::PointerType(pointer)
                    if pointer == managed_ptr_ty(self.context, self.managed_address_space)
                        && item.leaves.as_slice().len() == 1
                        && item.leaves.as_slice()[0].byte_offset == 0 =>
                {
                    leaves
                        .last()
                        .expect("the single managed leaf was materialized")
                        .value
                        .into()
                }
                _ => self
                    .builder
                    .build_load(
                        llvm_ty,
                        storage,
                        &format!("statepoint_{}_argument", safepoint.get()),
                    )
                    .map_err(|error| {
                        CodegenError(format!("reload materialized statepoint source: {error}"))
                    })?,
            };
            arguments.insert(item.source, value);
        }
        Ok(MaterializedStatepointLive { arguments, leaves })
    }

    pub(super) fn restore_statepoint_live(
        &mut self,
        live: MaterializedStatepointLive<'ctx>,
        safepoint: scoop_lir::SafepointId,
    ) -> Result<(), CodegenError> {
        for leaf in live.leaves {
            let store = self
                .builder
                .build_store(leaf.storage, leaf.value)
                .map_err(|error| {
                    CodegenError(format!(
                        "restore statepoint {} leaf: {error}",
                        safepoint.get()
                    ))
                })?;
            store.set_volatile(true).map_err(|error| {
                CodegenError(format!(
                    "make statepoint {} leaf store volatile: {error}",
                    safepoint.get()
                ))
            })?;
        }
        Ok(())
    }

    pub(super) fn prepare_root_storage(&mut self) -> Result<(), CodegenError> {
        for source in root_storage_sources(self.function) {
            if matches!(source, scoop_lir::CallerRootSource::Local(_)) {
                continue;
            }
            let lir_type = self.statepoint_source_type(source)?.clone();
            let ty = basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                &lir_type,
            )?;
            let pointer = self.entry_alloca(ty, "managed_root_storage")?;
            let initial = match source {
                scoop_lir::CallerRootSource::Param(index) => self
                    .llvm_function
                    .get_nth_param(index + self.param_offset)
                    .ok_or_else(|| CodegenError(format!("rooted param {index} is out of range")))?,
                scoop_lir::CallerRootSource::Temp(_) => ty.const_zero(),
                scoop_lir::CallerRootSource::Local(_) => unreachable!(),
            };
            self.builder
                .build_store(pointer, initial)
                .map_err(|error| {
                    CodegenError(format!("initialize managed root storage: {error}"))
                })?;
            if self
                .root_storage
                .insert(source, RootStorage { pointer, ty })
                .is_some()
            {
                return Err(CodegenError(
                    "complete root plans repeat one canonical source".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn publish_compiler_roots(
        &mut self,
        roots: &[scoop_lir::ExceptionalRoot],
    ) -> Result<CompilerRootFrame<'ctx>, CodegenError> {
        let context = self.context;
        let ptr = ptr_ty(context);
        let i64_type = context.i64_type();
        let index = self.compiler_invoke_index;
        self.compiler_invoke_index += 1;
        let mut entries = Vec::with_capacity(roots.len());
        for (root_index, exceptional) in roots.iter().enumerate() {
            let root = &exceptional.root;
            let storage = self.root_source_storage(root.source)?;
            let descriptor = emit_ref_scan(
                context,
                self.llvm,
                &format!("{}.invoke.{index}.root.{root_index}", self.function.symbol),
                root.scan.as_ref_scan(),
            )
            .expect("exceptional roots always carry a non-empty scan");
            entries.push((storage.pointer, descriptor));
        }

        let entry_type = context.struct_type(&[ptr.into(), ptr.into()], false);
        let entries_pointer = if entries.is_empty() {
            ptr.const_null()
        } else {
            let entry_count = u32::try_from(entries.len()).map_err(|_| {
                CodegenError("compiler-root entry count exceeds u32::MAX".to_string())
            })?;
            let array_type = entry_type.array_type(entry_count);
            let array = self.entry_alloca(array_type.into(), "compiler_root_entries")?;
            for (entry_index, (base, scan)) in entries.into_iter().enumerate() {
                let entry_index = u64::try_from(entry_index).map_err(|_| {
                    CodegenError("compiler-root entry index exceeds u64::MAX".to_string())
                })?;
                // SAFETY: `entry_index` is within the fixed entries array.
                let entry = unsafe {
                    self.builder.build_gep(
                        array_type,
                        array,
                        &[
                            context.i32_type().const_zero(),
                            context.i32_type().const_int(entry_index, false),
                        ],
                        "compiler_root_entry",
                    )
                }
                .map_err(|error| CodegenError(format!("compiler-root entry GEP: {error}")))?;
                let base_field = self
                    .builder
                    .build_struct_gep(entry_type, entry, 0, "compiler_root_base")
                    .map_err(|error| CodegenError(format!("compiler-root base GEP: {error}")))?;
                let scan_field = self
                    .builder
                    .build_struct_gep(entry_type, entry, 1, "compiler_root_scan")
                    .map_err(|error| CodegenError(format!("compiler-root scan GEP: {error}")))?;
                self.builder
                    .build_store(base_field, base)
                    .and_then(|_| self.builder.build_store(scan_field, scan))
                    .map_err(|error| {
                        CodegenError(format!("publish compiler-root entry: {error}"))
                    })?;
            }
            array
        };
        let frame_type = context.struct_type(&[ptr.into(), ptr.into(), i64_type.into()], false);
        let frame = self.entry_alloca(frame_type.into(), "compiler_root_frame")?;
        self.builder
            .build_store(frame, frame_type.const_zero())
            .map_err(|error| CodegenError(format!("zero compiler-root frame: {error}")))?;
        let push = self.gc_leaf_fn(
            "scoop_rt_push_compiler_roots",
            context
                .void_type()
                .fn_type(&[ptr.into(), ptr.into(), i64_type.into()], false),
        );
        let root_count = u64::try_from(roots.len())
            .map_err(|_| CodegenError("compiler-root count exceeds u64::MAX".to_string()))?;
        self.builder
            .build_call(
                push,
                &[
                    frame.into(),
                    entries_pointer.into(),
                    i64_type.const_int(root_count, false).into(),
                ],
                "push_compiler_roots",
            )
            .map_err(|error| CodegenError(format!("push compiler roots: {error}")))?;
        Ok(CompilerRootFrame { pointer: frame })
    }

    pub(super) fn validate_compiler_root_sources(
        &self,
        sources: impl IntoIterator<Item = scoop_lir::CallerRootSource>,
    ) -> Result<(), CodegenError> {
        for source in sources {
            if matches!(source, scoop_lir::CallerRootSource::Local(_)) {
                continue;
            }
            if !self.root_storage.contains_key(&source) {
                return Err(CodegenError(
                    "compiler-root edge names a source without canonical storage".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn pop_compiler_roots(
        &self,
        frame: CompilerRootFrame<'ctx>,
    ) -> Result<(), CodegenError> {
        let pop = self.gc_leaf_fn(
            "scoop_rt_pop_compiler_roots",
            self.context
                .void_type()
                .fn_type(&[ptr_ty(self.context).into()], false),
        );
        self.builder
            .build_call(pop, &[frame.pointer.into()], "pop_compiler_roots")
            .map_err(|error| CodegenError(format!("pop compiler roots: {error}")))?;
        Ok(())
    }

    pub(super) fn finish_unwind_compiler_roots(&mut self) -> Result<(), CodegenError> {
        if !self.compiler_unwind_blocks.contains(&self.current_block) {
            return Ok(());
        }
        let sources = self
            .unwind_root_sources
            .get(&self.current_block)
            .cloned()
            .unwrap_or_default();
        self.validate_compiler_root_sources(sources)?;
        let pop = self.gc_leaf_fn(
            "scoop_rt_pop_top_compiler_roots",
            self.context.void_type().fn_type(&[], false),
        );
        self.builder
            .build_call(pop, &[], "pop_unwind_compiler_roots")
            .map_err(|error| CodegenError(format!("pop unwind compiler roots: {error}")))?;
        Ok(())
    }
}
