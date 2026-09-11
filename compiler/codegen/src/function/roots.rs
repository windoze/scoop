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
                symbol = self.function.symbol()
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
            scoop_lir::CallerRootSource::Param(index) => self
                .function
                .signature
                .arguments()
                .get(index as usize)
                .map(scoop_lir::AbiArgument::logical_storage_type)
                .ok_or_else(|| CodegenError(format!("statepoint param {index} is out of range"))),
            scoop_lir::CallerRootSource::Local(id) => Ok(&self.function.locals[id].ty),
            scoop_lir::CallerRootSource::Temp(id) => Ok(&self.function.temps[id].ty),
        }
    }

    pub(super) fn root_source_storage(
        &self,
        source: scoop_lir::CallerRootSource,
    ) -> Result<RootStorage<'ctx>, CodegenError> {
        let source_ty = self.statepoint_source_type(source)?;
        if validation::contains_machine_scalar(self.structs, self.enums, source_ty) {
            return Err(CodegenError(format!(
                "root source in @{} cannot contain an internal machine scalar",
                self.function.symbol()
            )));
        }
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
            if validation::contains_machine_scalar(self.structs, self.enums, &item.ty) {
                return Err(CodegenError(format!(
                    "statepoint {} cannot treat an internal machine scalar as a managed root",
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
            let storage = match source {
                scoop_lir::CallerRootSource::Param(index) => {
                    let logical_index = index as usize;
                    let argument = self
                        .function
                        .signature
                        .arguments()
                        .get(logical_index)
                        .ok_or_else(|| {
                            CodegenError(format!("rooted param {index} is out of range"))
                        })?;
                    let location = self
                        .function
                        .signature
                        .argument_location(logical_index)
                        .expect("an existing ABI argument has a location");
                    match (argument, location) {
                        (
                            scoop_lir::AbiArgument::Direct(_),
                            scoop_lir::AbiArgumentLocation::Parameter(physical_index),
                        ) => {
                            let physical_index = u32::try_from(physical_index).map_err(|_| {
                                CodegenError(format!(
                                    "physical parameter for rooted param {index} exceeds u32::MAX"
                                ))
                            })?;
                            let initial = self
                                .llvm_function
                                .get_nth_param(physical_index)
                                .ok_or_else(|| {
                                    CodegenError(format!(
                                        "physical parameter for rooted param {index} is out of range"
                                    ))
                                })?;
                            let pointer = self.entry_alloca(ty, "managed_root_storage")?;
                            self.builder
                                .build_store(pointer, initial)
                                .map_err(|error| {
                                    CodegenError(format!(
                                        "initialize managed root storage: {error}"
                                    ))
                                })?;
                            RootStorage { pointer, ty }
                        }
                        (
                            scoop_lir::AbiArgument::Indirect(_),
                            scoop_lir::AbiArgumentLocation::Parameter(physical_index),
                        ) => {
                            let physical_index = u32::try_from(physical_index).map_err(|_| {
                                CodegenError(format!(
                                    "physical parameter for rooted param {index} exceeds u32::MAX"
                                ))
                            })?;
                            let parameter = self
                                .llvm_function
                                .get_nth_param(physical_index)
                                .ok_or_else(|| {
                                    CodegenError(format!(
                                        "physical parameter for rooted param {index} is out of range"
                                    ))
                                })?;
                            let BasicValueEnum::PointerValue(pointer) = parameter else {
                                return Err(CodegenError(format!(
                                    "indirect rooted param {index} does not use pointer storage"
                                )));
                            };
                            RootStorage { pointer, ty }
                        }
                        (
                            scoop_lir::AbiArgument::ElidedZst(_),
                            scoop_lir::AbiArgumentLocation::Elided,
                        ) => {
                            return Err(CodegenError(format!(
                                "elided param {index} appears in a root plan despite its empty scan"
                            )));
                        }
                        _ => {
                            return Err(CodegenError(format!(
                                "rooted param {index} has an inconsistent Scoop ABI location"
                            )));
                        }
                    }
                }
                scoop_lir::CallerRootSource::Temp(_) => {
                    let pointer = self.entry_alloca(ty, "managed_root_storage")?;
                    self.builder
                        .build_store(pointer, ty.const_zero())
                        .map_err(|error| {
                            CodegenError(format!("initialize managed root storage: {error}"))
                        })?;
                    RootStorage { pointer, ty }
                }
                scoop_lir::CallerRootSource::Local(_) => {
                    return Err(CodegenError(
                        "local root storage must reuse its ordinary alloca".to_string(),
                    ));
                }
            };
            if self.root_storage.insert(source, storage).is_some() {
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
                &format!(
                    "{}.invoke.{index}.root.{root_index}",
                    self.function.symbol()
                ),
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

    /// Establish post-call values while the corresponding runtime root frame
    /// is still published. The matching volatile stores happen immediately
    /// after the NoGC pop, making every continuation value explicitly depend
    /// on the collector-updated storage rather than on a pre-call SSA copy.
    pub(super) fn reload_published_roots(
        &self,
        sources: impl IntoIterator<Item = scoop_lir::CallerRootSource>,
    ) -> Result<Vec<ReloadedRoot<'ctx>>, CodegenError> {
        let mut reloaded = Vec::new();
        for source in sources {
            let storage = self.root_source_storage(source)?;
            let value = self
                .builder
                .build_load(storage.ty, storage.pointer, "published_root_reload")
                .map_err(|error| CodegenError(format!("reload published root: {error}")))?;
            value
                .as_instruction_value()
                .expect("a root reload is an instruction")
                .set_volatile(true)
                .map_err(|error| {
                    CodegenError(format!("make published root reload volatile: {error}"))
                })?;
            reloaded.push(ReloadedRoot { storage, value });
        }
        Ok(reloaded)
    }

    pub(super) fn restore_reloaded_roots(
        &self,
        roots: Vec<ReloadedRoot<'ctx>>,
    ) -> Result<(), CodegenError> {
        for root in roots {
            let store = self
                .builder
                .build_store(root.storage.pointer, root.value)
                .map_err(|error| CodegenError(format!("restore reloaded root: {error}")))?;
            store.set_volatile(true).map_err(|error| {
                CodegenError(format!("make reloaded root store volatile: {error}"))
            })?;
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
        self.validate_compiler_root_sources(sources.iter().copied())?;
        let reloaded = self.reload_published_roots(sources)?;
        let pop = self.gc_leaf_fn(
            "scoop_rt_pop_top_compiler_roots",
            self.context.void_type().fn_type(&[], false),
        );
        self.builder
            .build_call(pop, &[], "pop_unwind_compiler_roots")
            .map_err(|error| CodegenError(format!("pop unwind compiler roots: {error}")))?;
        self.restore_reloaded_roots(reloaded)?;
        Ok(())
    }
}
