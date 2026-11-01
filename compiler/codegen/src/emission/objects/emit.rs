use super::*;

use super::callables::{PreparedCallableObject, emit_callables};

pub(super) fn emit_object_set_with_production<
    D: scoop_lir::StrongDescriptorReference + Sync,
    C: Clone + Sync,
    I: Clone + Sync,
>(
    input: &scoop_lir::ConeLirOutput,
    mut production: scoop_lir::ConeProductionSection<D, C, I>,
    profile: ValidatedBackendProfile,
    jobs: std::num::NonZeroUsize,
) -> Result<EmittedConeObjectSet<scoop_lir::ConeProductionSection<D, C, I>>, CodegenError> {
    let module = input.module();
    let expected_safepoints = statepoint::expectations(module)?;
    let metadata_safepoints = expected_safepoints.without_body_sites();
    let declarations = ConeDeclarations::new(module, &production)?;
    let symbol_plans = crate::symbol_plan::SymbolPlanIndex::new(production.canonical_definitions());
    let mut callables = emit_callables(
        module,
        &production,
        &symbol_plans,
        &declarations,
        expected_safepoints.into_functions(),
        profile,
        jobs,
    )?;
    let foundation = finalization::finalize_production(
        input,
        &mut production,
        callables
            .iter()
            .map(|(body, object)| (*body, &object.plan, &object.backend_constants)),
    )?;
    let partition = ScoopLirObjectPartitionV1::from_foundation(
        input,
        &foundation,
        production.canonical_definitions(),
    )
    .map_err(|error| CodegenError(error.to_string()))?;
    let metadata_context = Context::create();
    let machine = profile.create_target_machine()?;
    let metadata_llvm = prepare_non_callable_strong_llvm_module(
        &metadata_context,
        module,
        &production,
        &machine,
        profile,
        &metadata_safepoints,
    )?;
    let metadata =
        metadata_partition::MetadataModule::new(metadata_llvm, production.canonical_definitions())?;
    let mut metadata_members = metadata.map_partitions_parallel(
        &machine,
        profile,
        &partition,
        jobs,
        |units, llvm, machine| {
            let mut bytes = metadata_object::emit(machine, &llvm)?;
            atom_boundaries::materialize_global_linkages_v1(
                &mut bytes,
                module.meta.target_profile,
                production.canonical_definitions(),
                units.definition_plans(),
            )?;
            Ok(EmittedConeObjectMemberV1 {
                units: units.clone(),
                bytes,
                kind: EmittedConeObjectMemberKindV1::NonCallable,
            })
        },
    )?;
    let mut members = Vec::with_capacity(partition.objects().len());
    for units in partition.objects() {
        let member = match units.kind() {
            ScoopLirObjectKindV1::NonCallable => metadata_members
                .remove(&units.definition_plans()[0])
                .expect("every metadata object was prepared"),
            ScoopLirObjectKindV1::CallableBody(body) => {
                let prepared = callables
                    .remove(&body)
                    .expect("every callable object was prepared");
                finish_callable_object(
                    prepared,
                    units,
                    production.canonical_definitions(),
                    profile,
                    body,
                )?
            }
        };
        members.push(member);
    }
    Ok(EmittedConeObjectSet {
        optimization: profile.optimization(),
        target_selection: profile.lir_target_selection(),
        foundation,
        production,
        partition,
        members,
    })
}

fn finish_callable_object(
    prepared: PreparedCallableObject,
    units: &ScoopLirObjectUnitSetV1,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
    profile: ValidatedBackendProfile,
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<EmittedConeObjectMemberV1, CodegenError> {
    let PreparedCallableObject { mut bytes, .. } = prepared;
    let definition = units
        .definition_plans()
        .iter()
        .filter_map(|id| surface.plan(*id))
        .find(|plan| plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody)
        .ok_or_else(|| {
            CodegenError(format!(
                "callable object for {body} has no canonical symbol plan"
            ))
        })?;
    crate::callable_atom_boundaries::materialize_v1(
        &mut bytes,
        profile.lir_target_profile(),
        definition,
    )?;
    let registrations = units
        .definition_plans()
        .iter()
        .copied()
        .filter(|id| *id != definition.definition_plan())
        .collect::<Vec<_>>();
    atom_boundaries::materialize_global_linkages_v1(
        &mut bytes,
        profile.lir_target_profile(),
        surface,
        &registrations,
    )?;
    Ok(EmittedConeObjectMemberV1 {
        units: units.clone(),
        bytes,
        kind: EmittedConeObjectMemberKindV1::CallableBody { body },
    })
}
