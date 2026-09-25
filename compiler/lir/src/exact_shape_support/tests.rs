use scoop_identity::{ConeIdentity, SourceNominalKind};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{CanonicalExactDescriptorExportsV1, LirTargetProfile};

mod fixture;
use fixture::Fixture;

#[test]
fn value_source_replays_all_roles_and_canonical_wire() {
    let fixture = Fixture::new(SourceNominalKind::Struct, false);
    let record = fixture.replay().unwrap();
    assert_eq!(record.source_nominal(), fixture.source_nominal());
    assert!(record.roles().boxed_value().available().is_some());
    assert!(record.roles().coroutine_step().available().is_some());
    assert!(record.roles().coroutine_slot().available().is_some());

    let bytes = encode(&record).unwrap();
    assert_eq!(bytes[0], 0xa8);
    let decoded = decode_canonical::<DecodedParamFreeShapeSupportExportV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.validate_against(&record).unwrap(), record);
}

#[test]
fn reference_source_uses_no_box_and_niche_helpers() {
    let fixture = Fixture::new(SourceNominalKind::Interface, false);
    let record = fixture.replay().unwrap();
    assert_eq!(
        record.roles().boxed_value(),
        &crate::ShapeSupportAvailabilityV1::NotApplicable(
            crate::ClosedShapeSupportReasonV1::ReferenceNominalRequiresNoBox,
        )
    );
    for support in [
        record.roles().coroutine_step().available().unwrap(),
        record.roles().coroutine_slot().available().unwrap(),
    ] {
        let descriptor = fixture.descriptors().get(support.exact()).unwrap();
        assert!(matches!(
            descriptor.value_layout().representation().kind(),
            crate::ExactRepresentationKindV1::NicheEnum(_)
        ));
    }
}

#[test]
fn replay_rejects_helper_with_a_different_payload_layout() {
    let fixture = Fixture::new(SourceNominalKind::Struct, true);
    assert!(matches!(
        fixture.replay(),
        Err(ParamFreeShapeSupportExportError::HelperPayload(_))
    ));
}

#[test]
fn table_enforces_independent_coverage_and_round_trips() {
    let fixture = Fixture::new(SourceNominalKind::Struct, false);
    let record = fixture.replay().unwrap();
    let table = CanonicalParamFreeShapeSupportExportsV1::try_new(
        std::slice::from_ref(fixture.source()),
        fixture.layouts(),
        fixture.descriptors(),
        fixture.foundation(),
        vec![record.clone()],
    )
    .unwrap();
    assert_eq!(table.get(record.source_nominal()), Some(&record));
    let bytes = encode(&table).unwrap();
    let decoded =
        decode_canonical::<DecodedCanonicalParamFreeShapeSupportExportsV1>(&bytes).unwrap();
    assert_eq!(
        decoded
            .validate(
                std::slice::from_ref(fixture.source()),
                fixture.layouts(),
                fixture.descriptors(),
                fixture.foundation(),
            )
            .unwrap(),
        table
    );

    let omitted =
        decode_canonical::<DecodedCanonicalParamFreeShapeSupportExportsV1>(b"\x80").unwrap();
    assert!(matches!(
        omitted.validate(
            std::slice::from_ref(fixture.source()),
            fixture.layouts(),
            fixture.descriptors(),
            fixture.foundation(),
        ),
        Err(ParamFreeShapeSupportTableError::Coverage)
    ));
}

mod core;
