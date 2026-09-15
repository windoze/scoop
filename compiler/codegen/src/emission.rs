use super::*;

/// Verified strong object emission inputs retained for `.slib` packaging.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmittedStrongObjectV1 {
    production: scoop_lir::StrongProductionSectionV1,
    runtime_metadata: EmittedStrongRuntimeMetadataV1,
}

impl EmittedStrongObjectV1 {
    pub const fn production(&self) -> &scoop_lir::StrongProductionSectionV1 {
        &self.production
    }

    pub const fn runtime_metadata(&self) -> &EmittedStrongRuntimeMetadataV1 {
        &self.runtime_metadata
    }

    pub fn into_parts(
        self,
    ) -> (
        scoop_lir::StrongProductionSectionV1,
        EmittedStrongRuntimeMetadataV1,
    ) {
        (self.production, self.runtime_metadata)
    }
}

/// Translate one sealed strong LIR product to a provisional object and retain
/// the exact production/patch authority required by `.slib` packaging.
pub fn emit_object(
    input: &scoop_lir::SingleConeStrongLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    entry_source: scoop_lir::EntryProductionSourceV1,
    output: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedStrongObjectV1, CodegenError> {
    let module = input.module();
    validation::validate_module(module)?;
    profile.validate_lir_target_profile(module.meta.target_profile)?;
    let production = input
        .build_production_section(coordinate.clone(), entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    let expected_safepoints = statepoint::expectations(module)?;
    let expected_eh = artifact::eh_expectations(module)?;
    let machine = profile.create_target_machine()?;
    let context = Context::create();
    let (llvm, runtime_metadata) = prepare_strong_llvm_module(
        &context,
        module,
        &production,
        &machine,
        profile,
        &expected_safepoints,
    )?;

    machine
        .write_to_file(&llvm, FileType::Object, output)
        .map_err(|e| CodegenError(format!("failed to write {}: {e}", output.display())))?;
    if let Err(error) = profile.verify_object(output, &expected_safepoints, &expected_eh) {
        if let Err(remove_error) = std::fs::remove_file(output) {
            return Err(CodegenError(format!(
                "{error}; also failed to discard invalid object {}: {remove_error}",
                output.display()
            )));
        }
        return Err(error);
    }
    Ok(EmittedStrongObjectV1 {
        production,
        runtime_metadata,
    })
}

/// Translate sealed strong LIR to verified LLVM IR text without writing an
/// artifact. The rendered module includes the complete runtime metadata v1
/// surface and still contains zeroed digest slots awaiting object finalization.
///
/// This is the same translation and statepoint rewrite used by [`emit_object`].
pub fn render_llvm_ir(
    input: &scoop_lir::SingleConeStrongLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    entry_source: scoop_lir::EntryProductionSourceV1,
    profile: ValidatedBackendProfile,
) -> Result<String, CodegenError> {
    let module = input.module();
    validation::validate_module(module)?;
    profile.validate_lir_target_profile(module.meta.target_profile)?;
    let production = input
        .build_production_section(coordinate.clone(), entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    let expected_safepoints = statepoint::expectations(module)?;
    let machine = profile.create_target_machine()?;
    let context = Context::create();
    let (llvm, _) = prepare_strong_llvm_module(
        &context,
        module,
        &production,
        &machine,
        profile,
        &expected_safepoints,
    )?;
    Ok(llvm.print_to_string().to_string())
}

fn prepare_strong_llvm_module<'ctx>(
    context: &'ctx Context,
    module: &Module,
    production: &scoop_lir::StrongProductionSectionV1,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
) -> Result<(LlvmModule<'ctx>, EmittedStrongRuntimeMetadataV1), CodegenError> {
    if let scoop_lir::LirOutput::Executable { entry } = module.output {
        validation::validate_executable_entry(module, entry)?;
    }
    let llvm = emit_llvm_module(context, module, machine, profile)?;
    if let scoop_lir::LirOutput::Executable { entry } = module.output {
        let builder = context.create_builder();
        emit_executable_entry_shim(context, &llvm, &builder, module, entry)?;
    }
    let runtime_metadata = runtime_metadata_v1::emit_strong_runtime_metadata_v1(
        context,
        &llvm,
        &machine.get_target_data(),
        production,
    )?;

    verify_and_rewrite_module(&llvm, machine, profile, expected_safepoints)?;
    Ok((llvm, runtime_metadata))
}

fn verify_and_rewrite_module(
    llvm: &LlvmModule<'_>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
) -> Result<(), CodegenError> {
    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid LLVM module: {e}")))?;

    // M9 (milestone9 DESIGN 3.1, M0 spike): rewrite every call and
    // invoke in the GC-strategy functions into a `gc.statepoint`; the
    // object file's `__llvm_stackmaps` section is produced from them.
    statepoint::rewrite(llvm, machine)?;
    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid post-RS4GC LLVM module: {e}")))?;
    statepoint::verify_rewritten(llvm, expected_safepoints, profile)
}

/// Test helper for constructing the one supported host profile. Production
/// code receives a profile selected by the driver.
#[cfg(test)]
pub(crate) fn host_target_machine() -> Result<TargetMachine, CodegenError> {
    ResolvedTargetProfile::resolve_host()?
        .backend()
        .create_target_machine()
}

/// Translate `module` to an (unverified) LLVM module: globals,
/// TypeDescriptors, and every function.
pub(crate) fn emit_llvm_module<'ctx>(
    context: &'ctx Context,
    module: &Module,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
) -> Result<LlvmModule<'ctx>, CodegenError> {
    profile.validate_lir_target_profile(module.meta.target_profile)?;
    validation::validate_module(module)?;
    let managed_address_space = profile.managed_address_space_contract();
    let llvm = context.create_module("scoop");
    let builder = context.create_builder();
    let target_data = machine.get_target_data();
    llvm.set_triple(&machine.get_triple());
    llvm.set_data_layout(&target_data.get_data_layout());

    let ptr_ty = context.ptr_type(AddressSpace::default());
    let i8_ty = context.i8_type();
    let i64_ty = context.i64_type();

    let td_ty = runtime_metadata_v1::RuntimeMetadataV1Types::new(context).type_descriptor();
    // Declare every local and external descriptor before building any
    // initializer. Semantic edges resolve through typed ids; symbols are read
    // only from the selected entity at final emission.
    let type_tds: Vec<GlobalValue> = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let global = llvm.add_global(td_ty, None, descriptor.identity.symbol());
            apply_persistent_linkage(&global, descriptor.identity.symbol_request())?;
            Ok(global)
        })
        .collect::<Result<_, CodegenError>>()?;
    let external_type_tds: Vec<GlobalValue> = module
        .meta
        .core_external_type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let symbol = descriptor.expected_symbol().symbol();
            let global = llvm.add_global(td_ty, None, symbol.as_str());
            global.set_linkage(inkwell::module::Linkage::External);
            global
        })
        .collect();
    let string_td = type_descriptor_global(
        module.meta.well_known_type_descriptors.string,
        &type_tds,
        &external_type_tds,
    )?;
    let array_tds: Vec<GlobalValue> = module
        .meta
        .arrays
        .iter()
        .map(|(_, array)| {
            type_descriptor_global(array.type_descriptor, &type_tds, &external_type_tds)
        })
        .collect::<Result<_, _>>()?;

    // Shared "array index out of bounds" message (only when the module
    // performs a checked array access); trap blocks reference it.
    let bounds_message = if module_uses_bounds_checks(module) {
        let bytes = b"array index out of bounds";
        let ty = i8_ty.array_type(bytes.len() as u32 + 1);
        let global = llvm.add_global(ty, None, "scoop.trap.bounds");
        global.set_constant(true);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_initializer(&context.const_string(bytes, true));
        Some(global)
    } else {
        None
    };
    let array_size_message = if module_uses_array_assembly(module) {
        let bytes = b"array size overflow";
        let ty = i8_ty.array_type(bytes.len() as u32 + 1);
        let global = llvm.add_global(ty, None, "scoop.trap.array_size");
        global.set_constant(true);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_initializer(&context.const_string(bytes, true));
        Some(global)
    } else {
        None
    };

    // Ordinary globals are disjoint from descriptor identities.
    let mut globals: Vec<Option<GlobalValue>> = Vec::with_capacity(module.globals.len());
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst { identity, value } => {
                // { ptr td, i64 gc_word, i64 len, [N x i8] data }
                // (runtime spec 2.4; the 16-byte header is M9).
                let bytes = value.as_bytes();
                let ty = context.struct_type(
                    &[
                        ptr_ty.into(),
                        i64_ty.into(),
                        i64_ty.into(),
                        i8_ty.array_type(bytes.len() as u32).into(),
                    ],
                    false,
                );
                let llvm_global =
                    llvm.add_global(ty, Some(managed_address_space.inkwell()), global.symbol());
                llvm_global.set_constant(true);
                llvm_global.set_initializer(&context.const_struct(
                    &[
                        string_td.as_pointer_value().into(),
                        i64_ty.const_zero().into(),
                        i64_ty.const_int(bytes.len() as u64, false).into(),
                        context.const_string(bytes, false).into(),
                    ],
                    false,
                ));
                apply_persistent_linkage(&llvm_global, identity.symbol_request())?;
                globals.push(Some(llvm_global));
            }
            GlobalInit::CString { value, .. } => {
                // [N+1 x i8] c"...\00" (e.g. trap messages); private,
                // only referenced from within the module.
                let bytes = value.as_bytes();
                let ty = i8_ty.array_type(bytes.len() as u32 + 1);
                let llvm_global = llvm.add_global(ty, None, global.symbol());
                llvm_global.set_constant(true);
                llvm_global.set_linkage(inkwell::module::Linkage::Private);
                llvm_global.set_initializer(&context.const_string(bytes, true));
                globals.push(Some(llvm_global));
            }
            GlobalInit::Storage {
                identity,
                layout: _,
                ty: lir_ty,
                initial_state,
                thread_local,
            } => {
                let logical_ty = basic_ty(
                    context,
                    &module.structs,
                    &module.enums,
                    managed_address_space,
                    lir_ty,
                )?;
                let logical_size = target_data.get_store_size(&logical_ty);
                let logical_alignment = target_data.get_abi_alignment(&logical_ty);
                let (storage_ty, value) = if logical_size == 0 {
                    (i8_ty.into(), i8_ty.const_zero().into())
                } else {
                    let value = match initial_state {
                        LirStaticInitialState::ZeroedForRuntimeUnit => logical_ty.const_zero(),
                        LirStaticInitialState::EncodedStaticValue { payload } => llvm_constant(
                            context,
                            &module.structs,
                            &module.enums,
                            &globals,
                            managed_address_space,
                            lir_ty,
                            payload,
                        )?,
                    };
                    (logical_ty, value)
                };
                let llvm_global = llvm.add_global(storage_ty, None, global.symbol());
                llvm_global.set_initializer(&value);
                llvm_global.set_alignment(logical_alignment);
                llvm_global.set_thread_local(*thread_local);
                apply_persistent_linkage(&llvm_global, identity.symbol_request())?;
                globals.push(Some(llvm_global));
            }
        }
    }
    image_roots::emit(
        context,
        &llvm,
        &target_data,
        &module.globals,
        &globals,
        string_td,
    )?;

    // Two passes: declare every function first so call sites never
    // create shadow extern declarations (a forward call would
    // otherwise declare the symbol as extern, and the later definition
    // would be renamed with a `.N` suffix by LLVM, breaking the link).
    for function in &module.functions {
        declare_function(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            profile,
            function,
        )?;
    }
    for (_, callable) in module.meta.core_external_callables.iter() {
        declare_core_external_callable(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callable,
        )?;
    }
    let initialization_units = initialization::emit(context, &llvm, module, &globals)?;
    let module_ctx = ModuleCtx {
        managed_address_space,
        functions: &module.functions,
        structs: &module.structs,
        enums: &module.enums,
        extern_functions: &module.extern_functions,
        native_globals: &module.native_globals,
        native_global_bridges: &module.native_global_bridges,
        callback_bridges: &module.callback_bridges,
        foreign_callback_families: &module.foreign_callback_families,
        foreign_callback_bridges: &module.foreign_callback_bridges,
        globals_arena: &module.globals,
        globals: &globals,
        initialization_units: &initialization_units,
        arrays: &module.meta.arrays,
        array_tds: &array_tds,
        type_tds: &type_tds,
        external_type_tds: &external_type_tds,
        target_data: &target_data,
        bounds_message,
        array_size_message,
    };
    for (_, callback) in module.callback_bridges.iter() {
        declare_callback_trampoline(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callback,
        )?;
    }
    let mut declared_foreign_trampolines = HashSet::new();
    for (_, callback) in module.foreign_callback_bridges.iter() {
        if !declared_foreign_trampolines.insert(callback.trampoline.entry().symbol()) {
            continue;
        }
        declare_foreign_callback_trampoline(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callback,
        )?;
        let descriptor = llvm.add_global(
            context.i8_type(),
            None,
            callback.trampoline.signature_descriptor_symbol(),
        );
        descriptor.set_linkage(inkwell::module::Linkage::External);
    }
    // Meta TypeDescriptors reference module functions (vtable / itable
    // slots), so they are emitted after the declare pass.
    emit_type_descriptors(context, &llvm, &type_tds, &external_type_tds, module)?;
    for function in &module.functions {
        emit_function(context, &llvm, &builder, &module_ctx, function)?;
    }
    Ok(llvm)
}

fn apply_persistent_linkage(
    global: &GlobalValue<'_>,
    request: scoop_lir::PersistentSymbolRequest,
) -> Result<(), CodegenError> {
    use inkwell::GlobalVisibility;
    use inkwell::module::Linkage;
    use scoop_lir::LinkageClass;

    match request.linkage() {
        LinkageClass::ConeStrong => global.set_linkage(Linkage::External),
        LinkageClass::TemplateSupportHidden => {
            global.set_linkage(Linkage::External);
            global.set_visibility(GlobalVisibility::Hidden);
        }
        LinkageClass::OdrWeak => global.set_linkage(Linkage::WeakODR),
        LinkageClass::RuntimeAbi => {
            return Err(CodegenError(format!(
                "persistent symbol `{}` cannot use runtime ABI linkage",
                request.symbol()
            )));
        }
    }
    Ok(())
}
