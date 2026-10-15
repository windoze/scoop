//! Dependency TD/dispatch references traverse the actual ten-field wire.

use super::*;
use crate::{ConeProductionSectionV2, DecodedConeProductionSectionV2, EntryProductionSourceV1};
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

#[test]
fn ten_field_section_preserves_foreign_type_references_on_wire() {
    let fixture = complete_fixture();
    let bytes = encode(&section(&fixture)).unwrap();
    assert_eq!(bytes[0], 0xaa);
    let decoded: DecodedConeProductionSectionV2 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn final_layout_join_keeps_dependency_checks_for_unexported_local_types() {
    let fixture = complete_fixture();
    let production = section(&fixture);
    let layout = empty_layout_section();
    assert!(matches!(
        production.validate_layout_abi(&layout),
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
        crate::target_fixtures::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let descriptors = crate::CanonicalExactDescriptorExportsV1::try_new(
        crate::target_fixtures::DARWIN_AARCH64,
        &foundation,
        Vec::new(),
    )
    .unwrap();
    let exports = crate::LayoutAbiExportConstituentsV1::try_new(
        layouts.clone(),
        descriptors.clone(),
        crate::CanonicalExactDispatchExportsV1::try_new(
            crate::target_fixtures::DARWIN_AARCH64,
            &foundation,
            Vec::new(),
        )
        .unwrap(),
        crate::CanonicalExactCallableAbiExportsV1::try_new(
            crate::target_fixtures::DARWIN_AARCH64,
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
