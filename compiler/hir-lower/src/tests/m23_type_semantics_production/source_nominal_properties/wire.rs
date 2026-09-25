use super::*;
use hir::{
    DecodedCanonicalNominalSourcePropertiesV1 as Decoded,
    NominalSourcePropertyResolutionError as Error,
};
use scoop_wire::{Encoder, WireEncode};

struct Records<'a>(&'a [hir::NominalSupportPropertyInterfaceV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[test]
fn nominal_source_properties_restore_both_payloads_from_canonical_bytes() {
    for input in [SOURCE, COMBINED] {
        with_source(input, |output, _| {
            let source = table(output);
            let bytes = encode(&source).unwrap();
            let decoded: Decoded = decode_canonical(&bytes).unwrap();
            let mut identities = source_inventory::identity_closure(output);
            let restored = decoded.resolve(&mut identities).unwrap();
            assert_eq!(restored, source);
            assert_eq!(encode(&restored).unwrap(), bytes);
            for record in restored.records() {
                assert_eq!(restored.get(record.declaration()), Some(record));
                let bytes = [
                    vec![0xa3, 1],
                    encode(&record.declaration()).unwrap(),
                    vec![2],
                    encode(record.declaration_access()).unwrap(),
                    vec![3],
                    encode(record.payload()).unwrap(),
                ]
                .concat();
                assert_eq!(encode(record).unwrap(), bytes);
                let mut extra = bytes;
                extra[0] = 0xa4;
                extra.extend([4, 0]);
                assert!(
                    decode_canonical::<hir::DecodedNominalSupportPropertyInterfaceV1>(&extra)
                        .is_err()
                );
            }
        });
    }
}

#[test]
fn nominal_source_properties_reject_duplicate_and_reordered_records() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let first = table.records()[0].clone();
        let duplicate = vec![first.clone(), first];
        assert!(matches!(
            Table::try_new(duplicate.clone()),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
        let mut reverse = table.records().to_vec();
        reverse.reverse();
        for records in [duplicate, reverse] {
            let decoded: Decoded = decode_canonical(&encode(&Records(&records)).unwrap()).unwrap();
            let mut identities = source_inventory::identity_closure(output);
            assert!(matches!(
                decoded.resolve(&mut identities),
                Err(Error::Inventory(
                    hir::SourceInventoryError::NonCanonicalOrder { .. }
                ))
            ));
        }
    });
}

#[test]
fn nominal_property_source_reader_requires_real_identity_closure() {
    with_source(SOURCE, |output, _| {
        let bytes = encode(&table(output)).unwrap();
        let decoded: Decoded = decode_canonical(&bytes).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            decoded.resolve(&mut empty),
            Err(Error::Contract(
                hir::NominalSupportPropertyResolutionError::Identity(_)
            ))
        ));
    });
}
