use super::*;
use crate::value_layout::fields::tests::{TARGET, value};
use scoop_identity::NonEmptyVec;
use scoop_wire::{decode_canonical, encode};

fn tuple(
    values: &[&ValueLayoutConstituentV1],
) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Tuple(
        NonEmptyVec::new(values.iter().map(|value| value.exact()).collect()).unwrap(),
    ))
    .unwrap()
}

#[test]
fn tuple_replay_preserves_exact_positions_zst_alignment_and_reference_offsets() {
    let values = [
        value("Byte", 1, 1, RefScan::None),
        value("Empty16", 0, 16, RefScan::None),
        value("Byte2", 1, 1, RefScan::None),
        value("Managed", 8, 8, RefScan::References(vec![0])),
    ];
    let refs: Vec<_> = values.iter().collect();
    let exact = tuple(&refs);
    let layout = TupleStorageLayoutV1::replay(TARGET, &exact, &refs).unwrap();
    assert_eq!(layout.exact(), exact.id());
    assert_eq!(layout.storage().byte_size(), 16);
    assert_eq!(layout.storage().alignment().get(), 16);
    assert_eq!(
        layout
            .elements()
            .iter()
            .map(|field| field.storage().offset().get())
            .collect::<Vec<_>>(),
        [0, 0, 1, 8]
    );
    for (ordinal, field) in layout.elements().iter().enumerate() {
        assert_eq!(field.index().tuple(), exact.id());
        assert_eq!(field.index().ordinal(), ordinal as u64);
        let bytes = encode(field).unwrap();
        let decoded: DecodedTupleElementStorageV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.validate_against(field).unwrap(), *field);
    }
    assert_eq!(
        layout.storage().nonzero().unwrap().scan().as_ref_scan(),
        &RefScan::References(vec![8])
    );
}

#[test]
fn all_zst_tuple_has_canonical_zero_offsets_and_maximum_alignment() {
    let values = [
        value("Unit", 0, 1, RefScan::None),
        value("Aligned", 0, 16, RefScan::None),
    ];
    let refs: Vec<_> = values.iter().collect();
    let layout = TupleStorageLayoutV1::replay(TARGET, &tuple(&refs), &refs).unwrap();
    assert_eq!(
        (
            layout.storage().byte_size(),
            layout.storage().alignment().get()
        ),
        (0, 16)
    );
    assert!(
        layout
            .elements()
            .iter()
            .all(|element| element.storage().offset().get() == 0)
    );
}

#[test]
fn tuple_replay_rejects_arity_and_exact_type_mismatches() {
    let first = value("First", 8, 8, RefScan::None);
    let second = value("Second", 8, 8, RefScan::None);
    let exact = tuple(&[&first]);
    assert!(matches!(
        TupleStorageLayoutV1::replay(TARGET, &exact, &[]),
        Err(TupleStorageReplayError::ArityMismatch)
    ));
    assert!(matches!(
        TupleStorageLayoutV1::replay(TARGET, &exact, &[&second]),
        Err(TupleStorageReplayError::ElementTypeMismatch)
    ));
    let nominal = CborIdentityRecord::from_key(ExactTypeKey::RawPointer(first.exact())).unwrap();
    assert!(matches!(
        TupleStorageLayoutV1::replay(TARGET, &nominal, &[&first]),
        Err(TupleStorageReplayError::ExpectedTuple)
    ));
}

#[test]
fn tuple_wire_cannot_change_position_access_alignment_or_zst_offset() {
    let value = value("Empty", 0, 8, RefScan::None);
    let exact = tuple(&[&value]);
    let layout = TupleStorageLayoutV1::replay(TARGET, &exact, &[&value]).unwrap();
    let field = &layout.elements()[0];
    let bytes = encode(field).unwrap();
    for index in [2, bytes.len() - 1] {
        let mut changed = bytes.clone();
        changed[index] = 1;
        let decoded: DecodedTupleElementStorageV1 = decode_canonical(&changed).unwrap();
        assert!(decoded.validate_against(field).is_err());
    }
    let whole = StorageGeometryV1::new(TARGET, 16, 8).unwrap();
    assert_eq!(
        FieldStorageV1::within(&value, 8, whole),
        Err(StorageReplayError::InvalidFieldPlacement)
    );
}

#[test]
fn physical_field_placement_checks_bounds_alignment_and_overflow() {
    let field = value("Word", 8, 8, RefScan::None);
    let whole = StorageGeometryV1::new(TARGET, 16, 8).unwrap();
    for offset in [1, 16, u64::MAX - 7] {
        assert_eq!(
            FieldStorageV1::within(&field, offset, whole),
            Err(StorageReplayError::InvalidFieldPlacement)
        );
    }
    assert_eq!(
        FieldStorageV1::within(&field, 8, whole)
            .unwrap()
            .offset()
            .get(),
        8
    );
    assert!(
        FieldStorageV1::within(&field, 0, StorageGeometryV1::new(TARGET, 16, 4).unwrap()).is_err()
    );
}
