use super::*;

/// Render every physical strong object module without writing artifacts.
pub fn render_llvm_ir_members(
    input: &scoop_lir::ConeLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    direct_dependencies: &[scoop_lir::ConeIdentity],
    entry_source: scoop_lir::EntryProductionSourceV1,
    profile: ValidatedBackendProfile,
) -> Result<Vec<RenderedConeObjectModuleV1>, CodegenError> {
    validate_object_set_input(input, profile)?;
    let module = input.module();
    let production = input
        .build_production_section(coordinate.clone(), direct_dependencies, entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    let partition =
        ScoopLirObjectPartitionV1::from_input(input, production.canonical_definitions())
            .map_err(|error| CodegenError(error.to_string()))?;
    let expected_safepoints = statepoint::expectations(module)?;
    let machine = profile.create_target_machine()?;
    let metadata_context = Context::create();
    let (metadata_llvm, metadata) = prepare_non_callable_strong_llvm_module(
        &metadata_context,
        module,
        &production,
        &machine,
        profile,
        &expected_safepoints.without_body_sites(),
    )?;
    let metadata = metadata_partition::MetadataModule::new(
        metadata_llvm,
        metadata,
        production.canonical_definitions(),
    )?;
    partition
        .objects()
        .iter()
        .map(|units| {
            let context = Context::create();
            let llvm_ir = match units.kind() {
                ScoopLirObjectKindV1::NonCallable => metadata
                    .project(&machine, profile, units.definition_plans())?
                    .0
                    .print_to_string()
                    .to_string(),
                ScoopLirObjectKindV1::CallableBody(body) => {
                    let function = module
                        .callable_bodies()
                        .find(|function| function.callable_body.id() == body)
                        .ok_or_else(|| {
                            CodegenError(format!(
                                "strong object partition selected missing callable body {body}"
                            ))
                        })?;
                    let selected = expected_safepoints.for_function(function.symbol())?;
                    prepare_callable_strong_llvm_module(
                        &context,
                        module,
                        &production,
                        &machine,
                        profile,
                        &selected,
                        body,
                    )?
                    .0
                    .print_to_string()
                    .to_string()
                }
            };
            Ok(RenderedConeObjectModuleV1 {
                units: units.clone(),
                llvm_ir,
            })
        })
        .collect()
}
