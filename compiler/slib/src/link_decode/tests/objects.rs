use super::*;

pub(super) fn link_object_plan() -> PlannedLinkObjectMemberSetV1 {
    let (canonical, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = ConeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let partition = ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
    let units = crate::CanonicalScoopLirObjectUnitSetV1::new(
        partition.scoop_lir_definition_plans().to_vec(),
    )
    .unwrap();
    PlannedLinkObjectMemberSetV1::new(&partition, vec![units], Vec::new()).unwrap()
}

pub(super) struct LinkObjectFixture {
    pub(super) bytes: Vec<u8>,
    pub(super) checked_offset: u64,
    pub(super) plan: PlannedLinkObjectMemberSetV1,
    pub(super) builtins: crate::VerifiedBuiltinObjectStrongRelocationSetV1,
}

pub(super) fn link_object_fixture() -> LinkObjectFixture {
    let (canonical, production) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = ConeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let surface = ObjectSymbolSurfaceV1::from_foundation(&foundation).unwrap();
    let plan = link_object_plan();
    let symbols =
        PlannedStrongObjectSymbolSetV1::new(selection().target(), &surface, &plan).unwrap();
    let (bytes, checked_offset) = crate::link_object::empty_image_object_for_link_decode_test(
        &symbols.members()[0],
        production.image_plan(),
    );
    let bridge_plan = production.generated_bridge_plan().clone();
    let profile = c_bridge_profile();
    let bridge_production =
        CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let bridge_production = crate::verify_c_bridge_production_envelopes_v1(
        bridge_plan,
        bridge_production,
        &profile,
        &plan,
        &[],
    )
    .unwrap();
    let builtins = crate::verify_builtin_object_strong_relocations_v1(
        &plan,
        &symbols,
        &[crate::ScoopLirObjectCandidateV1::new(
            plan.scoop_lir_members()[0].member_id(),
            &bytes,
        )],
        bridge_production,
        &[],
    )
    .unwrap();
    LinkObjectFixture {
        bytes,
        checked_offset,
        plan,
        builtins,
    }
}

pub(super) fn finalized_link_object_fixture() -> (
    ConeProductionSectionV1,
    crate::VerifiedEntryPatchSetV1,
    crate::CanonicalDefinedLinkSymbolOwnerSetV1,
    crate::CanonicalUndefinedSymbolRequirementSetV1,
) {
    let fixture = link_object_fixture();
    let (canonical, production) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = ConeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let objects = [crate::ScoopLirObjectCandidateV1::new(
        fixture.plan.scoop_lir_members()[0].member_id(),
        &fixture.bytes,
    )];
    let provisional_sites = [crate::ProvisionalDigestPatchSiteV1::new(
        digest_patch_intent(),
        fixture.plan.scoop_lir_members()[0].member_id(),
        fixture.checked_offset,
        32,
    )];
    let patch_sites = crate::verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins,
        &foundation,
        production.digest_finalization_plan().clone(),
        &objects,
        &provisional_sites,
    )
    .unwrap();
    let defined_symbols =
        crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
            patch_sites.builtins().strong_relocations(),
        )
        .unwrap();
    let registrations = production.registration_production();
    let stackmaps = crate::verify_scoop_lir_stackmaps_v1(
        patch_sites.builtins().clone(),
        registrations.safepoint_semantics(),
        &objects,
    )
    .unwrap();
    let safepoint_registrations = crate::verify_strong_safepoint_registrations_v1(
        stackmaps.clone(),
        patch_sites.clone(),
        registrations.safepoints().clone(),
        &objects,
    )
    .unwrap();
    let safepoints =
        crate::compute_strong_safepoint_fingerprints_v1(safepoint_registrations, &objects).unwrap();
    let callable_registrations = crate::verify_strong_callable_registrations_v1(
        patch_sites.clone(),
        registrations.callables().clone(),
        &objects,
    )
    .unwrap();
    let callable_objects = callable_registrations;
    let requirements = empty_undefined_requirements(&patch_sites, &foundation, &production);
    let callable_bodies = crate::compute_strong_callable_body_object_fingerprints_v1(
        callable_objects,
        stackmaps,
        requirements.clone(),
        &objects,
    )
    .unwrap();
    let callables = crate::compute_strong_callable_fingerprints_v1(
        callable_bodies.clone(),
        production.canonical_callable_definitions(),
    )
    .unwrap();
    let type_registrations = crate::verify_strong_type_registrations_v1(
        patch_sites.clone(),
        registrations.types().clone(),
        &objects,
    )
    .unwrap();
    let types = crate::compute_strong_type_fingerprints_v1(
        type_registrations,
        production.canonical_shape_definitions(),
        requirements.clone(),
        &objects,
    )
    .unwrap();
    let immortal_registrations = crate::verify_strong_immortal_object_registrations_v1(
        patch_sites.clone(),
        registrations.immortal_objects().clone(),
        &objects,
    )
    .unwrap();
    let immortal_registration_objects =
        crate::compute_strong_immortal_object_registration_object_fingerprints_v1(
            immortal_registrations,
            &objects,
        )
        .unwrap();
    let immortal_definitions = crate::compute_strong_immortal_object_definition_fingerprints_v1(
        immortal_registration_objects,
        requirements.clone(),
        &objects,
    )
    .unwrap();
    let immortal_objects = crate::compute_strong_immortal_object_fingerprints_v1(
        immortal_definitions,
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let static_storage_registrations = crate::verify_strong_static_storage_registrations_v1(
        patch_sites.clone(),
        registrations.static_storages().clone(),
        &objects,
    )
    .unwrap();
    let static_storage_objects =
        crate::compute_strong_static_storage_registration_object_fingerprints_v1(
            static_storage_registrations,
            &objects,
        )
        .unwrap();
    let static_storage_definitions =
        crate::compute_strong_static_storage_definition_fingerprints_v1(
            static_storage_objects,
            &objects,
        )
        .unwrap();
    let static_storage_shapes =
        crate::compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
            .unwrap();
    let static_storages = crate::compute_strong_static_storage_fingerprints_v1(
        static_storage_shapes,
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let initialization_registrations = crate::verify_strong_initialization_registrations_v1(
        patch_sites.clone(),
        registrations.initialization_units().clone(),
        &objects,
    )
    .unwrap();
    let initialization_objects =
        crate::compute_strong_initialization_registration_object_fingerprints_v1(
            initialization_registrations,
            &objects,
        )
        .unwrap();
    let initialization_definitions =
        crate::compute_strong_initialization_definition_fingerprints_v1(
            initialization_objects,
            &objects,
        )
        .unwrap();
    let initializations = crate::compute_strong_initialization_fingerprints_v1(
        initialization_definitions,
        &callable_bodies,
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let image = crate::verify_cone_image_v1(
        patch_sites.clone(),
        production.image_plan().clone(),
        &objects,
    )
    .unwrap();
    let entry =
        crate::verify_entry_production_v1(patch_sites, production.entry_plan().clone(), &objects)
            .unwrap();
    let patched = crate::patch_strong_registration_fingerprints_v1(
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        &objects,
    )
    .unwrap();
    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
            .unwrap();
    let image = crate::compute_runtime_image_fingerprint_v1(image, patched, compatibility).unwrap();
    let image = crate::patch_runtime_image_fingerprint_v1(image).unwrap();
    let final_objects = crate::patch_entry_production_v1(image, entry).unwrap();
    (production, final_objects, defined_symbols, requirements)
}

pub(super) fn link_object_bytes() -> Vec<u8> {
    finalized_link_object_fixture().1.objects()[0]
        .bytes()
        .to_vec()
}

fn empty_undefined_requirements(
    patch_sites: &crate::VerifiedScoopLirDigestPatchSiteSetV1,
    foundation: &ConeLirFoundation,
    production: &ConeProductionSectionV1,
) -> crate::CanonicalUndefinedSymbolRequirementSetV1 {
    let strong = patch_sites.builtins().strong_relocations().clone();
    let current = crate::verify_current_cone_undefined_requirements_v1(
        strong.clone(),
        production.generated_bridge_plan().clone(),
    )
    .unwrap();
    let native = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        selection().target(),
        foundation,
    )
    .unwrap();
    let core =
        crate::verify_dependency_strong_requirements_v1(selection().target(), strong, &[]).unwrap();
    let source = crate::verify_source_external_requirements_v1(core, native.clone()).unwrap();
    let runtime = crate::verify_runtime_and_eh_requirements_v1(source, selection()).unwrap();
    let bridge = crate::verify_generated_c_bridge_semantics_v1(
        patch_sites.clone(),
        production.generated_bridge_plan().clone(),
        native,
        &c_bridge_profile(),
    )
    .unwrap();
    let external = crate::verify_c_bridge_target_support_requirements_v1(runtime, bridge).unwrap();
    let external = crate::seal_builtin_object_external_requirements_v1(external).unwrap();
    crate::finalize_undefined_symbol_requirements_v1(current, external).unwrap()
}
