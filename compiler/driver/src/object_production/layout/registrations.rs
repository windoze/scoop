use super::*;

impl PreparedLayoutObjects {
    pub(crate) fn verify_callable_metadata(
        &self,
    ) -> Result<
        (
            slib::VerifiedStrongCallableRegistrationSetV1,
            slib::VerifiedStrongSafepointRegistrationSetV1,
        ),
        BuiltinObjectProductionError,
    > {
        let candidates = self.candidates();
        let callables = verify_strong_callable_registrations_v1(
            self.patch_sites.clone(),
            self.production.callable_registrations().clone(),
            &candidates,
        )
        .map_err(BuiltinObjectProductionError::CallableRegistrations)?;
        let safepoints = verify_strong_safepoint_registrations_v1(
            self.stackmaps.clone(),
            self.patch_sites.clone(),
            self.production.safepoint_registrations().clone(),
            &candidates,
        )
        .map_err(BuiltinObjectProductionError::SafepointRegistrations)?;
        Ok((callables, safepoints))
    }

    pub(crate) fn fingerprint_callable_objects(
        &self,
        callables: slib::VerifiedStrongCallableRegistrationSetV1,
        undefined: &slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    ) -> Result<slib::VerifiedStrongCallableBodyObjectFingerprintSetV1, BuiltinObjectProductionError>
    {
        slib::compute_layout_strong_callable_body_object_fingerprints_v1(
            callables,
            self.stackmaps.clone(),
            undefined.clone(),
            &self.candidates(),
        )
        .map_err(BuiltinObjectProductionError::CallableBodyFingerprints)
    }
}

pub(super) fn finalize(
    input: &PreparedLayoutObjects,
    undefined: &slib::FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
) -> Result<slib::VerifiedStrongRegistrationPatchSetV2, BuiltinObjectProductionError> {
    let candidates = input.candidates();
    let production = &input.production;
    let patches = &input.patch_sites;
    let (callables, safepoints) = input.verify_callable_metadata()?;
    let safepoints = compute_strong_safepoint_fingerprints_v1(safepoints)
        .map_err(BuiltinObjectProductionError::SafepointFingerprints)?;

    let callables = input.fingerprint_callable_objects(callables, undefined)?;
    let callables = compute_strong_callable_fingerprints_v1(
        callables,
        production.canonical_callable_definitions(),
    )
    .map_err(BuiltinObjectProductionError::CallableFingerprints)?;

    let types = slib::verify_strong_type_registrations_v2(
        patches.clone(),
        production.type_registrations().clone(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::TypeRegistrations)?;
    let types = slib::compute_strong_type_fingerprints_v2(
        types,
        production.canonical_shape_definitions(),
        undefined.clone(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::TypeFingerprints)?;

    let immortals = verify_strong_immortal_object_registrations_v1(
        patches.clone(),
        production.immortal_registrations().clone(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::ImmortalObjectRegistrations)?;
    let immortals = compute_strong_immortal_object_fingerprints_v1(
        immortals,
        production.canonical_shape_definitions(),
    )
    .map_err(BuiltinObjectProductionError::ImmortalObjectFingerprints)?;

    let storages = verify_strong_static_storage_registrations_v1(
        patches.clone(),
        production.static_storage_registrations().clone(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::StaticStorageRegistrations)?;
    let storages = compute_strong_static_storage_shape_fingerprints_v1(storages)
        .map_err(BuiltinObjectProductionError::StaticStorageShapeFingerprints)?;
    let storages = compute_strong_static_storage_fingerprints_v1(
        storages,
        production.canonical_shape_definitions(),
    )
    .map_err(BuiltinObjectProductionError::StaticStorageFingerprints)?;

    let initializations = slib::verify_strong_initialization_registrations_v2(
        patches.clone(),
        production.initialization_registrations().clone(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::InitializationRegistrations)?;
    let initializations = slib::compute_strong_initialization_fingerprints_v2(
        initializations,
        callables.body_objects(),
        production.canonical_shape_definitions(),
    )
    .map_err(BuiltinObjectProductionError::InitializationFingerprints)?;

    slib::patch_strong_registration_fingerprints_v2(
        safepoints,
        callables,
        types,
        immortals,
        storages,
        initializations,
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::RegistrationPatch)
}
