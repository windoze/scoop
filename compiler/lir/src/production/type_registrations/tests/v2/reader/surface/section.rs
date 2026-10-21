//! Production with dependency type references uses the compact persisted section.

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
fn compact_section_round_trips_production_with_foreign_type_references() {
    let fixture = complete_fixture();
    let bytes = encode(&section(&fixture)).unwrap();
    assert_eq!(bytes[0], 0xa4);
    let decoded: DecodedConeProductionSectionV2 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn production_preserves_complete_private_type_registrations() {
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
    assert_eq!(production.type_registrations(), &expected);
}
