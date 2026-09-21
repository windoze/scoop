use super::*;

mod strong;
pub use strong::*;

fn prepare_non_callable_strong_llvm_module<
    'ctx,
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
    I: Clone,
>(
    context: &'ctx Context,
    module: &Module,
    production: &crate::strong_production::StrongProductionEmissionView<'_, D, C, I>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
) -> Result<(LlvmModule<'ctx>, EmittedStrongRuntimeMetadataV1), CodegenError> {
    if let scoop_lir::LirOutput::Executable { entry } = module.output {
        validation::validate_executable_entry(module, entry)?;
    }
    let (llvm, runtime_metadata) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        machine,
        profile,
        StrongObjectEmissionSelection::NonCallable,
        |context, llvm, target_data, bounds_message, array_size_message| {
            let runtime_metadata = runtime_metadata_v1::emit_strong_runtime_metadata_v1(
                context,
                llvm,
                target_data,
                profile,
                production,
                bounds_message,
                array_size_message,
            )?;
            let (runtime_metadata, emitted_initialization_units) = runtime_metadata.into_parts();
            let initialization_units =
                initialization_unit_globals(module, &emitted_initialization_units)?;
            Ok((runtime_metadata, initialization_units))
        },
    )?;

    verify_and_rewrite_module(&llvm, machine, profile, expected_safepoints)?;
    Ok((llvm, runtime_metadata))
}

fn prepare_callable_strong_llvm_module<'ctx, D, C, I>(
    context: &'ctx Context,
    module: &Module,
    production: &crate::strong_production::StrongProductionEmissionView<'_, D, C, I>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<LlvmModule<'ctx>, CodegenError> {
    if let scoop_lir::LirOutput::Executable { entry } = module.output {
        validation::validate_executable_entry(module, entry)?;
    }
    let (llvm, ()) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        machine,
        profile,
        StrongObjectEmissionSelection::CallableBody(body),
        |context, llvm, _, _, _| {
            Ok((
                (),
                declare_initialization_unit_globals(context, llvm, module, production)?,
            ))
        },
    )?;
    verify_and_rewrite_module(&llvm, machine, profile, expected_safepoints)?;
    Ok(llvm)
}

fn write_object(
    machine: &TargetMachine,
    llvm: &LlvmModule<'_>,
    output: &Path,
) -> Result<(), CodegenError> {
    machine
        .write_to_file(llvm, FileType::Object, output)
        .map_err(|error| CodegenError(format!("failed to write {}: {error}", output.display())))
}

fn verify_and_seal_object(
    output: &Path,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
    expected_eh: &artifact::ExpectedEh,
) -> Result<(), CodegenError> {
    if let Err(error) = profile.verify_object(output, expected_safepoints, expected_eh) {
        return Err(discard_invalid_object(output, error));
    }
    let mut permissions = std::fs::metadata(output)
        .map_err(|error| {
            CodegenError(format!(
                "cannot inspect verified object {}: {error}",
                output.display()
            ))
        })?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(output, permissions).map_err(|error| {
        CodegenError(format!(
            "cannot seal verified object {} read-only: {error}",
            output.display()
        ))
    })?;
    Ok(())
}

fn discard_invalid_object(output: &Path, error: CodegenError) -> CodegenError {
    match std::fs::remove_file(output) {
        Ok(()) => error,
        Err(remove_error) => CodegenError(format!(
            "{error}; also failed to discard invalid object {}: {remove_error}",
            output.display()
        )),
    }
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
    ValidatedBackendProfile::darwin_aarch64_for_test().create_target_machine()
}

/// Translate `module` to an (unverified) LLVM module: globals,
/// TypeDescriptors, and every function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StrongObjectEmissionSelection {
    #[cfg(test)]
    CompleteTestModule,
    NonCallable,
    CallableBody(scoop_lir::PersistentCallableBodyId),
}

impl StrongObjectEmissionSelection {
    fn defines_non_callable(self) -> bool {
        match self {
            #[cfg(test)]
            Self::CompleteTestModule => true,
            Self::NonCallable => true,
            Self::CallableBody(_) => false,
        }
    }

    fn defines_callable(self, body: scoop_lir::PersistentCallableBodyId) -> bool {
        match self {
            #[cfg(test)]
            Self::CompleteTestModule => true,
            Self::NonCallable => false,
            Self::CallableBody(selected) => selected == body,
        }
    }
}

