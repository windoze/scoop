//! Dependency TD/dispatch references traverse the actual ten-field wire.

use super::*;
use crate::{
    ConeProductionSectionV2, ConeProductionSectionValidationError as SectionError,
    DecodedConeProductionSectionV2, EntryProductionSourceV1,
};
use scoop_identity::ConeCoordinate;

fn complete_fixture() -> Fixture {
    let mut fixture = foreign_fixture();
    crate::production::cone_section::tests::attach_image(
        &ConeCoordinate::reserved_single_file(),
        &mut fixture.foundation,
        &mut fixture.digests,
    );
    fixture
}

fn section(fixture: &Fixture) -> ConeProductionSectionV2 {
    ConeProductionSectionV2::from_parts(
        ConeCoordinate::reserved_single_file(),
        &[],
        &fixture.foundation,
        fixture.digests.clone(),
        surface(fixture, true),
        EntryProductionSourceV1::Library,
        &[],
        crate::canonical_callable::tests::fixture_definitions(&fixture.foundation),
        crate::CanonicalShapeAbisV1::new(Vec::new(), &fixture.foundation).unwrap(),
    )
    .unwrap()
}

fn replay_section(
    fixture: &Fixture,
    bytes: &[u8],
    definitions: &StrongTypeReferenceDefinitionsV2,
) -> Result<ConeProductionSectionV2, SectionError> {
    let decoded: DecodedConeProductionSectionV2 = decode_canonical(bytes).unwrap();
    decoded.replay(
        ConeCoordinate::reserved_single_file(),
        &[],
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        EntryProductionSourceV1::Library,
        &[],
        definitions,
        &StrongInitializationDefinitionCatalogV2::new(ConeIdentity::SINGLE_FILE, &[]).unwrap(),
    )
}

#[test]
fn ten_field_section_replays_foreign_parent_interface_and_dispatch() {
    let fixture = complete_fixture();
    let original = section(&fixture);
    let bytes = encode(&original).unwrap();
    assert_eq!(bytes[0], 0xaa);
    let shared: crate::DecodedConeProductionSectionV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&shared).unwrap(), bytes);
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let replayed = replay_section(&fixture, &bytes, &definitions).unwrap();

    assert_eq!(
        replayed.canonical_definitions(),
        original.canonical_definitions()
    );
    assert_eq!(
        replayed.object_definition_plans(),
        original.object_definition_plans()
    );
    assert_eq!(
        replayed.digest_finalization_plan(),
        original.digest_finalization_plan()
    );
    assert_eq!(
        replayed.registration_identities(),
        original.registration_production().identities()
    );
    assert_eq!(
        replayed.type_registrations(),
        original.registration_production().types()
    );
    assert_eq!(replayed.image_plan(), original.image_plan());
    assert_eq!(replayed.entry_plan(), original.entry_plan());
    assert_eq!(replayed.shape_support_plan(), original.shape_support_plan());
    assert_eq!(
        replayed.generated_bridge_plan(),
        original.generated_bridge_plan()
    );
}

#[test]
fn ten_field_section_rejects_missing_foreign_definitions_and_changed_definition_atoms() {
    let fixture = complete_fixture();
    let original = section(&fixture);
    let bytes = encode(&original).unwrap();
    let empty = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[], &[]).unwrap();
    assert!(matches!(
        replay_section(&fixture, &bytes, &empty),
        Err(SectionError::Registrations(Error::TypeReference(_)))
    ));
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let mut altered = bytes;
    let atom = fixture.foundation.definition_atoms()[0].id();
    let position = altered
        .windows(32)
        .position(|bytes| bytes == atom.as_array())
        .unwrap();
    altered[position] ^= 1;
    assert!(matches!(
        replay_section(&fixture, &altered, &definitions),
        Err(SectionError::SectionMismatch)
    ));
}

#[test]
fn ten_field_section_rejects_a_valid_digest_graph_missing_a_registration_input() {
    let fixture = complete_fixture();
    let bytes = crate::production::cone_section::tests::without_image_input(
        &section(&fixture),
        &fixture.foundation,
    );
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    assert!(matches!(
        replay_section(&fixture, &bytes, &definitions),
        Err(SectionError::DigestMismatch)
    ));
}

#[test]
fn final_layout_join_keeps_dependency_checks_for_unexported_local_types() {
    let fixture = complete_fixture();
    let bytes = encode(&section(&fixture)).unwrap();
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let replayed = replay_section(&fixture, &bytes, &definitions).unwrap();
    let layout = empty_layout_section();
    assert!(matches!(
        replayed.validate_layout_abi(&layout),
        Err(crate::StrongProductionLayoutJoinError::MissingSelectedDescriptor { provider, .. })
            if provider == ConeIdentity::CORE
    ));
}

#[test]
fn final_layout_join_preserves_complete_private_type_registrations() {
    let mut fixture = Fixture::new(Options::default());
    let coordinate = ConeCoordinate::reserved_single_file();
    crate::production::cone_section::tests::attach_image(
        &coordinate,
        &mut fixture.foundation,
        &mut fixture.digests,
    );
    let production = ConeProductionSectionV2::from_parts(
        coordinate,
        &[],
        &fixture.foundation,
        fixture.digests.clone(),
        surface(&fixture, false),
        EntryProductionSourceV1::Library,
        &[],
        crate::canonical_callable::tests::fixture_definitions(&fixture.foundation),
        crate::CanonicalShapeAbisV1::new(Vec::new(), &fixture.foundation).unwrap(),
    )
    .unwrap();
    let expected = production.registration_production().types().clone();
    assert!(!expected.registrations().is_empty());
    let joined = production
        .validate_layout_abi(&empty_layout_section())
        .unwrap();
    assert_eq!(joined.type_registrations(), &expected);
}

fn empty_layout_section() -> crate::CrossConeLayoutAbiSectionV1<'static> {
    let foundation = crate::ConeLirFoundation::try_new(
        ConeIdentity::SINGLE_FILE,
        crate::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    let layouts = crate::CanonicalExactLayoutExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let descriptors = crate::CanonicalExactDescriptorExportsV1::try_new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let exports = crate::LayoutAbiExportConstituentsV1::try_new(
        layouts.clone(),
        descriptors.clone(),
        crate::CanonicalExactDispatchExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalExactCallableAbiExportsV1::try_new(
            crate::LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalParamFreeShapeSupportExportsV1::from_sources(
            &[],
            &layouts,
            &descriptors,
            &foundation,
        )
        .unwrap(),
        crate::CrossConeLirBridgeSectionV1::try_new(&foundation, Vec::new(), Vec::new()).unwrap(),
    )
    .unwrap();
    crate::CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &[]).unwrap()
}
