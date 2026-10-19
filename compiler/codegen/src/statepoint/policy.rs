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
    let site_owners = expected
        .sites
        .values()
        .map(|site| site.function.as_str())
        .collect::<BTreeSet<_>>();
    for function in module.get_functions() {
        if function.count_basic_blocks() == 0 {
            continue;
        }
        let name = function
            .get_name()
            .to_str()
            .map_err(|error| CodegenError(format!("LLVM function name: {error}")))?;
        let no_gc =
            expected.functions.get(name) == Some(&GcEffect::NoGc) && !site_owners.contains(name);
        // Textual InstCombine enables a testing-only fixpoint check by default.
        // Use its normal bounded iteration policy in both release paths.
        let passes = match (profile.optimization(), no_gc) {
            (scoop_lir::OptimizationMode::Debug, _) => "sroa,mem2reg,sccp,unreachableblockelim",
            (scoop_lir::OptimizationMode::Release, true) => {
                "sroa,mem2reg,instcombine<no-verify-fixpoint>,early-cse,simplifycfg,reassociate,loop-simplify,lcssa,loop-mssa(loop-rotate,licm),loop(indvars),loop-vectorize,slp-vectorizer,instcombine<no-verify-fixpoint>,simplifycfg,dse,adce"
            }
            (scoop_lir::OptimizationMode::Release, false) => {
                // No PRE or backedge splitting: sites and logical root restores
                // must stay unique until the physical root plan is finalized.
                concat!(
                    "sroa,mem2reg,instcombine<no-verify-fixpoint>,early-cse,reassociate,",
                    "gvn<no-pre;no-load-pre;no-split-backedge-load-pre>,bdce,dse,adce,",
                    "simplifycfg<no-speculate-blocks;no-hoist-common-insts;no-sink-common-insts>,",
                    "early-cse<memssa>,sccp,unreachableblockelim"
                )
            }
        };
        function
            .run_passes(passes, machine, PassBuilderOptions::create())
            .map_err(|error| {
                CodegenError(format!(
                    "ordinary LLVM lowering of `{name}` failed: {error}"
                ))
            })?;
    }
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
