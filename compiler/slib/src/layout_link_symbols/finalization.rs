//! Reconstruct final bytes with the shared production algorithms.

use super::*;
use crate::StrongLinkObjectFinalizationError as FinalError;

mod registrations;

pub(super) fn replay(
    objects: &ReplayedLayoutLinkObjectContentsV1,
    undefined: &FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    input: &ReplayInputs<'_>,
) -> Result<VerifiedEntryPatchSetV2, LayoutLinkSymbolUseError> {
    let candidates = objects.objects().candidates();
    let registrations = registrations::replay(
        objects,
        undefined,
        &candidates,
        input.strong.canonical_callable_definitions(),
        input.strong.canonical_shape_definitions(),
    )?;

    let image = verify_cone_image_v1(
        objects.patch_sites().clone(),
        input.strong.image_plan().clone(),
        &candidates,
    )
    .map_err(FinalError::ImageValidation)?;
    let entry = verify_entry_production_v1(
        objects.patch_sites().clone(),
        input.strong.entry_plan().clone(),
        &candidates,
    )
    .map_err(FinalError::EntryValidation)?;
    let runtime = compute_runtime_image_fingerprint_v2(
        image,
        registrations,
        input.manifest.compatibility().clone(),
    )
    .map_err(FinalError::ImageFingerprint)?;
    if input.manifest.semantic_fingerprints().runtime_image()
        != crate::FingerprintAvailability::Available(runtime.fingerprint())
    {
        return Err(LayoutLinkSymbolUseError::RuntimeFingerprintMismatch);
    }
    let runtime = patch_runtime_image_fingerprint_v2(runtime).map_err(FinalError::ImagePatch)?;
    let finalized = patch_entry_production_v2(runtime, entry).map_err(FinalError::EntryPatch)?;
    crate::link_decode::verify_reconstructed_scoop_objects(objects.objects(), &finalized)
        .map_err(FinalError::FinalObjectMismatch)?;
    input
        .link
        .production_manifest_wire()
        .replay_runtime_projection(
            input.strong.registration_identities(),
            finalized.runtime_images().fingerprint(),
        )?;
    Ok(finalized)
}
