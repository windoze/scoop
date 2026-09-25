use super::*;
use crate::{InheritanceCallableDeclarationV1, InheritanceSourceSlotSelectionV1 as Selection};
use scoop_identity::{AccessorRole, PersistentId};
use scoop_wire::{WireDecode, decode_canonical, encode};

mod fixture;
use fixture::Fixture;

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

#[test]
fn selection_wire_preserves_abstract_concrete_default_and_callable_roles() {
    let mut fixture = Fixture::new();
    assert_eq!(encode(&Selection::Abstract).unwrap(), [0xa1, 0, 1]);
    for selection in fixture.selections() {
        let expected = match selection {
            Selection::Abstract => vec![0xa1, 0, 1],
            Selection::Concrete(id) => [vec![0xa2, 0, 2, 1], encode(&id).unwrap()].concat(),
            Selection::InterfaceDefault(id) => [vec![0xa2, 0, 3, 1], encode(&id).unwrap()].concat(),
        };
        assert_eq!(encode(&selection).unwrap(), expected);
        let decoded_selection: DecodedInheritanceSourceSlotSelectionV1 =
            decode_canonical(&expected).unwrap();
        assert_eq!(encode(&decoded_selection).unwrap(), expected);
        let record = InheritanceSourceSlotSelectionRecordV1::new(
            fixture.owners[0],
            fixture.slots[0],
            selection,
        );
        let expected_record = [
            vec![0xa3, 1],
            encode(&record.owner()).unwrap(),
            vec![2],
            encode(&record.slot()).unwrap(),
            vec![3],
            expected,
        ]
        .concat();
        assert_eq!(encode(&record).unwrap(), expected_record);
        let table = CanonicalInheritanceSourceSlotSelectionsV1::try_new(vec![record]).unwrap();
        let decoded: DecodedCanonicalInheritanceSourceSlotSelectionsV1 = decoded(&table);
        assert_eq!(decoded.resolve(&mut fixture).unwrap(), table);
    }
}

#[test]
fn slot_choices_sort_by_owner_then_slot_and_allow_different_choices_per_owner() {
    let mut fixture = Fixture::new();
    let selections = fixture.selections();
    let records: Vec<_> = fixture
        .owners
        .into_iter()
        .enumerate()
        .flat_map(|(owner_index, owner)| {
            fixture
                .slots
                .into_iter()
                .enumerate()
                .map(move |(index, slot)| (owner_index, owner, index, slot))
        })
        .map(|(owner_index, owner, index, slot)| {
            InheritanceSourceSlotSelectionRecordV1::new(
                owner,
                slot,
                selections[owner_index * 3 + index],
            )
        })
        .collect();
    let table = CanonicalInheritanceSourceSlotSelectionsV1::try_new(records.clone()).unwrap();
    let reversed =
        CanonicalInheritanceSourceSlotSelectionsV1::try_new(records.into_iter().rev().collect())
            .unwrap();
    assert_eq!(encode(&table).unwrap(), encode(&reversed).unwrap());
    assert!(
        table
            .records()
            .windows(2)
            .all(|pair| pair[0].key() < pair[1].key())
    );
    for record in table.records() {
        assert_eq!(
            table.get(record.owner(), record.slot()),
            Some(record.selection())
        );
    }
    let decoded: DecodedCanonicalInheritanceSourceSlotSelectionsV1 = decoded(&table);
    assert_eq!(encode(&decoded).unwrap(), encode(&table).unwrap());
    assert_eq!(decoded.resolve(&mut fixture).unwrap(), table);
    let empty = CanonicalInheritanceSourceSlotSelectionsV1::default();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    assert_eq!(empty.get(fixture.owners[0], fixture.slots[0]), None);
}

#[test]
fn duplicate_and_reversed_source_decisions_are_rejected_without_repair() {
    let mut fixture = Fixture::new();
    let record = InheritanceSourceSlotSelectionRecordV1::new(
        fixture.owners[0],
        fixture.slots[0],
        Selection::Abstract,
    );
    let changed = InheritanceSourceSlotSelectionRecordV1::new(
        record.owner(),
        record.slot(),
        fixture.selections()[1],
    );
    for second in [record, changed] {
        assert!(matches!(
            CanonicalInheritanceSourceSlotSelectionsV1::try_new(vec![record, second]),
            Err(SourceInventoryError::NonCanonicalOrder { index: 1, .. })
        ));
        let bytes = [
            vec![0x82],
            encode(&record).unwrap(),
            encode(&second).unwrap(),
        ]
        .concat();
        let decoded: DecodedCanonicalInheritanceSourceSlotSelectionsV1 =
            decode_canonical(&bytes).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture),
            Err(SourceInventoryError::NonCanonicalOrder { index: 1, .. })
        ));
    }
    let other = InheritanceSourceSlotSelectionRecordV1::new(
        fixture.owners[1],
        record.slot(),
        Selection::Abstract,
    );
    let mut pair = [record, other];
    pair.sort_unstable_by_key(|record| std::cmp::Reverse(record.key()));
    let bytes = [
        vec![0x82],
        encode(&pair[0]).unwrap(),
        encode(&pair[1]).unwrap(),
    ]
    .concat();
    let decoded: DecodedCanonicalInheritanceSourceSlotSelectionsV1 =
        decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { index: 1, .. })
    ));
}

#[test]
fn source_choice_wire_rejects_unknown_tags_and_inexact_products() {
    for bytes in [
        vec![0xa1, 0, 0],
        vec![0xa1, 0, 4],
        vec![0xa1, 0, 2],
        vec![0xa1, 0, 3],
        vec![0xa2, 0, 1, 1, 0],
        vec![0xa2, 0, 2, 1, 0xa2, 0, 4, 1, 0],
    ] {
        assert!(decode_canonical::<DecodedInheritanceSourceSlotSelectionV1>(&bytes).is_err());
    }
    let fixture = Fixture::new();
    let record = InheritanceSourceSlotSelectionRecordV1::new(
        fixture.owners[0],
        fixture.slots[0],
        Selection::Abstract,
    );
    for header in [0xa2, 0xa4] {
        let mut bytes = encode(&record).unwrap();
        bytes[0] = header;
        assert!(decode_canonical::<DecodedInheritanceSourceSlotSelectionRecordV1>(&bytes).is_err());
    }
}
