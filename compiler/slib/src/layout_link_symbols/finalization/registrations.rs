use super::*;
use crate::StrongLinkRegistrationDependencyFingerprintError as DependencyError;
use crate::StrongLinkRegistrationLeafFingerprintError as LeafError;

pub(super) fn replay(
    objects: &ReplayedLayoutLinkObjectContentsV1,
    undefined: &FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    candidates: &[ScoopLirObjectCandidateV1<'_>],
    canonical: &lir::CanonicalCallableAbisV1,
    shapes: &lir::CanonicalShapeAbisV1,
) -> Result<VerifiedStrongRegistrationPatchSetV2, LayoutLinkSymbolUseError> {
    let safepoints = compute_strong_safepoint_fingerprints_v1(objects.safepoints().clone())
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

    let types = compute_strong_type_fingerprints_v2(
        objects.types().clone(),
        shapes,
        undefined.clone(),
        candidates,
    )
    .map_err(DependencyError::Types)?;

    let immortals = objects.immortals().clone();
    let immortals = compute_strong_immortal_object_fingerprints_v1(immortals, shapes)
        .map_err(DependencyError::ImmortalObjects)?;

    let storages = objects.storages().clone();
    let storages = compute_strong_static_storage_shape_fingerprints_v1(storages)
        .map_err(DependencyError::StaticStorageShapes)?;
    let storages = compute_strong_static_storage_fingerprints_v1(storages, shapes)
        .map_err(DependencyError::StaticStorages)?;

    let initializations = objects.initializations().clone();
    let initializations = compute_strong_initialization_fingerprints_v2(
        initializations,
        callables.body_objects(),
        shapes,
    )
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
