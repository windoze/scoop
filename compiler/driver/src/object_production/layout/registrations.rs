use super::*;

impl PreparedLayoutObjects {
    pub(crate) fn callable_metadata(
        &self,
    ) -> (
        slib::CallableRegistrationDataV1,
        slib::SafepointRegistrationDataV1,
    ) {
        let callables = bind_callable_registrations_v1(
            self.builtins.clone(),
            self.production.callable_registrations().clone(),
        );
        let safepoints = bind_safepoint_registrations_v1(
            self.stackmaps.clone(),
            self.production.safepoint_registrations().clone(),
        );
        (callables, safepoints)
    }

    pub(crate) fn fingerprint_callable_objects(
        &self,
        callables: slib::CallableRegistrationDataV1,
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
) -> Result<slib::RegistrationObjectSetV2, BuiltinObjectProductionError> {
    let candidates = input.candidates();
    let production = &input.production;
    let (callables, safepoints) = input.callable_metadata();
    let safepoints = compute_strong_safepoint_fingerprints_v1(safepoints)
        .map_err(BuiltinObjectProductionError::SafepointFingerprints)?;

    let callables = input.fingerprint_callable_objects(callables, undefined)?;
    let callables = compute_strong_callable_fingerprints_v1(
        callables,
        production.canonical_callable_definitions(),
    )
    .map_err(BuiltinObjectProductionError::CallableFingerprints)?;

    let types = slib::bind_type_registrations_v1(
        input.builtins.clone(),
        production.type_registrations().clone(),
    );
    let types = slib::compute_strong_type_fingerprints_v2(
        types,
        production.canonical_shape_definitions(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::TypeFingerprints)?;

    let immortals = compute_strong_immortal_object_fingerprints_v1(
        production.immortal_registrations().clone(),
        production.canonical_shape_definitions(),
    )
    .map_err(BuiltinObjectProductionError::ImmortalObjectFingerprints)?;

    let storages = production.static_storage_registrations().clone();
    let storages = compute_strong_static_storage_shape_fingerprints_v1(storages)
        .map_err(BuiltinObjectProductionError::StaticStorageShapeFingerprints)?;
    let storages = compute_strong_static_storage_fingerprints_v1(
        storages,
        production.canonical_shape_definitions(),
    )
    .map_err(BuiltinObjectProductionError::StaticStorageFingerprints)?;

    let initializations = production.initialization_registrations().clone();
    let initializations = slib::compute_strong_initialization_fingerprints_v2(
        initializations,
        callables.body_objects(),
        production.canonical_shape_definitions(),
    )
    .map_err(BuiltinObjectProductionError::InitializationFingerprints)?;

    slib::collect_registration_objects_v2(
        safepoints,
        callables,
        types,
        immortals,
        storages,
        initializations,
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::RegistrationObjects)
}
