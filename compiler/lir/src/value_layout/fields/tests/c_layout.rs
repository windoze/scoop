use std::num::NonZeroU64;

use scoop_identity::{
    CLayoutByteAlignment, CLayoutOverride, CanonicalCAbiLayout, CanonicalCAbiLayoutField,
    CanonicalCAbiLayoutFingerprintRecord, CanonicalCStorageType, IntegerBitWidth, Signedness,
};

use super::*;

fn integer(value: &ValueLayoutConstituentV1, bits: IntegerBitWidth) -> CanonicalCStorageType {
    CanonicalCStorageType::Integer {
        exact_type: value.exact(),
        signedness: Signedness::Signed,
        bit_width: bits,
    }
}

fn contract(
    name: &str,
    size: u64,
    alignment: u64,
    aligned: CLayoutOverride,
    packed: CLayoutOverride,
    fields: Vec<CanonicalCAbiLayoutField>,
) -> CanonicalCAbiLayoutFingerprintRecord {
    CanonicalCAbiLayoutFingerprintRecord::new(CanonicalCAbiLayout::new(
        exact(name),
        size,
        NonZeroU64::new(alignment).unwrap(),
        aligned,
        packed,
        fields,
    ))
    .unwrap()
}

#[test]
fn packed_and_aligned_c_layouts_replay_the_canonical_contract_separately() {
    let values = [
        value("Byte", 1, 1, RefScan::None),
        value("Long", 8, 8, RefScan::None),
    ];
    let fields = vec![
        CanonicalCAbiLayoutField::new(field(0), 0, integer(&values[0], IntegerBitWidth::Bits8)),
        CanonicalCAbiLayoutField::new(field(1), 1, integer(&values[1], IntegerBitWidth::Bits64)),
    ];
    let canonical = contract(
        "Packed",
        16,
        16,
        CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes16),
        CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes1),
        fields,
    );
    let replay = CLayoutStorageReplayV1::replay(TARGET, &canonical, &input(&values), &[]).unwrap();
    assert_eq!(replay.aggregate().fields()[1].storage().offset().get(), 1);
    assert_eq!(replay.aggregate().fields()[1].access_alignment().get(), 1);
    assert_eq!(replay.aggregate().storage().alignment().get(), 16);
    let ordinary = AggregateStorageLayoutV1::ordinary(TARGET, &input(&values)).unwrap();
    assert_eq!(ordinary.fields()[1].storage().offset().get(), 8);
    assert_eq!(ordinary.storage().alignment().get(), 8);
}

#[test]
fn c_replay_rejects_zst_managed_scan_empty_and_forged_shape() {
    for value in [
        value("Empty", 0, 1, RefScan::None),
        value("Ref", 8, 8, RefScan::References(vec![0])),
    ] {
        let canonical = contract(
            "BadField",
            8,
            8,
            CLayoutOverride::Natural,
            CLayoutOverride::Natural,
            vec![CanonicalCAbiLayoutField::new(
                field(0),
                0,
                integer(&value, IntegerBitWidth::Bits64),
            )],
        );
        assert!(matches!(
            CLayoutStorageReplayV1::replay(
                TARGET,
                &canonical,
                &[DeclaredFieldStorageV1::new(field(0), &value)],
                &[]
            ),
            Err(StorageReplayError::InvalidCField(_))
        ));
    }
    let empty = contract(
        "EmptyC",
        0,
        1,
        CLayoutOverride::Natural,
        CLayoutOverride::Natural,
        vec![],
    );
    assert_eq!(
        CLayoutStorageReplayV1::replay(TARGET, &empty, &[], &[]),
        Err(StorageReplayError::EmptyCLayout)
    );
    let byte = value("Byte", 1, 1, RefScan::None);
    let wrong = contract(
        "Wrong",
        8,
        8,
        CLayoutOverride::Natural,
        CLayoutOverride::Natural,
        vec![CanonicalCAbiLayoutField::new(
            field(0),
            0,
            integer(&byte, IntegerBitWidth::Bits64),
        )],
    );
    assert!(matches!(
        CLayoutStorageReplayV1::replay(
            TARGET,
            &wrong,
            &[DeclaredFieldStorageV1::new(field(0), &byte)],
            &[]
        ),
        Err(StorageReplayError::InvalidCField(_))
    ));
}

#[test]
fn nested_c_fields_require_the_same_checked_layout_fingerprint() {
    let byte = value("Byte", 1, 1, RefScan::None);
    let inner = contract(
        "Inner",
        1,
        1,
        CLayoutOverride::Natural,
        CLayoutOverride::Natural,
        vec![CanonicalCAbiLayoutField::new(
            field(0),
            0,
            integer(&byte, IntegerBitWidth::Bits8),
        )],
    );
    let inner_replay = CLayoutStorageReplayV1::replay(
        TARGET,
        &inner,
        &[DeclaredFieldStorageV1::new(field(0), &byte)],
        &[],
    )
    .unwrap();
    let inner_value = value("Inner", 1, 1, RefScan::None);
    let outer = contract(
        "Outer",
        1,
        1,
        CLayoutOverride::Natural,
        CLayoutOverride::Natural,
        vec![CanonicalCAbiLayoutField::new(
            field(0),
            0,
            CanonicalCStorageType::Struct {
                exact_type: inner_value.exact(),
                layout: inner.fingerprint(),
            },
        )],
    );
    let inputs = [DeclaredFieldStorageV1::new(field(0), &inner_value)];
    assert_eq!(
        CLayoutStorageReplayV1::replay(TARGET, &outer, &inputs, &[]),
        Err(StorageReplayError::MissingNestedCLayout(
            inner.fingerprint()
        ))
    );
    CLayoutStorageReplayV1::replay(TARGET, &outer, &inputs, &[&inner_replay]).unwrap();
}
