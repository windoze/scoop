use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    FieldIdentityKey, PackagePath, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

mod c_layout;

pub(super) const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

fn source(name: &str) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    )
}

fn exact(name: &str) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source(name)).unwrap(),
    ))
    .unwrap()
}

fn field(index: usize) -> PersistentFieldId {
    PersistentFieldId::from_key(
        &FieldIdentityKey::source_declared(
            &source("Fields"),
            CanonicalIdentifier::new(&format!("f{index}")).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn value(
    name: &str,
    size: u64,
    alignment: u64,
    scan: RefScan,
) -> ValueLayoutConstituentV1 {
    let storage = if size == 0 {
        ValueStorageLayoutV1::zero_sized(alignment)
    } else {
        ValueStorageLayoutV1::inline(size, alignment, scan)
    }
    .unwrap();
    ValueLayoutConstituentV1::new(
        TARGET,
        LayoutKey::darwin_aarch64(exact(name), RepresentationRole::ManagedValue),
        storage,
    )
    .unwrap()
}

fn input<'a>(values: &'a [ValueLayoutConstituentV1]) -> Vec<DeclaredFieldStorageV1<'a>> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| DeclaredFieldStorageV1::new(field(index), value))
        .collect()
}

fn class_key(name: &str) -> LayoutKey {
    LayoutKey::darwin_aarch64(exact(name), RepresentationRole::ManagedObject)
}

#[test]
fn ordinary_fields_keep_zst_zero_offsets_and_alignment_without_cursor_padding() {
    let values = [
        value("Byte", 1, 1, RefScan::None),
        value("AlignedEmpty", 0, 16, RefScan::None),
        value("NextByte", 1, 1, RefScan::None),
    ];
    let aggregate = AggregateStorageLayoutV1::ordinary(TARGET, &input(&values)).unwrap();
    assert_eq!(
        aggregate
            .fields
            .iter()
            .map(|field| field.storage.offset().get())
            .collect::<Vec<_>>(),
        [0, 0, 1]
    );
    assert_eq!(
        (
            aggregate.storage.byte_size(),
            aggregate.storage.alignment().get()
        ),
        (16, 16)
    );
    let empty = AggregateStorageLayoutV1::ordinary(TARGET, &[]).unwrap();
    assert_eq!(
        (empty.storage.byte_size(), empty.storage.alignment().get()),
        (0, 1)
    );
    let zst = AggregateStorageLayoutV1::ordinary(
        TARGET,
        &[DeclaredFieldStorageV1::new(field(0), &values[1])],
    )
    .unwrap();
    assert_eq!(
        (zst.storage.byte_size(), zst.storage.alignment().get()),
        (0, 16)
    );
}

#[test]
fn aggregate_scan_uses_checked_physical_offsets_and_tail_padding() {
    let values = [
        value("Byte", 1, 1, RefScan::None),
        value("Ref", 8, 8, RefScan::References(vec![0])),
    ];
    let aggregate = AggregateStorageLayoutV1::ordinary(TARGET, &input(&values)).unwrap();
    assert_eq!(aggregate.fields[1].storage.offset().get(), 8);
    assert_eq!(
        aggregate.storage.nonzero().unwrap().scan().as_ref_scan(),
        &RefScan::References(vec![8])
    );
    assert_eq!(aggregate.storage.byte_size(), 16);
}

#[test]
fn class_prefix_preserves_base_tail_padding_and_complete_field_order() {
    let byte = value("Byte", 1, 1, RefScan::None);
    let base = ClassStorageLayoutV1::replay(
        TARGET,
        class_key("Base"),
        ClassBaseStorageV1::NoBase,
        &[DeclaredFieldStorageV1::new(field(0), &byte)],
    )
    .unwrap();
    assert_eq!(base.shape().minimum_size(), 24);
    assert_eq!(base.complete_fields()[0].storage.offset().get(), 16);
    let derived = ClassStorageLayoutV1::replay(
        TARGET,
        class_key("Derived"),
        ClassBaseStorageV1::Base(&base),
        &[DeclaredFieldStorageV1::new(field(1), &byte)],
    )
    .unwrap();
    assert_eq!(derived.declared_fields()[0].storage.offset().get(), 24);
    assert_eq!(derived.shape().minimum_size(), 32);
    assert_eq!(derived.complete_fields()[0], base.complete_fields()[0]);
    derived
        .validate_projection(derived.base_prefix(), derived.complete_fields())
        .unwrap();
    assert_eq!(
        derived.validate_projection(derived.base_prefix(), derived.declared_fields()),
        Err(StorageReplayError::ClassProjectionMismatch)
    );
    assert_eq!(
        ClassStorageLayoutV1::replay(
            TARGET,
            class_key("Base"),
            ClassBaseStorageV1::Base(&derived),
            &[]
        ),
        Err(StorageReplayError::ClassCycle(exact("Base")))
    );
}

#[test]
fn class_header_survives_empty_and_zst_only_own_fields() {
    let zst = value("Empty", 0, 16, RefScan::None);
    let class = ClassStorageLayoutV1::replay(
        TARGET,
        class_key("EmptyClass"),
        ClassBaseStorageV1::NoBase,
        &[DeclaredFieldStorageV1::new(field(0), &zst)],
    )
    .unwrap();
    assert_eq!(
        (
            class.shape().minimum_size(),
            class.shape().instance_alignment()
        ),
        (16, 16)
    );
    assert_eq!(class.complete_fields()[0].storage.offset().get(), 0);
}

#[test]
fn replay_rejects_duplicate_fields_wrong_role_and_size_overflow() {
    let byte = value("Byte", 1, 1, RefScan::None);
    let fields = [DeclaredFieldStorageV1::new(field(0), &byte); 2];
    assert_eq!(
        AggregateStorageLayoutV1::ordinary(TARGET, &fields),
        Err(StorageReplayError::DuplicateField(field(0)))
    );
    assert_eq!(
        ValueLayoutConstituentV1::new(
            TARGET,
            class_key("WrongRole"),
            ValueStorageLayoutV1::zero_sized(1).unwrap()
        ),
        Err(StorageReplayError::RepresentationRoleMismatch)
    );
    let huge = value("Huge", i64::MAX as u64, 1, RefScan::None);
    let values = [huge, byte];
    assert!(matches!(
        AggregateStorageLayoutV1::ordinary(TARGET, &input(&values)),
        Err(StorageReplayError::Shape(
            TypeInstanceShapeError::ManagedObjectTooLarge { .. }
        ))
    ));
}

#[test]
fn field_wire_replays_refs_and_rejects_forged_offset_exact_and_layout() {
    let values = [
        value("Empty", 0, 16, RefScan::None),
        value("Word", 8, 8, RefScan::None),
    ];
    let aggregate = AggregateStorageLayoutV1::ordinary(TARGET, &input(&values)).unwrap();
    for field in aggregate.fields() {
        let expected = field.storage();
        let bytes = encode(expected).unwrap();
        let decoded =
            decode_canonical::<DecodedFieldStorageV1>(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.validate_against(expected).unwrap(), *expected);
        for index in [6, 39] {
            let mut forged = bytes.clone();
            forged[index] ^= 1;
            let decoded =
                decode_canonical::<DecodedFieldStorageV1>(&forged, DecodeLimits::default())
                    .unwrap();
            assert_eq!(
                decoded.validate_against(expected),
                Err(StorageReplayError::FieldWireMismatch)
            );
        }
    }
    let stored = aggregate.fields()[1].storage();
    let mut bytes = encode(stored).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    assert_eq!(
        decode_canonical::<DecodedFieldStorageV1>(&bytes, DecodeLimits::default())
            .unwrap()
            .validate_against(stored),
        Err(StorageReplayError::FieldWireMismatch)
    );
}
