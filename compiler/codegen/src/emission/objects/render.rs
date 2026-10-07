use super::*;
use std::collections::BTreeMap;

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
    let mut production = input
        .build_production_section(coordinate.clone(), direct_dependencies, entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    let expected_safepoints = statepoint::expectations(module)?;
    let expected_eh = artifact::eh_expectations(module)?;
    let machine = profile.create_target_machine()?;
    let mut callables = BTreeMap::new();
    for function in module.callable_bodies() {
        let context = Context::create();
        let body = function.callable_body.id();
        let selected_safepoints = expected_safepoints.for_function(function.symbol())?;
        let selected_eh = expected_eh.for_function(function.symbol());
        let prepared = prepare_callable_strong_llvm_module(
            &context,
            module,
            &production,
            &machine,
            profile,
            (&selected_safepoints, &selected_eh),
            body,
        )?;
        callables.insert(
            body,
            (prepared.llvm.print_to_string().to_string(), prepared.plan),
        );
    }
    let foundation = finalization::finalize_production(
        input,
        &mut production,
        callables.iter().map(|(body, (_, plan))| (*body, plan)),
    )?;
    let partition = ScoopLirObjectPartitionV1::from_foundation(
        input,
        &foundation,
        production.canonical_definitions(),
    )
    .map_err(|error| CodegenError(error.to_string()))?;
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
            let llvm_ir = match units.kind() {
                ScoopLirObjectKindV1::NonCallable => metadata
                    .project(&machine, profile, units.definition_plans())?
                    .0
                    .print_to_string()
                    .to_string(),
                ScoopLirObjectKindV1::CallableBody(body) => {
                    callables
                        .remove(&body)
                        .expect("every callable module was rendered")
                        .0
                }
            };
            Ok(RenderedConeObjectModuleV1 {
                units: units.clone(),
                llvm_ir,
            })
        })
        .collect()
}
