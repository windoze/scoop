use super::*;

pub(crate) fn verified_code_link_object_members() -> crate::VerifiedCodeLinkObjectMemberSetV1 {
    let (_, final_objects, _, _) = finalized_link_object_fixture();
    let plan = link_object_plan();
    let member_plan = &plan.scoop_lir_members()[0];
    let member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();

    crate::verify_code_link_object_members_v1(final_objects, &[member.record().clone()]).unwrap()
}

/// Replay the versioned registration path from provisional bytes. Only the
/// unchanged registration domains reuse V1 proofs; type and initialization
/// proofs are independently constructed with the V2 semantic surface.
pub(crate) fn verified_layout_code_link_object_members() -> crate::VerifiedCodeLinkObjectMemberSetV2
{
    let fixture = link_object_fixture();
    let (production, previous, _, _) = finalized_link_object_fixture();
    let (canonical, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = ConeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let objects = [crate::ScoopLirObjectCandidateV1::new(
        fixture.plan.scoop_lir_members()[0].member_id(),
        &fixture.bytes,
    )];
    let patch_sites = previous.entry().patch_sites();
    let surface = scoop_lir::StrongRegistrationProductionSurfaceV2::empty(
        selection().target(),
        &foundation,
        production.digest_finalization_plan(),
    )
    .unwrap();
    let types = crate::verify_strong_type_registrations_v2(
        patch_sites.clone(),
        surface.types().clone(),
        &objects,
    )
    .unwrap();
    let requirements = previous
        .runtime_images()
        .fingerprint()
        .registrations()
        .callables()
        .body_objects()
        .object_definition_requirements()
        .clone();
    let types = crate::compute_strong_type_fingerprints_v2(
        types,
        production.canonical_shape_definitions(),
        requirements,
        &objects,
    )
    .unwrap();
    let initializations = crate::verify_strong_initialization_registrations_v2(
        patch_sites.clone(),
        surface.initialization_units().clone(),
        &objects,
    )
    .unwrap();
    let initializations = crate::compute_strong_initialization_registration_object_fingerprints_v2(
        initializations,
        &objects,
    )
    .unwrap();
    let initializations =
        crate::compute_strong_initialization_definition_fingerprints_v2(initializations, &objects)
            .unwrap();
    let previous_image = previous.runtime_images().fingerprint();
    let registrations = previous_image.registrations();
    let initializations = crate::compute_strong_initialization_fingerprints_v2(
        initializations,
        registrations.callables().body_objects(),
    )
    .unwrap();
    let patched = crate::patch_strong_registration_fingerprints_v2(
        registrations.safepoints().clone(),
        registrations.callables().clone(),
        types,
        registrations.immortal_objects().clone(),
        registrations.static_storages().clone(),
        initializations,
        &objects,
    )
    .unwrap();
    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::CROSS_CONE_GENERIC)
            .unwrap();
    let image = crate::compute_runtime_image_fingerprint_v2(
        previous_image.image().clone(),
        patched,
        compatibility,
    )
    .unwrap();
    let image = crate::patch_runtime_image_fingerprint_v2(image).unwrap();
    let final_objects = crate::patch_entry_production_v2(image, previous.entry().clone()).unwrap();
    assert_eq!(final_objects.objects(), previous.objects());
    let member_plan = &fixture.plan.scoop_lir_members()[0];
    let member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();
    crate::verify_code_link_object_members_v2(final_objects, &[member.record().clone()]).unwrap()
}
