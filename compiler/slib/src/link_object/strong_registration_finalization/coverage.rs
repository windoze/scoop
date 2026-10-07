use super::*;

pub(super) fn validate_proof_coverage<D, C, I>(
    safepoints: &VerifiedStrongSafepointFingerprintSetV1,
    callables: &VerifiedStrongCallableFingerprintSetV1,
    types: &VerifiedStrongTypeFingerprintSetV1<D, C>,
    immortal_objects: &VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: &VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: &VerifiedStrongInitializationFingerprintSetV1<I>,
) -> Result<(), StrongRegistrationPatchError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let verified_safepoints = safepoints.registrations().registrations();
    if verified_safepoints.len() != safepoints.registrations().plan().registrations().len()
        || verified_safepoints.len() != safepoints.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let verified_callables = callables.body_objects().registrations().registrations();
    if verified_callables.len()
        != callables
            .body_objects()
            .registrations()
            .plan()
            .registrations()
            .len()
        || verified_callables.len() != callables.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let type_registrations = types.registrations();
    if type_registrations.registrations().len() != type_registrations.plan().registrations().len()
        || type_registrations.registrations().len() != types.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let immortal_registrations = immortal_objects.registrations();
    if immortal_registrations.registrations().len()
        != immortal_registrations.plan().registrations().len()
        || immortal_registrations.registrations().len() != immortal_objects.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let static_storage_registrations = static_storages.shapes().registrations();
    if static_storage_registrations.registrations().len()
        != static_storage_registrations.plan().registrations().len()
        || static_storage_registrations.registrations().len()
            != static_storages.shapes().fingerprints().len()
        || static_storage_registrations.registrations().len()
            != static_storages.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    let initialization_registrations = initializations.registrations();
    let initialization_count = initialization_registrations.registrations().len();
    if initialization_count != initialization_registrations.plan().registrations().len()
        || initialization_count != initializations.fingerprints().len()
    {
        return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
    }
    for computed in initializations.fingerprints() {
        let gateway = (
            computed.gateway_body(),
            computed.gateway_definition_node(),
            computed.gateway_definition(),
        );
        let (Some(body), Some(node), Some(definition)) = gateway else {
            if gateway == (None, None, None) {
                continue;
            }
            return Err(StrongRegistrationPatchError::ProofCoverageMismatch);
        };
        let Some(callable) = callables
            .fingerprints()
            .iter()
            .find(|candidate| candidate.body() == body)
        else {
            return Err(StrongRegistrationPatchError::ProofMismatch);
        };
        if callable.body_definition_node() != node || callable.body_definition() != definition {
            return Err(StrongRegistrationPatchError::ProofMismatch);
        }
    }
    Ok(())
}
