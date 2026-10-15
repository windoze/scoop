use scoop_identity::*;
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::exact_layout::tests::{Bound, unit};
use crate::*;

fn fixture() -> (ConeLirFoundation, Vec<ExactLayoutExportV1>) {
    let value = unit();
    let value_bound = Bound::value(value.identity().exact_record().clone());
    let instance_bound = Bound::instance(value.identity().exact_record().clone());
    let instance = ExactInstanceLayoutV1::boxed_payload(
        instance_bound.identity,
        &value,
        &instance_bound.foundation,
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
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, foundation).unwrap(),
        vec![value.into(), instance.into()],
    )
}

fn table() -> CanonicalExactLayoutExportsV1 {
    let (foundation, records) = fixture();
    CanonicalExactLayoutExportsV1::try_new(
        crate::target_fixtures::DARWIN_AARCH64,
        &foundation,
        records,
    )
    .unwrap()
}

#[test]
fn layout_table_canonicalizes_roles_without_collapsing_exact_identity() {
    let (foundation, mut records) = fixture();
    let expected = table();
    records.reverse();
    let actual = CanonicalExactLayoutExportsV1::try_new(
        crate::target_fixtures::DARWIN_AARCH64,
        &foundation,
        records,
    )
    .unwrap();
    assert_eq!(expected, actual);
    assert_eq!(expected.provider(), ConeIdentity::SINGLE_FILE);
    assert_eq!(expected.target(), crate::target_fixtures::DARWIN_AARCH64);
    assert_eq!(expected.records().len(), 2);
    for record in expected.records() {
        assert_eq!(expected.get(record.identity().layout()), Some(record));
    }
    let raw = decode_canonical::<DecodedCanonicalExactLayoutExportsV1>(&encode(&expected).unwrap())
        .unwrap();
    assert_eq!(encode(&raw).unwrap(), encode(&expected).unwrap());
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_canonical_authority(
            expected.records()[0].identity().exact_record().clone(),
        )
        .unwrap();
    for record in foundation.layouts() {
        pending
            .register_external_canonical_authority(record.clone())
            .unwrap();
    }
    for record in foundation.scans() {
        pending.register_authority(record.id()).unwrap();
    }
    for record in foundation.definition_plans() {
        pending.register_authority(record.id()).unwrap();
    }
    let actual = raw
        .read(
            expected.target(),
            &foundation,
            &mut pending.finish().unwrap(),
        )
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn layout_table_rejects_duplicate_keys_and_foreign_or_incomplete_foundation() {
    let (foundation, records) = fixture();
    assert!(matches!(
        CanonicalExactLayoutExportsV1::try_new(
            crate::target_fixtures::DARWIN_AARCH64,
            &foundation,
            vec![records[0].clone(), records[0].clone()]
        ),
        Err(ExactLayoutTableError::Duplicate(_))
    ));
    for (provider, foreign) in [
        (ConeIdentity::CORE, true),
        (ConeIdentity::SINGLE_FILE, false),
    ] {
        let empty = ConeLirFoundation::try_new(provider, CanonicalLirFoundation::empty()).unwrap();
        let error = CanonicalExactLayoutExportsV1::try_new(
            crate::target_fixtures::DARWIN_AARCH64,
            &empty,
            records.clone(),
        )
        .unwrap_err();
        assert_eq!(matches!(error, ExactLayoutTableError::Provider(_)), foreign);
        assert_eq!(matches!(error, ExactLayoutTableError::Binding(_)), !foreign);
    }
}

#[test]
fn layout_table_preserves_empty_wire() {
    let foundation =
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty())
            .unwrap();
    let empty = CanonicalExactLayoutExportsV1::try_new(
        crate::target_fixtures::DARWIN_AARCH64,
        &foundation,
        vec![],
    )
    .unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    let raw = decode_canonical::<DecodedCanonicalExactLayoutExportsV1>(&[0x80]).unwrap();
    assert_eq!(
        raw.read(
            empty.target(),
            &foundation,
            &mut PendingIdentityValidation::new().finish().unwrap()
        )
        .unwrap(),
        empty
    );
}
