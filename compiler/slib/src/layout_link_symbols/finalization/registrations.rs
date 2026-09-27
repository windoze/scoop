use super::*;
use crate::StrongLinkRegistrationDependencyFingerprintError as DependencyError;
use crate::StrongLinkRegistrationLeafFingerprintError as LeafError;

pub(super) fn replay(
    objects: &ReplayedLayoutLinkObjectContentsV1,
    undefined: &FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    candidates: &[ScoopLirObjectCandidateV1<'_>],
    canonical: &lir::CanonicalCallableLirDefinitionsV1,
) -> Result<VerifiedStrongRegistrationPatchSetV2, LayoutLinkSymbolUseError> {
    let safepoints =
        compute_strong_safepoint_fingerprints_v1(objects.safepoints().clone(), candidates)
            .map_err(LeafError::Safepoints)?;
    let callables = objects.callables().clone();
    let callables = compute_layout_strong_callable_body_object_fingerprints_v1(
        callables,
        objects.stackmaps().clone(),
        undefined.clone(),
        candidates,
    )
    .map_err(DependencyError::CallableBodies)?;
    let callables = compute_strong_callable_fingerprints_v1(callables, canonical)
        .map_err(DependencyError::Callables)?;

    let types = compute_strong_type_registration_object_fingerprints_v2(
        objects.types().clone(),
        candidates,
    )
    .map_err(LeafError::Types)?;
    let types = compute_strong_type_dependency_fingerprints_v2(types, candidates)
        .map_err(DependencyError::TypeDependencies)?;
    let types = compute_strong_type_fingerprints_v2(types).map_err(DependencyError::Types)?;

    let immortals = compute_strong_immortal_object_registration_object_fingerprints_v1(
        objects.immortals().clone(),
        candidates,
    )
    .map_err(LeafError::ImmortalObjects)?;
    let immortals = compute_layout_strong_immortal_object_definition_fingerprints_v1(
        immortals,
        undefined.clone(),
        candidates,
    )
    .map_err(DependencyError::ImmortalObjectDefinitions)?;
    let immortals = compute_strong_immortal_object_fingerprints_v1(immortals)
        .map_err(DependencyError::ImmortalObjects)?;

    let storages = compute_strong_static_storage_registration_object_fingerprints_v1(
        objects.storages().clone(),
        candidates,
    )
    .map_err(LeafError::StaticStorages)?;
    let storages = compute_strong_static_storage_definition_fingerprints_v1(storages, candidates)
        .map_err(DependencyError::StaticStorageDefinitions)?;
    let storages = compute_strong_static_storage_shape_fingerprints_v1(storages)
        .map_err(DependencyError::StaticStorageShapes)?;
    let storages = compute_strong_static_storage_fingerprints_v1(storages)
        .map_err(DependencyError::StaticStorages)?;

    let initializations = compute_strong_initialization_registration_object_fingerprints_v2(
        objects.initializations().clone(),
        candidates,
    )
    .map_err(LeafError::InitializationUnits)?;
    let initializations =
        compute_strong_initialization_definition_fingerprints_v2(initializations, candidates)
            .map_err(DependencyError::InitializationDefinitions)?;
    let initializations =
        compute_strong_initialization_fingerprints_v2(initializations, callables.body_objects())
            .map_err(DependencyError::Initializations)?;
    Ok(patch_strong_registration_fingerprints_v2(
        safepoints,
        callables,
        types,
        immortals,
        storages,
        initializations,
        candidates,
    )
    .map_err(FinalError::RegistrationPatch)?)
}
