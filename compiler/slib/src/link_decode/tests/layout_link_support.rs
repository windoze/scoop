use super::*;

/// Exercise finalization with the V2 type and initialization semantic surface.
pub(crate) fn verified_layout_code_link_object_members() -> crate::FinalCodeObjectSetV2 {
    let fixture = link_object_fixture();
    let (production, previous, _, _) = finalized_link_object_fixture();
    let (canonical, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = ConeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let objects = [crate::ScoopLirObjectCandidateV1::new(
        fixture.plan.scoop_lir_members()[0].member_id(),
        &fixture.bytes,
    )];
    let builtins = previous.entry().builtins();
    let surface = scoop_lir::StrongRegistrationProductionSurfaceV2::empty(
        selection().target(),
        &foundation,
        production.digest_finalization_plan(),
    )
    .unwrap();
    let types = crate::bind_type_registrations_v1(builtins.clone(), surface.types().clone());
    let types = crate::compute_strong_type_fingerprints_v2(
        types,
        production.canonical_shape_definitions(),
        &objects,
    )
    .unwrap();
    let initializations = surface.initialization_units().clone();
    let previous_image = previous.runtime_image();
    let registrations = previous_image.registrations();
    let initializations = crate::compute_strong_initialization_fingerprints_v2(
        initializations,
        registrations.callables().body_objects(),
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let patched = crate::collect_registration_objects_v2(
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
    let final_objects = crate::FinalRuntimeObjectsV1::new(image, previous.entry().clone());
    assert_eq!(final_objects.objects(), previous.objects());
    let member_plan = &fixture.plan.scoop_lir_members()[0];
    let member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        crate::SlibMemberRole::LinkObject,
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();
    crate::build_code_link_object_members_v2(final_objects, &[member.record().content().clone()])
        .unwrap()
}
