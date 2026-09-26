use super::*;

pub(super) fn complete(
    objects: &ReplayedLayoutLinkObjectContentsV1,
    input: &ReplayInputs<'_>,
    native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
    shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
) -> Result<FinalizedLayoutUndefinedSymbolRequirementPartitionsV1, LayoutLinkSymbolUseError> {
    let current = verify_current_cone_undefined_requirements_v1(
        objects
            .patch_sites()
            .builtins()
            .strong_relocations()
            .clone(),
        input.strong.generated_bridge_plan().clone(),
    )?;

    let source =
        verify_source_external_requirements_after_external_shape_v1(shape, native.clone())?;

    let runtime = verify_runtime_and_eh_requirements_v1(source, input.selection)?;

    let bridges = verify_generated_c_bridge_semantics_v1(
        objects.patch_sites().clone(),
        input.strong.generated_bridge_plan().clone(),
        native.clone(),
        input.profile,
    )?;

    let external = verify_c_bridge_target_support_requirements_v1(runtime, bridges)?;
    let external = seal_builtin_object_external_requirements_v1(external)?;

    Ok(finalize_layout_partitioned_undefined_symbol_requirements_v1(current, external, shape)?)
}
