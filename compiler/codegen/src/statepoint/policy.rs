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

/// Run the qualified ordinary passes and retain their logical root identities.
pub(crate) fn optimize(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
    expected: &ExpectedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<(), CodegenError> {
    address_bits::protect(module, profile.managed_address_space_contract())?;
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
        if profile.optimization() == scoop_lir::OptimizationMode::Release {
            storage::promote(function)?;
        }
        // Textual InstCombine enables a testing-only fixpoint check by default.
        // Use its normal bounded iteration policy in both release paths.
        let passes = match (profile.optimization(), no_gc) {
            (scoop_lir::OptimizationMode::Debug, _) => {
                "sroa,mem2reg,sccp,instcombine<no-verify-fixpoint>,early-cse,unreachableblockelim"
            }
            (scoop_lir::OptimizationMode::Release, true) => {
                "sroa,mem2reg,instcombine<no-verify-fixpoint>,early-cse,reassociate,constraint-elimination,correlated-propagation,sccp,instcombine<no-verify-fixpoint>,gvn<no-pre;no-load-pre;no-split-backedge-load-pre>,bdce,memcpyopt,dse,adce,early-cse<memssa>,unreachableblockelim,simplifycfg,loop-simplify,lcssa,loop-mssa(loop-rotate,licm,simple-loop-unswitch<nontrivial;trivial>),loop(indvars),loop-load-elim,loop-unroll<O2;full-unroll-max=4;no-partial;no-peeling;no-profile-peeling;no-runtime;no-upperbound>,loop-vectorize,slp-vectorizer,instcombine<no-verify-fixpoint>,simplifycfg,dse,adce"
            }
            (scoop_lir::OptimizationMode::Release, false) => {
                // No PRE or backedge splitting: sites and logical root restores
                // must stay unique until the physical root plan is finalized.
                concat!(
                    "sroa,mem2reg,instcombine<no-verify-fixpoint>,early-cse,reassociate,",
                    "constraint-elimination,correlated-propagation,sccp,",
                    "instcombine<no-verify-fixpoint>,",
                    "gvn<no-pre;no-load-pre;no-split-backedge-load-pre>,bdce,memcpyopt,dse,adce,",
                    "simplifycfg<no-speculate-blocks;no-hoist-common-insts;no-sink-common-insts>,",
                    "early-cse<memssa>,unreachableblockelim"
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
        if profile.optimization() == scoop_lir::OptimizationMode::Release && !no_gc {
            bounded_loops::optimize(function, machine)?;
            // GVN/MemorySSA can make earlier dynamic bounds equal. Consume
            // those path constraints before finalizing roots and sites.
            finish_managed_optimization(function, machine)?;
        }
    }
    crate::gc_regions::inline_array_queries(module, machine)?;
    crate::gc_card_groups::inline_fast_paths(module, machine)?;
    aggregate_values::expose(module, profile.managed_address_space_contract())?;
    finalization::finalize(module, expected, profile)
}

pub(super) fn finish_managed_optimization(
    function: FunctionValue<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    function
        .run_passes(
            "constraint-elimination,sccp,unreachableblockelim",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("final LLVM constraint cleanup failed: {error}")))
}

pub(crate) fn lower(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
) -> Result<LoweredSafepoints, CodegenError> {
    module
        .run_passes(
            "rewrite-statepoints-for-gc",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("rewrite-statepoints-for-gc failed: {error}")))?;
    address_bits::lower(module)?;
    // Instruction selection also simplifies terminal blocks. Finalize only
    // after that cleanup, so dead relocates before unreachable do not inflate
    // registration counts. Returning calls retain their call-duration roots.
    lowered::discard_terminal_relocations(module);
    Ok(lowered::collect(module))
}

#[cfg(test)]
pub(crate) fn rewrite(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
    expected: &ExpectedSafepoints,
    profile: ValidatedBackendProfile,
) -> Result<LoweredSafepoints, CodegenError> {
    optimize(module, machine, expected, profile)?;
    lower(module, machine)
}
