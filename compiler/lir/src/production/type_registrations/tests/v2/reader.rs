//! Whole registration records are replayed; selections remain a section join.

use super::*;
use crate::{
    StrongRegistrationProductionValidationError as Error,
    StrongTypeReferenceResolutionErrorV2 as RefError,
};

mod fixture;
use fixture::*;

mod surface;

#[test]
fn v2_reader_replays_legacy_and_foreign_complete_registration_records() {
    for has_itable in [false, true] {
        let fixture = Fixture::new(Options {
            first_type_has_itable: has_itable,
            ..Options::default()
        });
        let expected = semantics(&fixture, None);
        let plans = build(&fixture, None).unwrap();
        assert_eq!(
            validate(&fixture, decoded(&plans), &catalog(&expected)).unwrap(),
            expected
        );
    }
    let fixture = foreign_fixture();
    let expected = semantics(&fixture, Some(ConeIdentity::CORE));
    let plans = build(&fixture, Some(ConeIdentity::CORE)).unwrap();
    let actual = validate(&fixture, decoded(&plans), &catalog(&expected)).unwrap();
    assert_eq!(actual, expected);
    let replayed = StrongTypeRegistrationPlanSetV2::new(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        &actual,
        &fixture.digests,
    )
    .unwrap();
    for (original, replayed) in plans.registrations().iter().zip(replayed.registrations()) {
        assert_eq!(encode(original).unwrap(), encode(replayed).unwrap());
    }
}

#[test]
fn foreign_parent_and_dispatch_require_the_same_available_provider() {
    let fixture = foreign_fixture();
    let expected = semantics(&fixture, Some(ConeIdentity::CORE));
    let definitions = catalog(&expected);
    let empty =
        crate::StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[], &[]).unwrap();
    let plans = build(&fixture, Some(ConeIdentity::CORE)).unwrap();
    assert!(matches!(
        validate(&fixture, decoded(&plans), &empty),
        Err(Error::TypeReference(
            RefError::UnknownDependencyDescriptor { .. }
        ))
    ));
    let changed_provider =
        scoop_identity::ConeCoordinate::new("test", "different-provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
    let changed = build(&fixture, Some(changed_provider)).unwrap();
    assert!(matches!(
        validate(&fixture, decoded(&changed), &definitions),
        Err(Error::TypeReference(
            RefError::UnknownDependencyDescriptor { .. }
        ))
    ));
    let descriptors_only = crate::StrongTypeReferenceDefinitionsV2::new(
        ConeIdentity::SINGLE_FILE,
        definitions.descriptor_definitions(),
        &[],
    )
    .unwrap();
    assert!(matches!(
        validate(&fixture, decoded(&plans), &descriptors_only),
        Err(Error::TypeReference(
            RefError::UnknownDependencyCallable { .. }
        ))
    ));
}

#[test]
fn a_foreign_itable_reference_cannot_alias_a_local_descriptor_definition() {
    let fixture = Fixture::new(Options {
        first_type_has_itable: true,
        ..Options::default()
    });
    let semantics = semantics(&fixture, Some(ConeIdentity::CORE));
    let plans = build(&fixture, Some(ConeIdentity::CORE)).unwrap();
    assert!(matches!(
        validate(&fixture, decoded(&plans), &catalog(&semantics)),
        Err(Error::TypeReference(RefError::LocalDescriptorPartition(_)))
    ));
}

#[test]
fn type_reader_recomputes_definition_fields_and_requires_complete_coverage() {
    let fixture = foreign_fixture();
    let semantics = semantics(&fixture, Some(ConeIdentity::CORE));
    let definitions = catalog(&semantics);
    let plans = build(&fixture, Some(ConeIdentity::CORE)).unwrap();
    let mut records = decoded(&plans);
    records.pop();
    assert!(matches!(
        validate(&fixture, records, &definitions),
        Err(Error::TableLength { .. })
    ));
    let mut bytes = encode(&plans.registrations()[0]).unwrap();
    let id = plans.registrations()[0].descriptor_definition_plan();
    let position = bytes
        .windows(32)
        .position(|window| window == id.as_array())
        .unwrap();
    bytes[position] ^= 1;
    let mut records = decoded(&plans);
    records[0] = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        validate(&fixture, records, &definitions),
        Err(Error::EntryMismatch { index: 0, .. })
    ));
}
