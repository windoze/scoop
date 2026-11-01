use super::*;
use std::collections::BTreeMap;

struct PreparedCallableObject {
    bytes: Vec<u8>,
    plan: CallableCodePlan,
    backend_constants: std::collections::BTreeSet<scoop_lir::CallableConstantKind>,
}

pub(super) fn emit_object_set_with_production<
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
    I: Clone,
>(
    input: &scoop_lir::ConeLirOutput,
    mut production: scoop_lir::ConeProductionSection<D, C, I>,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSet<scoop_lir::ConeProductionSection<D, C, I>>, CodegenError> {
    let module = input.module();
    let expected_safepoints = statepoint::expectations(module)?;
    let metadata_safepoints = expected_safepoints.without_body_sites();
    let mut callable_safepoints = expected_safepoints.into_functions();
    let declarations = ConeDeclarations::new(module, &production)?;
    let machine = profile.create_target_machine()?;
    let mut callables = BTreeMap::new();
    let symbol_plans = crate::symbol_plan::SymbolPlanIndex::new(production.canonical_definitions());
    let callable_definitions = production
        .canonical_definitions()
        .plans()
        .iter()
        .filter(|plan| plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody)
        .map(|plan| {
            let scoop_lir::StrongDefinitionEntityKind::CallableBody(body) = plan.owner().kind()
            else {
                unreachable!("callable definition has a typed body owner");
            };
            (body, plan)
        })
        .collect::<BTreeMap<_, _>>();
    let context = Context::create();
    for function in module.callable_bodies() {
        let body = function.callable_body.id();
        let selected_safepoints = callable_safepoints
            .remove(function.symbol())
            .expect("the manifest collects every callable body");
        let prepared = prepare_callable_strong_llvm_module(
            &context,
            module,
            &production,
            &symbol_plans,
            &machine,
            profile,
            &selected_safepoints,
            function,
            &declarations,
        )?;
        let mut bytes = emit_object(&machine, &prepared.llvm)?;
        if profile.lir_target_profile().native_object_format()
            == scoop_lir::NativeObjectFormat::Coff64
        {
            crate::coff_object::merge_callable_constants(&mut bytes)?;
        }
        let backend_constants = crate::callable_constants::present(
            &bytes,
            profile.lir_target_profile(),
            callable_definitions[&body],
        )?;
        callables.insert(
            body,
            PreparedCallableObject {
                bytes,
                plan: prepared.plan,
                backend_constants,
            },
        );
    }
    drop(context);
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
    let mut metadata_members =
        metadata.map_partitions(&machine, profile, &partition, |units, llvm| {
            let mut bytes = metadata_object::emit(&machine, &llvm)?;
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
        })?;
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
