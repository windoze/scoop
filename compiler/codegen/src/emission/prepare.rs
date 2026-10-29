use super::*;

pub(super) struct CallableCodePlan {
    pub safepoints: statepoint::LoweredSafepoints,
    pub has_unwind: bool,
}

pub(super) struct PreparedCallableModule<'ctx> {
    pub llvm: LlvmModule<'ctx>,
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
) -> Result<LlvmModule<'ctx>, CodegenError> {
    let symbol_plans = crate::symbol_plan::SymbolPlanIndex::new(production.canonical_definitions());
    let (llvm, ()) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        &symbol_plans,
        production
            .registration_production()
            .callables()
            .runtime_scans(),
        machine,
        profile,
        StrongObjectEmissionSelection::NonCallable,
        &GlobalReferences::all(module),
        |context, llvm, _, bounds_message, array_size_message| {
            let emitted_initialization_units =
                runtime_metadata_v1::emit_strong_runtime_metadata_v1(
                    context,
                    llvm,
                    profile,
                    production,
                    bounds_message,
                    array_size_message,
                )?;
            let initialization_units =
                initialization_unit_globals(module, &emitted_initialization_units)?;
            Ok(((), initialization_units))
        },
    )?;

    optimize_and_lower_module(&llvm, machine, profile, expected_safepoints)?;
    Ok(llvm)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_callable_strong_llvm_module<'ctx, D, C, I>(
    context: &'ctx Context,
    module: &Module,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    symbol_plans: &crate::symbol_plan::SymbolPlanIndex,
    machine: &TargetMachine,
    profile: ValidatedBackendProfile,
    expected: &statepoint::ExpectedSafepoints,
    function: &Function,
    declarations: &ConeDeclarations,
) -> Result<PreparedCallableModule<'ctx>, CodegenError> {
    let body = function.callable_body.id();
    let references = GlobalReferences::for_function(module, function, declarations);
    let (llvm, ()) = emit_llvm_module_with_surface(
        context,
        module,
        production.canonical_definitions(),
        symbol_plans,
        production
            .registration_production()
            .callables()
            .runtime_scans(),
        machine,
        profile,
        StrongObjectEmissionSelection::CallableBody(function),
        &references,
        |context, llvm, _, _, _| {
            Ok((
                runtime_metadata_v1::emit_callable_metadata_v1(
                    context, llvm, profile, production, body,
                )?,
                declare_initialization_unit_globals(
                    context,
                    llvm,
                    &declarations.initialization_units,
                    &references.initialization_units,
                )?,
            ))
        },
    )?;
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    statepoint::optimize(&llvm, machine, expected, profile)?;
    let has_unwind = prune_unused_personalities(&llvm);
    let safepoints = statepoint::lower(&llvm, machine)?;
    runtime_metadata_v1::emit_callable_safepoints(context, &llvm, production, body, &safepoints)?;
    if profile.lir_target_profile().native_object_format() == scoop_lir::NativeObjectFormat::Coff64
    {
        llvm.run_passes(
            "globaldce",
            machine,
            inkwell::passes::PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("COFF declaration cleanup: {error}")))?;
        crate::coff_llvm::prepare(&llvm, symbol_plans)?;
    }
    if profile.lir_target_profile().native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        symbol_plans.prepare_callable(&llvm, machine)?;
    }
    crate::metadata_sections::place_immutable_metadata(&llvm, profile);
    Ok(PreparedCallableModule {
        llvm,
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
    statepoint::lower(llvm, machine).map(|_| ())
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
