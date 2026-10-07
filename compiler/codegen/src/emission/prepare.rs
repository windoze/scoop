use super::*;

pub(super) struct CallableCodePlan {
    pub safepoints: statepoint::ExpectedSafepoints,
    pub eh: artifact::ExpectedEh,
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

pub(super) fn prepare_callable_strong_llvm_module<'ctx, D, C, I>(
    context: &'ctx Context,
    module: &Module,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected: (&statepoint::ExpectedSafepoints, &artifact::ExpectedEh),
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<PreparedCallableModule<'ctx>, CodegenError> {
    let (llvm, mut runtime_metadata) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        machine,
        profile,
        StrongObjectEmissionSelection::CallableBody(body),
        |context, llvm, target_data, _, _| {
            Ok((
                runtime_metadata_v1::emit_callable_metadata_v1(
                    context,
                    llvm,
                    target_data,
                    profile,
                    production,
                    body,
                )?,
                declare_initialization_unit_globals(context, llvm, module, production)?,
            ))
        },
    )?;
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    llvm.verify()
        .map_err(|error| CodegenError(format!("invalid LLVM module: {error}")))?;
    let safepoints = statepoint::optimize(&llvm, machine, expected.0, profile)?;
    let eh = expected.1.finalize(&llvm)?;
    runtime_metadata_v1::emit_callable_safepoints(
        context,
        &llvm,
        &machine.get_target_data(),
        production,
        body,
        &safepoints,
        &mut runtime_metadata,
    )?;
    if profile.lir_target_profile().native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        crate::elf_llvm::prepare(&llvm, production.canonical_definitions())?;
    }
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    lower_and_verify(&llvm, machine, profile, &safepoints)?;
    Ok(PreparedCallableModule {
        llvm,
        runtime_metadata,
        plan: CallableCodePlan { safepoints, eh },
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

pub(super) fn verify_and_seal_object(
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

pub(super) fn discard_invalid_object(output: &Path, error: CodegenError) -> CodegenError {
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
) -> Result<statepoint::ExpectedSafepoints, CodegenError> {
    crate::metadata_sections::place_immutable_metadata(llvm, profile);
    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid LLVM module: {e}")))?;

    let final_plan = statepoint::optimize(llvm, machine, expected_safepoints, profile)?;
    lower_and_verify(llvm, machine, profile, &final_plan)?;
    Ok(final_plan)
}

fn lower_and_verify(
    llvm: &LlvmModule<'_>,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    final_plan: &statepoint::ExpectedSafepoints,
) -> Result<(), CodegenError> {
    statepoint::lower(llvm, machine)?;
    llvm.verify()
        .map_err(|e| CodegenError(format!("invalid post-RS4GC LLVM module: {e}")))?;
    statepoint::verify_rewritten(llvm, final_plan, profile)
}
