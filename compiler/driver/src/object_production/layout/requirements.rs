use super::*;

pub(crate) fn complete_requirements(
    input: &PreparedLayoutObjects,
    native: &scoop_lir::CanonicalNativeExternalRequirementSurfaceV1,
    shape: &slib::VerifiedExternalShapeRequirementClosureV1<'_>,
) -> Result<slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1, BuiltinObjectProductionError>
{
    let bridge_plan = input.production.generated_bridge_plan();
    let current = verify_current_cone_undefined_requirements_v1(
        input.patch_sites.builtins().strong_relocations().clone(),
        bridge_plan.clone(),
    )
    .map_err(BuiltinObjectProductionError::CurrentConeRequirements)?;
    let source =
        slib::verify_source_external_requirements_after_external_shape_v1(shape, native.clone())
            .map_err(BuiltinObjectProductionError::SourceExternalRequirements)?;
    let runtime = verify_runtime_and_eh_requirements_v1(source, input.target_selection)
        .map_err(BuiltinObjectProductionError::RuntimeAndEhRequirements)?;
    let bridges = verify_generated_c_bridge_semantics_v1(
        input.patch_sites.clone(),
        bridge_plan.clone(),
        native.clone(),
        &input.c_bridge_profile,
    )
    .map_err(BuiltinObjectProductionError::GeneratedBridgeSemantics)?;
    let external = verify_c_bridge_target_support_requirements_v1(runtime, bridges)
        .map_err(BuiltinObjectProductionError::CBridgeTargetSupportRequirements)?;
    let external = seal_builtin_object_external_requirements_v1(external)
        .map_err(BuiltinObjectProductionError::UnclassifiedExternalRequirement)?;
    slib::finalize_layout_partitioned_undefined_symbol_requirements_v1(current, external, shape)
        .map_err(BuiltinObjectProductionError::UndefinedSymbols)
}
