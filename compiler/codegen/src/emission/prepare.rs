use super::*;

pub(super) struct CallableCodePlan {
    pub safepoints: statepoint::ExpectedSafepoints,
    pub has_unwind: bool,
}

pub(super) struct PreparedCallableModule<'ctx> {
    pub llvm: LlvmModule<'ctx>,
    pub runtime_metadata: EmittedStrongRuntimeMetadataV1,
    pub plan: CallableCodePlan,
}

pub(super) fn prepare_non_callable_strong_llvm_module<
    'ctx,
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
    I: Clone,
>(
    context: &'ctx Context,
    module: &Module,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
) -> Result<(LlvmModule<'ctx>, EmittedStrongRuntimeMetadataV1), CodegenError> {
    let (llvm, runtime_metadata) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        production
            .registration_production()
            .callables()
            .runtime_scans(),
        machine,
        profile,
        StrongObjectEmissionSelection::NonCallable,
        |context, llvm, _, bounds_message, array_size_message| {
            let runtime_metadata = runtime_metadata_v1::emit_strong_runtime_metadata_v1(
                context,
                llvm,
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

    optimize_and_lower_module(&llvm, machine, profile, expected_safepoints)?;
    Ok((llvm, runtime_metadata))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_callable_strong_llvm_module<'ctx, D, C, I>(
    context: &'ctx Context,
    module: &Module,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    elf_symbols: &crate::elf_llvm::SymbolPlanIndex,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected: &statepoint::ExpectedSafepoints,
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<PreparedCallableModule<'ctx>, CodegenError> {
    let (llvm, mut runtime_metadata) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        production
            .registration_production()
            .callables()
            .runtime_scans(),
        machine,
        profile,
        StrongObjectEmissionSelection::CallableBody(body),
        |context, llvm, _, _, _| {
            Ok((
                runtime_metadata_v1::emit_callable_metadata_v1(
                    context, llvm, profile, production, body,
                )?,
                declare_initialization_unit_globals(context, llvm, module, production)?,
            ))
        },
    )?;
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    let safepoints = statepoint::optimize(&llvm, machine, expected, profile)?;
    let has_unwind = prune_unused_personalities(&llvm);
    runtime_metadata_v1::emit_callable_safepoints(
        context,
        &llvm,
        production,
        body,
        &safepoints,
        &mut runtime_metadata,
    )?;
    if profile.lir_target_profile().native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        elf_symbols.prepare(&llvm)?;
    }
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    statepoint::lower(&llvm, machine)?;
    Ok(PreparedCallableModule {
        llvm,
        runtime_metadata,
        plan: CallableCodePlan {
            safepoints,
            has_unwind,
        },
    })
}

pub(super) fn write_object(
    machine: &TargetMachine,
    llvm: &LlvmModule<'_>,
    output: &Path,
) -> Result<(), CodegenError> {
    machine
        .write_to_file(llvm, FileType::Object, output)
        .map_err(|error| CodegenError(format!("failed to write {}: {error}", output.display())))
}

pub(super) fn discard_invalid_object(output: &Path, error: CodegenError) -> CodegenError {
    match std::fs::remove_file(output) {
        Ok(()) => error,
        Err(remove_error) => CodegenError(format!(
            "{error}; also failed to discard invalid object {}: {remove_error}",
            output.display()
        )),
    }
}

fn optimize_and_lower_module(
    llvm: &LlvmModule<'_>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected_safepoints: &statepoint::ExpectedSafepoints,
) -> Result<(), CodegenError> {
    crate::metadata_sections::place_immutable_metadata(llvm, profile);
    statepoint::optimize(llvm, machine, expected_safepoints, profile)?;
    statepoint::lower(llvm, machine)
}

/// Remove empty personality metadata after ordinary optimization deletes invokes.
fn prune_unused_personalities(module: &LlvmModule<'_>) -> bool {
    use inkwell::values::{AsValueRef, InstructionOpcode};
    let mut has_unwind = false;
    for function in module.get_functions() {
        if function.get_basic_blocks().iter().any(|block| {
            block
                .get_instructions()
                .any(|instruction| instruction.get_opcode() == InstructionOpcode::Invoke)
        }) {
            has_unwind = true;
        } else if function.get_personality_function().is_some() {
            // SAFETY: this function has no remaining local unwind edges.
            unsafe {
                inkwell::llvm_sys::core::LLVMSetPersonalityFn(
                    function.as_value_ref(),
                    std::ptr::null_mut(),
                );
            }
        }
    }
    has_unwind
}
