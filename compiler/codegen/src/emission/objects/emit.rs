use super::*;
use std::collections::BTreeMap;

struct PreparedCallableObject {
    path: std::path::PathBuf,
    runtime_metadata: EmittedStrongRuntimeMetadataV1,
    plan: CallableCodePlan,
}

pub(super) fn emit_object_set_with_production<
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
    I: Clone,
>(
    input: &scoop_lir::ConeLirOutput,
    mut production: scoop_lir::ConeProductionSection<D, C, I>,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSet<scoop_lir::ConeProductionSection<D, C, I>>, CodegenError> {
    let module = input.module();
    let expected_safepoints = statepoint::expectations(module)?;
    let expected_eh = artifact::eh_expectations(module)?;
    let machine = profile.create_target_machine()?;
    std::fs::create_dir_all(temporary_parent).map_err(|error| {
        CodegenError(format!(
            "cannot create object temporary parent {}: {error}",
            temporary_parent.display()
        ))
    })?;
    let backing = tempfile::Builder::new()
        .prefix("scoop-lir-")
        .tempdir_in(temporary_parent)
        .map_err(|error| {
            CodegenError(format!(
                "cannot create immutable object backing under {}: {error}",
                temporary_parent.display()
            ))
        })?;
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
        let path = backing.path().join(format!("{body}.o"));
        write_object(&machine, &prepared.llvm, &path)?;
        callables.insert(
            body,
            PreparedCallableObject {
                path,
                runtime_metadata: prepared.runtime_metadata,
                plan: prepared.plan,
            },
        );
    }
    let foundation = finalization::finalize_production(
        input,
        &mut production,
        callables.iter().map(|(body, object)| (*body, &object.plan)),
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
    let mut members = Vec::with_capacity(partition.objects().len());
    for units in partition.objects() {
        let member = match units.kind() {
            ScoopLirObjectKindV1::NonCallable => {
                let path = backing
                    .path()
                    .join(format!("{}.o", units.definition_plans()[0]));
                let selected_safepoints = expected_safepoints.without_body_sites();
                let selected_eh = expected_eh.without_body_metadata();
                let (llvm, runtime_metadata) =
                    metadata.project(&machine, profile, units.definition_plans())?;
                write_object(&machine, &llvm, &path)?;
                atom_boundaries::materialize_global_linkages_v1(
                    &path,
                    module.meta.target_profile,
                    production.canonical_definitions(),
                    units.definition_plans(),
                )?;
                verify_and_seal_object(&path, profile, &selected_safepoints, &selected_eh)?;
                let digest_patches =
                    object_materialization::resolve_digest_patch_materializations_v1(
                        &path,
                        module.meta.target_profile,
                        production.canonical_definitions(),
                        &runtime_metadata,
                    )?;
                EmittedConeObjectMemberV1 {
                    units: units.clone(),
                    path,
                    kind: EmittedConeObjectMemberKind::NonCallable {
                        runtime_metadata,
                        digest_patches,
                    },
                }
            }
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
    let mut patches = members
        .iter()
        .flat_map(|member| member.digest_patches())
        .map(|patch| patch.location())
        .collect::<Vec<_>>();
    patches.sort_unstable_by_key(|patch| patch.intent());
    runtime_metadata_v1::validate_patch_coverage(&production, &patches)?;
    Ok(EmittedConeObjectSet {
        optimization: profile.optimization(),
        target_selection: profile.lir_target_selection(),
        foundation,
        production,
        partition,
        members,
        backing,
    })
}

fn finish_callable_object(
    prepared: PreparedCallableObject,
    units: &ScoopLirObjectUnitSetV1,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
    profile: ValidatedBackendProfile,
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<EmittedConeObjectMemberV1, CodegenError> {
    let PreparedCallableObject {
        path,
        runtime_metadata,
        plan,
    } = prepared;
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
    if let Err(error) = crate::callable_atom_boundaries::materialize_v1(
        &path,
        profile.lir_target_profile(),
        definition,
        body,
    ) {
        return Err(discard_invalid_object(&path, error));
    }
    let registrations = units
        .definition_plans()
        .iter()
        .copied()
        .filter(|id| *id != definition.definition_plan())
        .collect::<Vec<_>>();
    atom_boundaries::materialize_global_linkages_v1(
        &path,
        profile.lir_target_profile(),
        surface,
        &registrations,
    )?;
    verify_and_seal_object(&path, profile, &plan.safepoints, &plan.eh)?;
    let digest_patches = object_materialization::resolve_digest_patch_materializations_v1(
        &path,
        profile.lir_target_profile(),
        surface,
        &runtime_metadata,
    )?;
    Ok(EmittedConeObjectMemberV1 {
        units: units.clone(),
        path,
        kind: EmittedConeObjectMemberKind::CallableBody {
            body,
            runtime_metadata,
            digest_patches,
        },
    })
}
