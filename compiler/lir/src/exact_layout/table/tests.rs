use scoop_identity::*;
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::exact_layout::tests::{Bound, meter, unit};
use crate::*;

fn fixture() -> (OdrFreeLirFoundation, Vec<ExactLayoutExportV1>) {
    let value = unit();
    let value_bound = Bound::value(value.identity().exact_record().clone());
    let instance_bound = Bound::instance(value.identity().exact_record().clone());
    let instance = ExactInstanceLayoutV1::boxed_payload(
        instance_bound.identity,
        &value,
        &instance_bound.foundation,
        &mut meter(),
    )
    .unwrap();
    let sources = [&value_bound.foundation, &instance_bound.foundation];
    let mut foundation = CanonicalLirFoundation::empty();
    foundation
        .set_layouts(
            sources
                .iter()
                .flat_map(|source| source.layouts().iter().cloned())
                .collect(),
        )
        .unwrap();
    foundation
        .set_scans(
            sources
                .iter()
                .flat_map(|source| source.scans().iter().cloned())
                .collect(),
        )
        .unwrap();
    foundation
        .set_definition_plans(
            sources
                .iter()
                .flat_map(|source| source.definition_plans().iter().cloned())
                .collect(),
        )
        .unwrap();
    foundation
        .set_definition_atoms(
            sources
                .iter()
                .flat_map(|source| source.definition_atoms().iter().cloned())
                .collect(),
        )
        .unwrap();
    foundation.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            sources
                .iter()
                .flat_map(|source| source.symbol_requests().iter().copied())
                .collect(),
        )
        .unwrap(),
    );
    (
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, foundation).unwrap(),
        vec![value.into(), instance.into()],
    )
}

fn table() -> CanonicalExactLayoutExportsV1 {
    let (foundation, records) = fixture();
    CanonicalExactLayoutExportsV1::try_new(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        records,
        &mut meter(),
    )
    .unwrap()
}

fn decode_records(records: &[&ExactLayoutExportV1]) -> DecodedCanonicalExactLayoutExportsV1 {
    struct Records<'a>(&'a [&'a ExactLayoutExportV1]);
    impl WireEncode for Records<'_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(self.0.len() as u64)?;
            for record in self.0 {
                record.encode(encoder)?;
            }
            Ok(())
        }
    }
    decode_canonical(&encode(&Records(records)).unwrap(), DecodeLimits::default()).unwrap()
}

#[test]
fn layout_table_canonicalizes_roles_without_collapsing_exact_identity() {
    let (foundation, mut records) = fixture();
    let expected = table();
    records.reverse();
    let actual = CanonicalExactLayoutExportsV1::try_new(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        records,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(expected, actual);
    assert_eq!(expected.provider(), ConeIdentity::SINGLE_FILE);
    assert_eq!(expected.target(), LirTargetProfile::DARWIN_AARCH64);
    assert_eq!(expected.records().len(), 2);
    for record in expected.records() {
        assert_eq!(expected.get(record.identity().layout()), Some(record));
    }
    let raw = decode_canonical::<DecodedCanonicalExactLayoutExportsV1>(
        &encode(&expected).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(encode(&raw).unwrap(), encode(&expected).unwrap());
    let validated = raw.validate_against(&expected, &mut meter()).unwrap();
    assert!(Arc::ptr_eq(&validated.0, &expected.0));
}

#[test]
fn layout_table_rejects_duplicate_keys_and_foreign_or_incomplete_foundation() {
    let (foundation, records) = fixture();
    assert!(matches!(
        CanonicalExactLayoutExportsV1::try_new(
            LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            vec![records[0].clone(), records[0].clone()],
            &mut meter()
        ),
        Err(ExactLayoutTableError::Duplicate(_))
    ));
    for (provider, foreign) in [
        (ConeIdentity::CORE, true),
        (ConeIdentity::SINGLE_FILE, false),
    ] {
        let empty =
            OdrFreeLirFoundation::try_new(provider, CanonicalLirFoundation::empty()).unwrap();
        let error = CanonicalExactLayoutExportsV1::try_new(
            LirTargetProfile::DARWIN_AARCH64,
            &empty,
            records.clone(),
            &mut meter(),
        )
        .unwrap_err();
        assert_eq!(matches!(error, ExactLayoutTableError::Provider(_)), foreign);
        assert_eq!(matches!(error, ExactLayoutTableError::Binding(_)), !foreign);
    }
}

#[test]
fn layout_table_reader_rejects_reordering_duplication_and_omission() {
    let expected = table();
    let records = expected.records();
    for raw in [
        decode_records(&[&records[1], &records[0]]),
        decode_records(&[&records[0], &records[0]]),
        decode_records(&[&records[0]]),
    ] {
        assert!(raw.validate_against(&expected, &mut meter()).is_err());
    }
}

#[test]
fn layout_table_preserves_empty_wire_and_charges_shared_work_before_sorting() {
    let foundation =
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty())
            .unwrap();
    let empty = CanonicalExactLayoutExportsV1::try_new(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        vec![],
        &mut meter(),
    )
    .unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    let raw =
        decode_canonical::<DecodedCanonicalExactLayoutExportsV1>(&[0x80], DecodeLimits::default())
            .unwrap();
    assert_eq!(raw.validate_against(&empty, &mut meter()).unwrap(), empty);
    let (foundation, records) = fixture();
    let limits = DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        CanonicalExactLayoutExportsV1::try_new(
            LirTargetProfile::DARWIN_AARCH64,
            &foundation,
            records,
            &mut BudgetMeter::new(limits)
        ),
        Err(ExactLayoutTableError::Resource(_))
    ));
    let expected = table();
    let raw = decode_records(&expected.records().iter().collect::<Vec<_>>());
    assert!(matches!(
        raw.validate_against(&expected, &mut BudgetMeter::new(limits)),
        Err(ExactLayoutTableError::Resource(_))
    ));
}