fn emit_llvm_module_with_surface<'ctx, R>(
    context: &'ctx Context,
    module: &Module,
    surface: &scoop_lir::StrongObjectSymbolSurfaceV1,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    selection: StrongObjectEmissionSelection,
    emit_runtime_metadata: impl FnOnce(
        &'ctx Context,
        &LlvmModule<'ctx>,
        &inkwell::targets::TargetData,
        GlobalValue<'ctx>,
        GlobalValue<'ctx>,
    ) -> Result<(R, Vec<GlobalValue<'ctx>>), CodegenError>,
) -> Result<(LlvmModule<'ctx>, R), CodegenError> {
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
            global.set_constant(true);
            Ok(global)
        })
        .collect::<Result<_, CodegenError>>()?;
    let external_type_tds: Vec<GlobalValue> = module
        .meta
        .external_type_descriptors
        .iter()
        .map(|(_, descriptor)| {
            let symbol = descriptor.expected_symbol().symbol();
            let global = llvm.add_global(td_ty, None, symbol.as_str());
            global.set_linkage(inkwell::module::Linkage::External);
            global
        })
        .collect();
    let type_descriptor_globals = TypeDescriptorGlobals {
        local: &type_tds,
        external: &external_type_tds,
    };
    let string_td = type_descriptor_global(
        module.meta.well_known_type_descriptors.string,
        type_descriptor_globals,
    )?;
    let array_tds: Vec<GlobalValue> = module
        .meta
        .arrays
        .iter()
        .map(|(_, array)| type_descriptor_global(array.type_descriptor, type_descriptor_globals))
        .collect::<Result<_, _>>()?;

    let bounds_message = emit_cone_trap_message(
        context,
        &llvm,
        &target_data,
        surface,
        module.cone,
        (
            scoop_lir::ConeImageSupportRole::ArrayBoundsMessage,
            b"array index out of bounds",
        ),
        selection.defines_non_callable(),
    )?;
    let array_size_message = emit_cone_trap_message(
        context,
        &llvm,
        &target_data,
        surface,
        module.cone,
        (
            scoop_lir::ConeImageSupportRole::ArraySizeOverflowMessage,
            b"array size overflow",
        ),
        selection.defines_non_callable(),
    )?;

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
                if selection.defines_non_callable() {
                    llvm_global.set_initializer(&context.const_struct(
                        &[
                            string_td.as_pointer_value().into(),
                            i64_ty.const_zero().into(),
                            i64_ty.const_int(bytes.len() as u64, false).into(),
                            context.const_string(bytes, false).into(),
                        ],
                        false,
                    ));
                }
                apply_persistent_linkage(&llvm_global, identity.symbol_request())?;
                globals.push(Some(llvm_global));
            }
            GlobalInit::CString { identity, value } => {
                // [N+1 x i8] c"...\00" (e.g. trap messages), emitted as a
                // callable-owned support atom with a stable boundary symbol.
                let bytes = value.as_bytes();
                let ty = i8_ty.array_type(bytes.len() as u32 + 1);
                let llvm_global = llvm.add_global(ty, None, global.symbol());
                llvm_global.set_constant(true);
                apply_persistent_linkage(&llvm_global, identity.symbol_request())?;
                if selection.defines_callable(identity.owner()) {
                    llvm_global.set_initializer(&context.const_string(bytes, true));
                }
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
                let storage_ty = if logical_size == 0 {
                    i8_ty.into()
                } else {
                    logical_ty
                };
                let llvm_global = llvm.add_global(storage_ty, None, global.symbol());
                llvm_global.set_alignment(logical_alignment);
                llvm_global.set_thread_local(*thread_local);
                apply_persistent_linkage(&llvm_global, identity.symbol_request())?;
                if selection.defines_non_callable() {
                    let section = if logical_size == 0
                        || matches!(initial_state, LirStaticInitialState::ZeroedForRuntimeUnit)
                    {
                        profile.zero_fill_storage_section()
                    } else {
                        profile.writable_storage_section()
                    };
                    llvm_global.set_section(Some(section));
                    let value = if logical_size == 0 {
                        i8_ty.const_zero().into()
                    } else {
                        match initial_state {
                            LirStaticInitialState::ZeroedForRuntimeUnit => storage_ty.const_zero(),
                            LirStaticInitialState::EncodedStaticValue { payload } => llvm_constant(
                                context,
                                &module.structs,
                                &module.enums,
                                &globals,
                                managed_address_space,
                                lir_ty,
                                payload,
                            )?,
                        }
                    };
                    llvm_global.set_initializer(&value);
                }
                globals.push(Some(llvm_global));
            }
        }
    }
    atom_boundaries::emit_global_atom_boundaries_v1(
        &llvm,
        &target_data,
        surface,
        module.globals.iter().filter_map(|(_, global)| {
            let GlobalInit::CString { identity, .. } = &global.init else {
                return None;
            };
            if !selection.defines_callable(identity.owner()) {
                return None;
            }
            let owner = llvm
                .get_global(identity.symbol())
                .expect("the callable C string was emitted above");
            Some(atom_boundaries::GlobalAtomMaterializationV1::new(
                identity.atom_record().id(),
                owner,
            ))
        }),
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
    for (_, callable) in module.meta.dependency_external_callables.iter() {
        declare_dependency_external_callable(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callable,
        )?;
    }
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
    // Runtime records require only declarations of callable bodies and type
    // descriptors. Emitting them before function bodies exposes the exact
    // typed initialization coordinator definitions used by LIR values.
    let (runtime_metadata, initialization_units) = emit_runtime_metadata(
        context,
        &llvm,
        &target_data,
        bounds_message,
        array_size_message,
    )?;

    if selection.defines_non_callable() {
        shape_definitions::emit_strong_shape_definitions_v1(
            context,
            &llvm,
            &target_data,
            surface,
            module,
            type_descriptor_globals,
        )?;
    }
    let module_ctx = ModuleCtx {
        managed_address_space,
        functions: &module.functions,
        structs: &module.structs,
        enums: &module.enums,
        extern_functions: &module.extern_functions,
        core_external_callables: &module.meta.core_external_callables,
        dependency_external_callables: &module.meta.dependency_external_callables,
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
    let runtime_scan_plans = scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_module(module)
        .map_err(|error| CodegenError(format!("callable runtime scan planning failed: {error}")))?;
    for function in &module.functions {
        if selection.defines_callable(function.callable_body.id()) {
            let runtime_scan_plan = runtime_scan_plans
                .callable(function.callable_body.id())
                .expect("runtime scan planning covers every function");
            emit_function(
                context,
                &llvm,
                &builder,
                &module_ctx,
                function,
                surface,
                runtime_scan_plan,
            )?;
        }
    }
    Ok((llvm, runtime_metadata))
}

pub(crate) fn emit_cone_trap_message<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &inkwell::targets::TargetData,
    surface: &scoop_lir::StrongObjectSymbolSurfaceV1,
    producer: scoop_lir::ConeIdentity,
    message: (scoop_lir::ConeImageSupportRole, &[u8]),
    define: bool,
) -> Result<GlobalValue<'ctx>, CodegenError> {
    let (support, bytes) = message;
    let plan_key = scoop_lir::ObjectDefinitionPlanKey::strong(
        producer,
        scoop_lir::StrongDefinitionEntity::cone_image(producer),
        scoop_lir::StrongDefinitionRole::ImageDescriptor,
    )
    .map_err(|error| CodegenError(format!("cannot derive Cone image plan: {error}")))?;
    let plan = scoop_lir::ObjectDefinitionPlanId::from_key(&plan_key)
        .map_err(|error| CodegenError(format!("cannot derive Cone image plan id: {error}")))?;
    let atom =
        scoop_lir::ObjectDefinitionAtomId::from_key(&scoop_lir::ObjectDefinitionAtomKey::new(
            plan,
            scoop_lir::DefinitionAtomRole::AddressTakenConstant,
            scoop_lir::DefinitionAtomSubkey::ConeImageSupport(support),
        ))
        .map_err(|error| CodegenError(format!("cannot derive Cone trap-message atom: {error}")))?;
    let boundary = surface
        .plan(plan)
        .and_then(|plan| {
            plan.atom_boundaries()
                .iter()
                .find(|boundary| boundary.atom() == atom)
        })
        .copied()
        .ok_or_else(|| CodegenError(format!("Cone trap-message atom {atom} is unplanned")))?;
    let symbol = boundary.start().symbol();
    if llvm.get_global(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "Cone trap-message symbol `{symbol}` is already declared"
        )));
    }
    let ty = context.i8_type().array_type(bytes.len() as u32 + 1);
    let global = llvm.add_global(ty, None, symbol.as_str());
    global.set_constant(true);
    apply_persistent_linkage(&global, boundary.start())?;
    if define {
        global.set_initializer(&context.const_string(bytes, true));
        atom_boundaries::emit_global_atom_boundaries_v1(
            llvm,
            target_data,
            surface,
            [atom_boundaries::GlobalAtomMaterializationV1::new(
                atom, global,
            )],
        )?;
    }
    Ok(global)
}

