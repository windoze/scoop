use super::*;
use hir::{
    DecodedCanonicalNominalSourceConstructorsV1 as Decoded,
    NominalSourceConstructorResolutionError as Error,
};
use scoop_wire::{Encoder, WireEncode};

struct Records<'a>(&'a [hir::NominalSupportConstructorInterfaceV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(e)?;
        }
        Ok(())
    }
}

#[test]
fn nominal_constructor_sources_restore_exact_contract_bytes_and_reject_extra_fields() {
    for input in [SOURCE, PARAMETERS, INHERITANCE, EFFECTS] {
        with_source(input, |output, _| {
            let source = table(output);
            let bytes = encode(&source).unwrap();
            let decoded: Decoded = decode_canonical(&bytes).unwrap();
            let restored = decoded
                .resolve(&mut source_inventory::identity_closure(output))
                .unwrap();
            assert_eq!(restored, source);
            assert_eq!(encode(&restored).unwrap(), bytes);
            for record in restored.records() {
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
                    decode_canonical::<hir::DecodedNominalSupportConstructorInterfaceV1>(&extra)
                        .is_err()
                );
            }
        });
    }
}

#[test]
fn nominal_constructor_reader_rejects_duplicate_reordered_and_unknown_declarations() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let duplicate = vec![table.records()[0].clone(); 2];
        assert!(matches!(
            Table::try_new(duplicate.clone()),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
        let mut reverse = table.records().to_vec();
        reverse.reverse();
        for records in [duplicate, reverse] {
            let decoded: Decoded = decode_canonical(&encode(&Records(&records)).unwrap()).unwrap();
            assert!(matches!(
                decoded.resolve(&mut source_inventory::identity_closure(output)),
                Err(Error::Inventory(
                    hir::SourceInventoryError::NonCanonicalOrder { .. }
                ))
            ));
        }
        let decoded: Decoded = decode_canonical(&encode(&table).unwrap()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            decoded.resolve(&mut empty),
            Err(Error::Contract(
                hir::ProtectedCallableInterfaceResolutionError::Identity(_)
            ))
        ));
    });
}
