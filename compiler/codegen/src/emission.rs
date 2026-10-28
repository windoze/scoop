use super::*;

mod callable_declarations;
use callable_declarations::CallableDeclarations;

mod global_declarations;
mod global_references;
use global_references::{ConeDeclarations, GlobalReferences};

mod initialization;
#[cfg(test)]
pub(crate) use initialization::initialization_unit_globals_from_pairs;
use initialization::{declare_initialization_unit_globals, initialization_unit_globals};

mod metadata_partition;
mod objects;
mod storage;
pub use objects::*;

mod prepare;
use prepare::*;

/// Test helper for constructing the one supported host profile. Production
/// code receives a profile selected by the driver.
#[cfg(test)]
pub(crate) fn host_target_machine() -> Result<TargetMachine, CodegenError> {
    ValidatedBackendProfile::darwin_aarch64_for_test().create_target_machine()
}

/// Definitions emitted into one physical LLVM module.
#[derive(Clone, Copy)]
enum StrongObjectEmissionSelection<'a> {
    #[cfg(test)]
    CompleteTestModule,
    NonCallable,
    CallableBody(&'a Function),
}

impl StrongObjectEmissionSelection<'_> {
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
            Self::CallableBody(selected) => selected.callable_body.id() == body,
        }
    }
}

