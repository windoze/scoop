//! Registrations and Context cells selected together with one callable body.

use super::*;

pub(crate) fn emit_callable_metadata_v1<'ctx, D, C, I>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    profile: ValidatedBackendProfile,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<EmittedStrongRuntimeMetadataV1, CodegenError> {
    let callables = emit_strong_callable_registrations_v1(
        context,
        llvm,
        production.registration_production().callables(),
        production.canonical_definitions(),
        profile,
        body,
    )?;
    let safepoints = emit_strong_safepoint_registrations_v1(
        context,
        llvm,
        production.registration_production().safepoints(),
        body,
    )?;
    let mut patches = Vec::new();
    let mut atoms = callables.context_atoms.clone();
    for registration in callables.registrations() {
        {
            let patch = registration.body_definition_patch();
            record_patch(production, &mut patches, patch.into_parts())?;
        }
        atoms.push(GlobalAtomMaterializationV1::new(
            registration.body_definition_patch().atom(),
            registration.descriptor(),
        ));
    }
    for registration in safepoints.registrations() {
        {
            let patch = registration.normalized_stackmap_patch();
            record_patch(production, &mut patches, patch.into_parts())?;
        }
        atoms.push(GlobalAtomMaterializationV1::new(
            registration.normalized_stackmap_patch().atom(),
            registration.descriptor(),
        ));
    }
    emit_global_atom_boundaries_v1(llvm, target_data, production.canonical_definitions(), atoms)?;
    patches.sort_unstable_by_key(|patch| patch.intent);
    Ok(EmittedStrongRuntimeMetadataV1 {
        producer: callables.producer(),
        patch_locations: patches,
    })
}
