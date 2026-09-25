use super::*;
use hir::{
    DecodedCanonicalInheritanceSourcePropertiesV1 as Decoded,
    InheritanceSourcePropertyResolutionError as Error,
};
use scoop_wire::{Encoder, WireEncode};

mod constants;

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
fn property_sources_reuse_three_required_contract_fields() {
    with_source(SOURCE, |output, _| {
        for record in table(output).records() {
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
            let mut incomplete = bytes;
            incomplete[0] = 0xa2;
            assert!(
                decode_canonical::<hir::DecodedNominalSupportPropertyInterfaceV1>(&incomplete)
                    .is_err()
            );
        }
    });
}

#[test]
fn property_source_reader_rejects_duplicate_and_reordered_contracts() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let duplicate = vec![table.records()[0].clone(), table.records()[0].clone()];
        assert!(matches!(
            Table::try_new(duplicate.clone()),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
        let mut reversed = table.records().to_vec();
        reversed.reverse();
        for records in [duplicate, reversed] {
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
fn property_source_reader_rejects_unknown_identities() {
    with_source(SOURCE, |output, _| {
        let bytes = encode(&table(output)).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        let decode = || decode_canonical::<Decoded>(&bytes).unwrap();
        assert!(matches!(
            decode().resolve(&mut empty),
            Err(Error::Contract(
                hir::NominalSupportPropertyResolutionError::Identity(_)
            ))
        ));
    });
}