// Every caller has checked this immutable module at its emission entry.
#[allow(clippy::too_many_arguments)]
fn emit_llvm_module_with_surface<'ctx, R>(
    context: &'ctx Context,
    module: &Module,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
    runtime_scan_plans: &scoop_lir::StrongCallableRuntimeScanPlanSetV1,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    selection: StrongObjectEmissionSelection<'_>,
    references: &GlobalReferences,
    emit_runtime_metadata: impl FnOnce(
        &'ctx Context,
        &LlvmModule<'ctx>,
        &inkwell::targets::TargetData,
        GlobalValue<'ctx>,
        GlobalValue<'ctx>,
    ) -> Result<
        (
            R,
            BTreeMap<scoop_lir::InitializationUnitId, GlobalValue<'ctx>>,
        ),
        CodegenError,
    >,
) -> Result<(LlvmModule<'ctx>, R), CodegenError> {
    let managed_address_space = profile.managed_address_space_contract();
    let llvm = context.create_module("scoop");
    let builder = context.create_builder();
    let target_data = machine.get_target_data();
    llvm.set_triple(&machine.get_triple());
    llvm.set_data_layout(&managed_address_space.data_layout(&target_data));

    let metadata_types = runtime_metadata_v1::RuntimeMetadataV1Types::new(context);
    let td_ty = metadata_types.type_descriptor();
    // Metadata initializers use the complete graph; callable modules need
    // only their typed references, without declarations for unrelated data.
    let type_tds = references
        .local_descriptors
        .iter()
        .map(|&id| {
            let descriptor = &module.meta.type_descriptors[id];
            let concrete_td = metadata_types.type_descriptor_storage(
                u32::try_from(descriptor.relations.related_types().len())
                    .expect("function arity fits its metadata count"),
            );
            let global = llvm.add_global(concrete_td, None, descriptor.identity.symbol());
            apply_persistent_linkage(
                &global,
                descriptor.identity.symbol_request(),
                selection.defines_non_callable(),
            )?;
            global.set_constant(true);
            Ok((id, global))
        })
        .collect::<Result<BTreeMap<_, _>, CodegenError>>()?;
    let external_type_tds = references
        .external_descriptors
        .iter()
        .map(|&id| {
            let descriptor = &module.meta.external_type_descriptors[id];
            let symbol = descriptor.expected_symbol().symbol();
            let global = llvm.add_global(td_ty, None, symbol.as_str());
            global.set_linkage(inkwell::module::Linkage::External);
            (id, global)
        })
        .collect::<BTreeMap<_, _>>();
    let type_descriptor_globals = TypeDescriptorGlobals {
        local: &type_tds,
        external: &external_type_tds,
    };
    let array_tds = references
        .arrays
        .iter()
        .map(|&id| {
            type_descriptor_global(
                module.meta.arrays[id].type_descriptor,
                type_descriptor_globals,
            )
            .map(|global| (id, global))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let bounds_message = emit_cone_trap_message(
        context,
        &llvm,
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
        surface,
        module.cone,
        (
            scoop_lir::ConeImageSupportRole::ArraySizeOverflowMessage,
            b"array size overflow",
        ),
        selection.defines_non_callable(),
    )?;

    let globals = global_declarations::emit(
        context,
        &llvm,
        module,
        surface,
        &target_data,
        profile,
        selection,
        references,
        type_descriptor_globals,
    )?;
    // Declare selected symbols before bodies to preserve forward and recursive
    // targets without introducing LLVM's automatic `.N` renaming.
    let declarations = CallableDeclarations::new(module, selection);
    for &function in &declarations.functions {
        declare_function(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            profile,
            function,
            selection.defines_callable(function.callable_body.id()),
        )?;
    }
    for &id in &declarations.external {
        let callable = &module.meta.external_callables[id];
        declare_external_callable(
            context,
            &llvm,
            &module.structs,
            &module.enums,
            managed_address_space,
            callable,
        )?;
    }
    for &id in &declarations.callbacks {
        let callback = &module.callback_bridges[id];
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
    for &id in &declarations.foreign_callbacks {
        let callback = &module.foreign_callback_bridges[id];
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
    // typed initialization registration definitions used by LIR values.
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
            surface,
            module,
            type_descriptor_globals,
        )?;
    }
    let debug = if profile.debug_info() == scoop_lir::DebugInfoLevel::None {
        None
    } else {
        declarations
            .functions
            .iter()
            .filter(|function| selection.defines_callable(function.callable_body.id()))
            .find_map(|function| crate::debug::source_file(function))
            .map(|source| crate::debug::DebugContext::new(context, &llvm, profile, source))
    };
    let module_ctx = ModuleCtx {
        debug: debug.as_ref(),
        // Snapshot forwarding regressed AArch64 reference-copy kernels.
        reuse_interface_values: profile.optimization() == scoop_lir::OptimizationMode::Release
            && machine.get_target().get_name() == c"x86-64",
        optimize_allocations: profile.optimization() == scoop_lir::OptimizationMode::Release,
        optimize_reference_writes: profile.optimization() == scoop_lir::OptimizationMode::Release,
        // Direct lookup regressed large random reference writes on x86.
        direct_array_regions: machine.get_target().get_name() == c"aarch64",
        // Larger x86 initialization regions had unstable register-allocation costs.
        allocation_initialization_limit: if machine.get_target().get_name() == c"aarch64" {
            128
        } else {
            64
        },
        // Both range simplification and alignment-only removal regressed x86 trees.
        optimize_tlab_range: machine.get_target().get_name() == c"aarch64",
        // The extra live register and cache check regressed x86 wide allocations.
        reuse_thread_addresses: profile.optimization() == scoop_lir::OptimizationMode::Release
            && machine.get_target().get_name() == c"aarch64",
        managed_address_space,
        functions: &module.functions,
        structs: &module.structs,
        enums: &module.enums,
        extern_functions: &module.extern_functions,
        external_callables: &module.meta.external_callables,
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
        type_descriptors: &module.meta.type_descriptors,
        type_tds: &type_tds,
        external_type_tds: &external_type_tds,
        target_data: &target_data,
    };
    for &function in &declarations.functions {
        if selection.defines_callable(function.callable_body.id()) {
            let runtime_scan_plan = runtime_scan_plans
                .callable(function.callable_body.id())
                .ok_or_else(|| {
                    CodegenError(format!(
                        "callable {} has no runtime scan plan",
                        function.callable_body.id()
                    ))
                })?;
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
    if module.meta.target_profile.native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        if !matches!(selection, StrongObjectEmissionSelection::CallableBody(_)) {
            crate::elf_llvm::prepare(&llvm, surface)?;
        }
        for &global in external_type_tds.values() {
            crate::elf_llvm::apply_visibility(global);
        }
        for (&id, &emitted) in &globals {
            if matches!(
                module.globals[id].init,
                GlobalInit::ImportedStorage { .. } | GlobalInit::ImportedImmortal { .. }
            ) {
                crate::elf_llvm::apply_visibility(emitted);
            }
        }
        for &id in &declarations.external {
            let callable = &module.meta.external_callables[id];
            if let Some(function) = llvm.get_function(callable.expected_symbol().symbol().as_str())
            {
                crate::elf_llvm::apply_visibility(function.as_global_value());
            }
        }
    }
    if let Some(debug) = debug {
        debug.finish();
    }
    Ok((llvm, runtime_metadata))
}

pub(crate) fn emit_cone_trap_message<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
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
    apply_persistent_linkage(&global, boundary.start(), define)?;
    if define {
        global.set_initializer(&context.const_string(bytes, true));
        atom_boundaries::emit_global_atom_boundaries_v1(
            llvm,
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
    profile.validate_lir_target_profile(module.meta.target_profile)?;
    if !module.initialization_units.is_empty() {
        return Err(CodegenError(
            "initialization units require a sealed strong production section".to_string(),
        ));
    }
    let foundation = scoop_lir::ConeLirFoundation::from_module(module)
        .map_err(|error| CodegenError(format!("strong LIR projection failed: {error}")))?;
    let surface = scoop_lir::ObjectSymbolSurfaceV1::from_foundation(&foundation)
        .map_err(|error| CodegenError(format!("strong symbol projection failed: {error}")))?;
    let runtime_scans = scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_module(module)
        .map_err(|error| CodegenError(format!("callable runtime scan planning failed: {error}")))?;
    emit_llvm_module_with_surface(
        context,
        module,
        &surface,
        &runtime_scans,
        machine,
        profile,
        StrongObjectEmissionSelection::CompleteTestModule,
        &GlobalReferences::all(module),
        |_, _, _, _, _| Ok(((), BTreeMap::new())),
    )
    .map(|(llvm, ())| llvm)
}

pub(crate) fn apply_persistent_linkage(
    global: &GlobalValue<'_>,
    request: scoop_lir::PersistentSymbolRequest,
    definition: bool,
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
        LinkageClass::OdrWeak => global.set_linkage(if definition {
            Linkage::WeakODR
        } else {
            Linkage::External
        }),
        LinkageClass::RuntimeAbi => {
            return Err(CodegenError(format!(
                "persistent symbol `{}` cannot use runtime ABI linkage",
                request.symbol()
            )));
        }
    }
    Ok(())
}
