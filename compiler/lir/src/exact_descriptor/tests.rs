use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::*;

mod fixture;
mod projection;
mod shape_link;
use fixture::Fixture;

#[test]
fn replay_joins_layout_shape_dispatch_diagnostic_and_physical_definition() {
    let fixture = Fixture::new();
    let descriptor = fixture.replay(fixture.semantic()).unwrap();

    assert_eq!(descriptor.exact(), fixture.exact());
    assert_eq!(
        descriptor.value_layout().identity().layout(),
        fixture.value_layout()
    );
    assert_eq!(
        descriptor.instance_layout().identity().layout(),
        fixture.instance_layout()
    );
    assert_eq!(descriptor.shape(), fixture.instance().shape());
    assert_eq!(
        descriptor.object_scan(),
        fixture.instance().shape().object_scan()
    );
    assert_eq!(descriptor.ancestry().parent(), None);
    assert!(descriptor.ancestry().interfaces().is_empty());
    assert_eq!(descriptor.dispatch().vtable(), fixture.vtable());
    assert!(descriptor.dispatch().itables().is_empty());
    assert_eq!(descriptor.diagnostic_name().as_str(), fixture.name());
    assert_eq!(descriptor.definition().semantic_id(), fixture.exact());
    assert_eq!(descriptor.registration(), fixture.registration());
}

#[test]
fn descriptor_and_table_have_canonical_checked_wire_round_trips() {
    let fixture = Fixture::new();
    let descriptor = fixture.replay(fixture.semantic()).unwrap();
    let bytes = encode(&descriptor).unwrap();
    assert_eq!(bytes[0], 0xaa);
    let decoded =
        decode_canonical::<DecodedExactDescriptorExportV1>(&bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .validate_against(&descriptor, &mut fixture.meter())
            .unwrap(),
        descriptor
    );

    let table = CanonicalExactDescriptorExportsV1::try_new(
        LirTargetProfile::DARWIN_AARCH64,
        fixture.foundation(),
        vec![descriptor],
        &mut fixture.meter(),
    )
    .unwrap();
    assert_eq!(table.provider(), scoop_identity::ConeIdentity::SINGLE_FILE);
    assert_eq!(table.get(fixture.exact()), table.records().first());
    let bytes = encode(&table).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalExactDescriptorExportsV1>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .validate_against(&table, &mut fixture.meter())
            .unwrap(),
        table
    );
}

#[test]
fn replay_rejects_independent_name_shape_and_dispatch_claims() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.replay(fixture.semantic_with_name("not-canonical")),
        Err(ExactDescriptorError::DiagnosticName(exact)) if exact == fixture.exact()
    ));
    assert!(matches!(
        fixture.replay(fixture.semantic_with_shape(TypeInstanceShapeV1::abstract_ref())),
        Err(ExactDescriptorError::InstanceShape(exact)) if exact == fixture.exact()
    ));
    let missing = scoop_identity::PersistentDispatchTableId::from_key(
        &scoop_identity::DispatchTableKey::vtable(fixture.foreign_exact()),
    )
    .unwrap();
    assert!(matches!(
        fixture.replay(fixture.semantic_with_vtable(missing)),
        Err(ExactDescriptorError::DispatchTable(table)) if table == missing
    ));
}

#[test]
fn table_rejects_duplicate_records_and_consumes_shared_entry_budget() {
    let fixture = Fixture::new();
    let descriptor = fixture.replay(fixture.semantic()).unwrap();
    assert!(matches!(
        CanonicalExactDescriptorExportsV1::try_new(
            LirTargetProfile::DARWIN_AARCH64,
            fixture.foundation(),
            vec![descriptor.clone(), descriptor.clone()],
            &mut fixture.meter(),
        ),
        Err(ExactDescriptorTableError::Duplicate(exact)) if exact == fixture.exact()
    ));
    let limits = DecodeLimits {
        semantic_table_entries: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        CanonicalExactDescriptorExportsV1::try_new(
            LirTargetProfile::DARWIN_AARCH64,
            fixture.foundation(),
            vec![descriptor],
            &mut scoop_wire::BudgetMeter::new(limits),
        ),
        Err(ExactDescriptorTableError::Resource(_))
    ));
}