#[cfg(test)]
pub(crate) fn emit_llvm_module<'ctx>(
    context: &'ctx Context,
    module: &Module,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
) -> Result<LlvmModule<'ctx>, CodegenError> {
    validation::validate_module(module)?;
    if !module.initialization_units.is_empty() {
        return Err(CodegenError(
            "initialization units require a sealed strong production section".to_string(),
        ));
    }
    let foundation = scoop_lir::OdrFreeLirFoundation::from_module(module)
        .map_err(|error| CodegenError(format!("strong LIR projection failed: {error}")))?;
    let surface = scoop_lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation)
        .map_err(|error| CodegenError(format!("strong symbol projection failed: {error}")))?;
    emit_llvm_module_with_surface(
        context,
        module,
        &surface,
        machine,
        profile,
        StrongObjectEmissionSelection::CompleteTestModule,
        |_, _, _, _, _| Ok(((), Vec::new())),
    )
    .map(|(llvm, ())| llvm)
}

fn initialization_unit_globals<'ctx>(
    module: &Module,
    emitted: &EmittedStrongInitializationUnitRegistrationSetV1<'ctx>,
) -> Result<Vec<GlobalValue<'ctx>>, CodegenError> {
    initialization_unit_globals_from_pairs(
        module,
        emitted
            .registrations()
            .iter()
            .map(|registration| (registration.unit(), registration.coordinator_descriptor())),
    )
}

