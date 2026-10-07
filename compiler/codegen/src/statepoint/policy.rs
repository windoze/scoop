use super::*;

/// Apply the LLVM GC strategy from typed LIR and the frame policy from the
/// validated backend projection.
pub(crate) fn configure_function(
    context: &Context,
    function: FunctionValue<'_>,
    effect: GcEffect,
    profile: ValidatedBackendProfile,
) {
    function.add_attribute(
        AttributeLoc::Function,
        context.create_string_attribute("frame-pointer", profile.frame_pointer_attribute()),
    );
    if profile.disable_red_zone() {
        function.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("noredzone"), 0),
        );
    }
    if effect == GcEffect::Managed {
        function.set_gc(GC_STRATEGY);
        function.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute(
                "disable-tail-calls",
                profile.disable_tail_calls_attribute(),
            ),
        );
    }
}

/// Run the qualified ordinary passes and finalize their physical root groups.
pub(crate) fn optimize(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
    expected: &ExpectedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<ExpectedSafepoints, CodegenError> {
    let passes = match profile.optimization() {
        scoop_lir::OptimizationMode::Debug => "function(sroa,mem2reg,sccp,unreachableblockelim)",
        scoop_lir::OptimizationMode::Release => {
            "function(sroa,mem2reg,instcombine,early-cse,dse,adce,early-cse,sccp,unreachableblockelim)"
        }
    };
    module
        .run_passes(passes, machine, PassBuilderOptions::create())
        .map_err(|error| CodegenError(format!("ordinary LLVM lowering failed: {error}")))?;
    finalization::finalize(module, expected, profile)
}

pub(crate) fn lower(module: &LlvmModule<'_>, machine: &TargetMachine) -> Result<(), CodegenError> {
    module
        .run_passes(
            "rewrite-statepoints-for-gc",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("rewrite-statepoints-for-gc failed: {error}")))
}

#[cfg(test)]
pub(crate) fn rewrite(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
    expected: &ExpectedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<ExpectedSafepoints, CodegenError> {
    let plan = optimize(module, machine, expected, profile)?;
    lower(module, machine)?;
    Ok(plan)
}
