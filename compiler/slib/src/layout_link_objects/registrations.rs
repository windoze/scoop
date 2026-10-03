use super::*;

pub(super) fn replay<'input>(
    objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    generated: Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    patches: VerifiedScoopLirDigestPatchSiteSetV1,
    stackmaps: VerifiedScoopLirStackmapSetV1,
    strong: &lir::ConeProductionSectionV2,
) -> Result<ReplayedLayoutLinkObjectContentsV1, LayoutLinkObjectContentsError> {
    let candidates = objects.candidates();

    macro_rules! verify {
        ($plan:ident, $verifier:ident) => {{
            let plan = strong.$plan();
            $verifier(patches.clone(), plan.clone(), &candidates)?
        }};
    }
    let callables = verify!(
        callable_registrations,
        verify_strong_callable_registrations_v1
    );
    let types = verify!(type_registrations, verify_strong_type_registrations_v2);
    let immortals = verify!(
        immortal_registrations,
        verify_strong_immortal_object_registrations_v1
    );
    let storages = verify!(
        static_storage_registrations,
        verify_strong_static_storage_registrations_v1
    );
    let initializations = verify!(
        initialization_registrations,
        verify_strong_initialization_registrations_v2
    );
    let plan = strong.safepoint_registrations();
    let safepoints =
        verify_strong_safepoint_registrations_v1(stackmaps, patches, plan.clone(), &candidates)?;
    let path = scoop_wire::WirePath::root();
    let mut generated_bytes = Vec::new();
    scoop_wire::allocation::try_reserve(&mut generated_bytes, generated.len(), &path)?;
    for object in generated {
        let mut bytes = Vec::new();
        scoop_wire::allocation::try_reserve(&mut bytes, object.bytes().len(), &path)?;
        bytes.extend_from_slice(object.bytes());
        generated_bytes.push((object.member(), bytes));
    }
    Ok(ReplayedLayoutLinkObjectContentsV1 {
        objects,
        generated: generated_bytes,
        safepoints,
        callables,
        types,
        immortals,
        storages,
        initializations,
    })
}
