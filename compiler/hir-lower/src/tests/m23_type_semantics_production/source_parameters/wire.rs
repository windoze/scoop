use super::*;
use scoop_wire::{Encoder, WireEncode};

mod parameters;
mod references;

struct Records<'a>(&'a [Record]);
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
fn source_parameter_reader_rejects_duplicate_and_reordered_owners() {
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
            assert!(matches!(
                decoded.resolve(&mut source_inventory::identity_closure(output)),
                Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
            ));
        }
    });
}

#[test]
fn source_parameter_reader_rejects_unknown_ids() {
    with_source(SOURCE, |output, _| {
        let bytes = encode(&table(output)).unwrap();
        let decode = || decode_canonical::<Decoded>(&bytes).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            decode().resolve(&mut empty),
            Err(hir::SourceInventoryError::Reference(_))
        ));
    });
}
