use scoop_identity::SourceNominalKind;
use scoop_wire::{BudgetMeter, DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::NominalRepresentationShapeV1;
use crate::cross_cone_type_semantics::representation::tests::support::Fixture;

fn record(fixture: &Fixture) -> NominalRepresentationSupportV1 {
    NominalRepresentationSupportV1::try_new(
        &fixture.key,
        fixture.access.clone(),
        NominalRepresentationShapeV1::Interface,
    )
    .unwrap()
}

#[test]
fn representation_table_has_explicit_empty_wire_and_rejects_duplicate_production() {
    assert_eq!(
        encode(&CanonicalNominalRepresentationSupportV1::default()).unwrap(),
        [0x80]
    );
    let fixture = Fixture::new(SourceNominalKind::Interface);
    let record = record(&fixture);
    assert!(
        CanonicalNominalRepresentationSupportV1::try_new(vec![record.clone(), record.clone()])
            .is_err()
    );
    let table = CanonicalNominalRepresentationSupportV1::try_new(vec![record.clone()]).unwrap();
    assert_eq!(table.get(record.owner()), Some(&record));
}

#[test]
fn reader_resolves_canonical_records_with_the_shared_budget() {
    let mut fixture = Fixture::new(SourceNominalKind::Interface);
    let record = record(&fixture);
    let table = CanonicalNominalRepresentationSupportV1::try_new(vec![record]).unwrap();
    let bytes = encode(&table).unwrap();
    let decoded: DecodedCanonicalNominalRepresentationSupportV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    assert_eq!(
        decoded.clone().resolve(&mut fixture, &mut meter).unwrap(),
        table
    );
    assert!(meter.usage().decoded_nodes > 0);
    let mut exhausted = BudgetMeter::new(DecodeLimits {
        decoded_nodes: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut exhausted),
        Err(NominalRepresentationTableResolutionError::Resource(_))
    ));
}

#[test]
fn reader_rejects_duplicate_records_without_silently_normalizing() {
    let mut fixture = Fixture::new(SourceNominalKind::Interface);
    let record = record(&fixture);
    let bytes = encode(&Sequence(vec![record.clone(), record])).unwrap();
    let decoded: DecodedCanonicalNominalRepresentationSupportV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture, &mut BudgetMeter::new(DecodeLimits::default())),
        Err(NominalRepresentationTableResolutionError::Order(_))
    ));
}

struct Sequence(Vec<NominalRepresentationSupportV1>);
impl WireEncode for Sequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.0)
    }
}