fn declare_initialization_unit_globals<'ctx, D, C, I>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    module: &Module,
    production: &crate::strong_production::StrongProductionEmissionView<'_, D, C, I>,
) -> Result<Vec<GlobalValue<'ctx>>, CodegenError> {
    let plans = production
        .initialization_units()
        .registrations()
        .iter()
        .map(|plan| (plan.semantic().unit(), plan))
        .collect::<std::collections::BTreeMap<_, _>>();
    let ty = runtime_metadata_v1::coordinator_descriptor_type(context);
    let globals = module
        .initialization_units
        .iter()
        .map(|(_, unit)| {
            let id = unit.identity.id();
            let plan = plans.get(&id).ok_or_else(|| {
                CodegenError(format!(
                    "initialization unit {id} has no strong production plan"
                ))
            })?;
            let request = plan.descriptor_symbol();
            let symbol = request.symbol();
            if llvm.get_global(symbol.as_str()).is_some()
                || llvm.get_function(symbol.as_str()).is_some()
            {
                return Err(CodegenError(format!(
                    "initialization coordinator declaration `{symbol}` collides with an LLVM value"
                )));
            }
            let global = llvm.add_global(ty, None, symbol.as_str());
            global.set_constant(true);
            apply_persistent_linkage(&global, request)?;
            Ok(global)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if globals.len() != plans.len() {
        return Err(CodegenError(format!(
            "strong initialization declaration coverage mismatch: LIR {}, planned {}",
            globals.len(),
            plans.len()
        )));
    }
    Ok(globals)
}

pub(crate) fn initialization_unit_globals_from_pairs<'ctx>(
    module: &Module,
    emitted: impl IntoIterator<Item = (scoop_lir::PersistentInitializationUnitId, GlobalValue<'ctx>)>,
) -> Result<Vec<GlobalValue<'ctx>>, CodegenError> {
    let mut by_unit = std::collections::BTreeMap::new();
    for (unit, descriptor) in emitted {
        if by_unit.insert(unit, descriptor).is_some() {
            return Err(CodegenError(
                "strong initialization emission contains duplicate unit ids".to_string(),
            ));
        }
    }
    let globals = module
        .initialization_units
        .iter()
        .map(|(_, unit)| {
            by_unit.get(&unit.identity.id()).copied().ok_or_else(|| {
                CodegenError(format!(
                    "initialization unit {} has no emitted coordinator descriptor",
                    unit.identity.id()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if globals.len() != by_unit.len() {
        return Err(CodegenError(format!(
            "strong initialization coverage mismatch: LIR {}, emitted {}",
            globals.len(),
            by_unit.len()
        )));
    }
    Ok(globals)
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
