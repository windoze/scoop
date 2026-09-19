use scoop_identity::{ConeIdentity, PersistentDispatchSlotId};
use scoop_wire::{BudgetMeter, DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::Fixture;
use crate::{PersistentAccessConstraintV1, PersistentAccessDomainV1};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn fixture() -> (Fixture, Vec<ProtectedDefaultSlotCallDomainV1>) {
    let mut fixture = Fixture::default();
    let owner = fixture.class("Owner");
    let first = fixture.function(owner, "first", false, vec![]);
    let second = fixture.function(owner, "second", false, vec![]);
    let first = fixture.slot(first);
    let second = fixture.slot(second);
    let domain = PersistentAccessDomainV1::try_from_constraints(vec![
        PersistentAccessConstraintV1::Cone(ConeIdentity::CORE),
        PersistentAccessConstraintV1::LexicalOwner(owner.source),
        PersistentAccessConstraintV1::SubclassesOf(owner.exact),
    ])
    .unwrap();
    let mut records = vec![
        record(first, domain),
        record(second, PersistentAccessDomainV1::universal()),
    ];
    records.sort_unstable_by_key(ProtectedDefaultSlotCallDomainV1::slot);
    (fixture, records)
}

fn record(
    slot: PersistentDispatchSlotId,
    domain: PersistentAccessDomainV1,
) -> ProtectedDefaultSlotCallDomainV1 {
    ProtectedDefaultSlotCallDomainV1::new(slot, PersistentSlotContractDomainV1::new(domain))
}

#[test]
fn slot_domain_wire_round_trips_complete_typed_access_regions() {
    let (mut fixture, ordered) = fixture();
    let table = CanonicalProtectedDefaultSlotCallDomainsV1::try_new(
        ordered.iter().rev().cloned().collect(),
    )
    .unwrap();
    assert_eq!(table.records(), ordered);
    for record in table.records() {
        let bytes = encode(record).unwrap();
        assert_eq!(&bytes[..2], &[0xa2, 1]);
        let decoded: DecodedProtectedDefaultSlotCallDomainV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.resolve(&mut fixture, &mut meter()).unwrap(),
            *record
        );
        assert_eq!(table.get(record.slot()), Some(record));
    }
    let bytes = encode(&table).unwrap();
    let decoded: DecodedCanonicalProtectedDefaultSlotCallDomainsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), table);
}

#[test]
fn duplicate_slot_is_rejected_even_when_domains_match_or_differ() {
    let (mut fixture, ordered) = fixture();
    for second in [
        ordered[0].clone(),
        record(ordered[0].slot(), PersistentAccessDomainV1::empty()),
    ] {
        let records = vec![ordered[0].clone(), second];
        assert!(matches!(
            CanonicalProtectedDefaultSlotCallDomainsV1::try_new(records.clone()),
            Err(ProtectedDefaultSlotCallDomainsBuildError::Duplicate { index: 1, .. })
        ));
        let decoded: DecodedCanonicalProtectedDefaultSlotCallDomainsV1 = decode_canonical(
            &encode(&RawTable(records)).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture, &mut meter()),
            Err(ProtectedDefaultSlotCallDomainResolutionError::Build(
                ProtectedDefaultSlotCallDomainsBuildError::Duplicate { index: 1, .. }
            ))
        ));
    }
}

#[test]
fn slot_domain_reader_rejects_noncanonical_order_and_unknown_typed_identity() {
    let (mut fixture, mut ordered) = fixture();
    ordered.reverse();
    let decoded: DecodedCanonicalProtectedDefaultSlotCallDomainsV1 = decode_canonical(
        &encode(&RawTable(ordered.clone())).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(ProtectedDefaultSlotCallDomainResolutionError::Build(
            ProtectedDefaultSlotCallDomainsBuildError::NonCanonicalOrder { index: 1 }
        ))
    ));
    let decoded: DecodedProtectedDefaultSlotCallDomainV1 =
        decode_canonical(&encode(&ordered[0]).unwrap(), DecodeLimits::default()).unwrap();
    fixture.slots.clear();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut meter()),
        Err(ProtectedDefaultSlotCallDomainResolutionError::Identity(
            "unknown slot"
        ))
    ));
}

#[test]
fn slot_domain_reader_requires_exact_product_shape_and_domain_encoding() {
    let (_, records) = fixture();
    let mut wrong_fields = encode(&records[0]).unwrap();
    wrong_fields[0] = 0xa1;
    assert!(
        decode_canonical::<DecodedProtectedDefaultSlotCallDomainV1>(
            &wrong_fields,
            DecodeLimits::default()
        )
        .is_err()
    );
    let bytes = encode(&BadDomain(records[0].slot())).unwrap();
    assert!(
        decode_canonical::<DecodedProtectedDefaultSlotCallDomainV1>(
            &bytes,
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn slot_domain_resolution_charges_shared_node_work_and_collection_budgets() {
    let (mut fixture, records) = fixture();
    let table = CanonicalProtectedDefaultSlotCallDomainsV1::try_new(records).unwrap();
    let decoded: DecodedCanonicalProtectedDefaultSlotCallDomainsV1 =
        decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decoded
                .clone()
                .resolve(&mut fixture, &mut BudgetMeter::new(limits)),
            Err(ProtectedDefaultSlotCallDomainResolutionError::Resource(_))
        ));
    }
    let empty = CanonicalProtectedDefaultSlotCallDomainsV1::try_new(Vec::new()).unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    let decoded: DecodedCanonicalProtectedDefaultSlotCallDomainsV1 =
        decode_canonical(&[0x80], DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture, &mut meter()).unwrap(), empty);
}

struct RawTable(Vec<ProtectedDefaultSlotCallDomainV1>);
impl WireEncode for RawTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.0)
    }
}

struct BadDomain(PersistentDispatchSlotId);
impl WireEncode for BadDomain {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.0.encode(encoder)?;
        encoder.field(2)?;
        wire::tag(encoder, 1, 3)
    }
}
