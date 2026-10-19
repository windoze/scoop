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
    PlannedLinkObjectMemberSetV1::new(
        scoop_target_data::DARWIN_AARCH64,
        &partition,
        vec![units],
        Vec::new(),
    )
    .unwrap()
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
    let bridge_production =
        crate::read_generated_c_objects_v1(bridge_production, &profile, &[]).unwrap();
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
    let provisional_sites = [crate::DigestPatchSiteV1::new(
        digest_patch_intent(),
        fixture.plan.scoop_lir_members()[0].member_id(),
        fixture.checked_offset,
    )];
    let patch_sites =
        crate::ScoopLirDigestPatchSitesV1::new(fixture.builtins, (provisional_sites).to_vec());
    let defined_symbols =
        crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
            patch_sites.builtins().strong_relocations(),
        )
        .unwrap();
    let registrations = production.registration_production();
    let stackmaps = crate::read_scoop_stackmaps_v1(
        patch_sites.builtins().clone(),
        registrations.safepoint_semantics(),
        &objects,
    )
    .unwrap();
    let safepoint_registrations = crate::bind_safepoint_registrations_v1(
        stackmaps.clone(),
        patch_sites.clone(),
        registrations.safepoints().clone(),
    )
    .unwrap();
    let safepoints =
        crate::compute_strong_safepoint_fingerprints_v1(safepoint_registrations).unwrap();
    let callable_registrations = crate::bind_callable_registrations_v1(
        patch_sites.clone(),
        registrations.callables().clone(),
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
    let type_registrations =
        crate::bind_type_registrations_v1(patch_sites.clone(), registrations.types().clone())
            .unwrap();
    let types = crate::compute_strong_type_fingerprints_v1(
        type_registrations,
        production.canonical_shape_definitions(),
        &objects,
    )
    .unwrap();
    let immortal_objects = crate::compute_strong_immortal_object_fingerprints_v1(
        registrations.immortal_objects().clone(),
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let static_storage_registrations = crate::bind_static_storage_registrations_v1(
        &patch_sites,
        registrations.static_storages().clone(),
    )
    .unwrap();
    let static_storage_objects = static_storage_registrations;
    let static_storage_definitions = static_storage_objects;
    let static_storage_shapes =
        crate::compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
            .unwrap();
    let static_storages = crate::compute_strong_static_storage_fingerprints_v1(
        static_storage_shapes,
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let initialization_registrations = crate::bind_initialization_registrations_v1(
        &patch_sites,
        registrations.initialization_units().clone(),
    )
    .unwrap();
    let initialization_objects = initialization_registrations;
    let initialization_definitions = initialization_objects;
    let initializations = crate::compute_strong_initialization_fingerprints_v1(
        initialization_definitions,
        &callable_bodies,
        production.canonical_shape_definitions(),
    )
    .unwrap();
    let image =
        crate::bind_cone_image_v1(&patch_sites.clone(), production.image_plan().clone()).unwrap();
    let entry =
        crate::bind_entry_production_v1(patch_sites, production.entry_plan().clone()).unwrap();
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
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::CROSS_CONE_GENERIC)
            .unwrap();
    let image = crate::compute_runtime_image_fingerprint_v1(image, patched, compatibility).unwrap();
    let image = crate::patch_runtime_image_fingerprint_v1(image).unwrap();
    let final_objects = crate::patch_entry_production_v1(image, entry).unwrap();
    (production, final_objects, defined_symbols, requirements)
}

fn empty_undefined_requirements(
    patch_sites: &crate::ScoopLirDigestPatchSitesV1,
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
    let external = crate::classify_c_bridge_target_support_requirements_v1(
        runtime,
        production.generated_bridge_plan(),
        &c_bridge_profile(),
    )
    .unwrap();
    crate::finalize_undefined_symbol_requirements_v1(current, external)
}
