use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::cross_cone_interface::external_references::test_support::{
    Fixture, TargetOriginAuthority, TargetOriginAuthorityError,
};

#[test]
fn producer_sorts_by_typed_target_and_rejects_duplicate_targets() {
    let fixture = Fixture::new();
    let first = fixture.signature_reference(fixture.first_alias);
    let second = fixture.signature_reference(fixture.second_alias);
    let table =
        CanonicalExternalHirReferencesV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    assert!(table.records()[0].target() < table.records()[1].target());
    assert_eq!(table.get(first.target()), Some(&first));
    assert!(!table.is_empty());
    assert!(
        CanonicalExternalHirReferencesV1::try_new(Vec::new())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        CanonicalExternalHirReferencesV1::try_new(vec![first.clone(), first]),
        Err(ExternalHirReferenceSetBuildError::DuplicateTarget(
            ExternalHirTargetV1::TypeAlias(fixture.first_alias)
        ))
    );
}

#[test]
fn canonical_table_round_trips_through_typed_authority() {
    let fixture = Fixture::new();
    let expected = CanonicalExternalHirReferencesV1::try_new(vec![
        fixture.signature_reference(fixture.second_alias),
        fixture.signature_reference(fixture.first_alias),
    ])
    .unwrap();
    let decoded: DecodedCanonicalExternalHirReferencesV1 =
        decode_canonical(&encode(&expected).unwrap(), DecodeLimits::default()).unwrap();

    assert_eq!(decoded.resolve(&mut fixture.authority()).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_target_order() {
    let fixture = Fixture::new();
    let canonical = CanonicalExternalHirReferencesV1::try_new(vec![
        fixture.signature_reference(fixture.first_alias),
        fixture.signature_reference(fixture.second_alias),
    ])
    .unwrap();
    let low = canonical.records()[0].clone();
    let high = canonical.records()[1].clone();

    let duplicate = decode_table(&RecordSequence(vec![low.clone(), low.clone()]));
    assert!(matches!(
        duplicate.resolve(&mut fixture.authority()),
        Err(ExternalHirReferenceSetValidationError::DuplicateTarget { index: 1, .. })
    ));

    let reversed = decode_table(&RecordSequence(vec![high, low]));
    assert!(matches!(
        reversed.resolve(&mut fixture.authority()),
        Err(ExternalHirReferenceSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn table_semantics_reports_the_failing_canonical_record_index() {
    let fixture = Fixture::new();
    let table = CanonicalExternalHirReferencesV1::try_new(vec![
        fixture.signature_reference(fixture.first_alias),
        fixture.signature_reference(fixture.second_alias),
    ])
    .unwrap();
    let mut authority =
        TargetOriginAuthority::new(scoop_identity::ConeIdentity::CORE, fixture.provider);

    assert!(
        table
            .validate_semantics(
                &mut authority,
                &mut route_meter(),
                &scoop_wire::WirePath::root()
            )
            .is_ok()
    );

    authority.fail_on(table.records()[1].target());
    assert!(matches!(
        table.validate_semantics(&mut authority, &mut route_meter(), &scoop_wire::WirePath::root()),
        Err(ExternalHirReferenceSetSemanticValidationError::Record {
            index: 1,
            error,
        }) if matches!(
            error.as_ref(),
            ExternalHirReferenceSemanticValidationError::TargetOrigin {
                target,
                error: TargetOriginAuthorityError,
            } if *target == table.records()[1].target()
        )
    ));
}

struct RecordSequence(Vec<ExternalHirReferenceV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

fn decode_table(value: &impl WireEncode) -> DecodedCanonicalExternalHirReferencesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

fn route_meter() -> scoop_wire::BudgetMeter {
    scoop_wire::BudgetMeter::new(scoop_wire::DecodeLimits::default())
}
