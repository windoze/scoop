use super::*;

/// Apply the LLVM GC strategy from typed LIR and the frame policy from the
/// validated backend projection.
pub(crate) fn configure_function(
    context: &Context,
    function: FunctionValue<'_>,
    effect: GcEffect,
    profile: ValidatedBackendProfile,
) {
    if effect == GcEffect::Managed {
        function.set_gc(GC_STRATEGY);
        function.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute("frame-pointer", profile.frame_pointer_attribute()),
        );
        function.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute(
                "disable-tail-calls",
                profile.disable_tail_calls_attribute(),
            ),
        );
    }
}

/// Scalarize aggregate root storage, build SSA values, then run RS4GC.
pub(crate) fn rewrite(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    module
        .run_passes(
            "function(sroa,mem2reg),rewrite-statepoints-for-gc",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("rewrite-statepoints-for-gc failed: {error}")))
}
