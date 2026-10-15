use super::*;
use scoop_identity::PendingIdentityValidation;
use scoop_wire::{decode_canonical, encode};

#[test]
fn saved_inline_storage_roundtrips_without_an_exported_layout() {
    let values = [
        value("Byte", 1, 1, RefScan::None),
        value("Empty", 0, 8, RefScan::None),
        value("Word", 8, 8, RefScan::None),
    ];
    let mut pending = PendingIdentityValidation::new();
    for value in &values {
        pending.register_authority(value.exact()).unwrap();
    }
    let mut identities = pending.finish().unwrap();
    for value in &values {
        let bytes = encode(value).unwrap();
        let decoded: DecodedValueLayoutConstituentV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.read(TARGET, &mut identities).unwrap(), *value);
    }
    let aggregate = AggregateStorageLayoutV1::ordinary(TARGET, &input(&values)).unwrap();
    for field in aggregate.fields() {
        let bytes = encode(field.storage()).unwrap();
        let decoded: DecodedFieldStorageV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(
            decoded
                .read(&mut identities, TARGET, aggregate.storage().byte_size())
                .unwrap(),
            *field.storage(),
        );
    }
    let stored = aggregate.fields().last().unwrap().storage();
    let decoded: DecodedFieldStorageV1 = decode_canonical(&encode(stored).unwrap()).unwrap();
    assert!(decoded.read(&mut identities, TARGET, 1).is_err());
}
