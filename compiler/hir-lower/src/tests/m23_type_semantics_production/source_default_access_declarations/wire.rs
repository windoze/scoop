use super::*;
use hir::DefaultSourceAccessDeclarationResolutionError as Error;
use scoop_wire::{Encoder, WireEncode};
struct Records<'a>(&'a [Record]);
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
fn default_access_declaration_reader_roundtrips_exact_bytes_and_rejects_shape_corruption() {
    for source in [SOURCE, COMBINATIONS] {
        with_hir_source(source, |output, _| {
            let table = table(output);
            let bytes = encode(&table).unwrap();
            let decoded: Decoded = decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
            let restored = decoded.resolve(&mut identity_closure(output)).unwrap();
            assert_eq!(restored, table);
            assert_eq!(encode(&restored).unwrap(), bytes);
            for record in table.records() {
                let mut bytes = encode(record).unwrap();
                assert_eq!(
                    bytes,
                    [
                        vec![0xa2, 1],
                        encode(&record.subject()).unwrap(),
                        vec![2],
                        encode(record.declaration_access()).unwrap()
                    ]
                    .concat()
                );
                bytes[0] = 0xa3;
                bytes.extend([3, 0]);
                assert!(
                    decode_canonical::<hir::DecodedDefaultSourceAccessDeclarationV1>(&bytes)
                        .is_err()
                );
                let bytes = [vec![0xa1, 1], encode(&record.subject()).unwrap()].concat();
                assert!(
                    decode_canonical::<hir::DecodedDefaultSourceAccessDeclarationV1>(&bytes)
                        .is_err()
                );
            }
            let sample = &table.records()[0];
            for tag in 9..=19 {
                let mut subject = encode(&sample.subject()).unwrap();
                assert_eq!(&subject[..2], &[0xa2, 0]);
                subject[2] = tag;
                let bytes = [
                    vec![0xa2, 1],
                    subject,
                    vec![2],
                    encode(sample.declaration_access()).unwrap(),
                ]
                .concat();
                assert!(
                    decode_canonical::<hir::DecodedDefaultSourceAccessDeclarationV1>(&bytes)
                        .is_err(),
                    "tag={tag}"
                );
            }
        });
    }
}
#[test]
fn default_access_declaration_reader_rejects_order_duplicates_and_unknown_identity() {
    with_hir_source(SOURCE, |output, _| {
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
                decoded.resolve(&mut identity_closure(output)),
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
            Err(Error::Identity(_))
        ));
    });
}
