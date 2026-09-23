use super::*;
use crate::{SourceAccessConstraintV1, SourceAccessDomainV1};
use scoop_wire::{BudgetMeter, WirePath};

#[test]
fn restricted_default_witness_round_trips_without_public_capability() {
    let fixture = Fixture::new();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let domain = SourceAccessDomainV1::from_constraints(
        vec![SourceAccessConstraintV1::LexicalOwner(
            crate::SourceNominalId::Concrete(fixture.type_id),
        )],
        &mut meter,
        &WirePath::root(),
    )
    .unwrap();
    let witness = ExportDefaultAccessWitnessV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        domain.clone(),
        Some(domain.clone()),
        domain,
    )
    .unwrap();
    assert_eq!(witness.public_call_domain(), None);
    assert!(!witness.target_domain().is_universal());
    let bytes = encode(&witness).unwrap();
    let decoded: DecodedExportDefaultAccessWitnessV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut meter, &WirePath::root()),
        Ok(witness)
    );
}

#[test]
fn default_witness_rejects_constructor_slots_and_non_optional_slot_arrays() {
    let fixture = Fixture::new();
    let witness = ExportDefaultAccessWitnessV1::new(
        CallableTemplateOrigin::Constructor(fixture.constructor),
        ExportDefaultCallDomainV1::DirectAndPublicSlot,
    );
    let decoded: DecodedExportDefaultAccessWitnessV1 =
        decode_canonical(&encode(&witness).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(
            &mut fixture.resolver(),
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root()
        ),
        Err(ExportDefaultAccessWitnessResolutionError::Build(
            ExportDefaultAccessWitnessBuildError::SlotForConstructor
        ))
    ));

    let mut bytes = encode(&ExportDefaultAccessWitnessV1::new(
        CallableTemplateOrigin::Function(fixture.function),
        ExportDefaultCallDomainV1::DirectPublic,
    ))
    .unwrap();
    let slot = bytes.len() - 7;
    assert_eq!(bytes[slot], 0x80);
    let mut duplicate_slots = vec![0x82];
    for _ in 0..2 {
        duplicate_slots.extend(encode(&SourceAccessDomainV1::universal()).unwrap());
    }
    bytes.splice(slot..slot + 1, duplicate_slots);
    let truncated = &bytes[..slot + 1];
    let error =
        decode_canonical::<DecodedExportDefaultAccessWitnessV1>(truncated, DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2
        }
    );
    let error =
        decode_canonical::<DecodedExportDefaultAccessWitnessV1>(&bytes, DecodeLimits::default())
            .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2
        }
    );
}
