use super::*;
use scoop_wire::{Encoder, WireEncode};
fn rows(records: &[hir::DefaultSourceTemplateV1]) -> Vec<u8> {
    struct Rows<'a>(&'a [hir::DefaultSourceTemplateV1]);
    impl WireEncode for Rows<'_> {
        fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            e.array(self.0.len() as u64)?;

            for record in self.0 {
                record.index_locals().unwrap().encode(e)?;
            }
            Ok(())
        }
    }
    encode(&Rows(records)).unwrap()
}
#[test]
fn source_table_producer_sorts_but_reader_rejects_duplicate_and_reversed_keys() {
    with_hir_source(SOURCE, |output, _| {
        let production = Production::from_dependency_hir(output).unwrap();
        let original = production.templates();
        let mut reversed = original.records().to_vec();
        reversed.reverse();
        assert_eq!(&Table::try_new(reversed.clone()).unwrap(), original);
        let input: Decoded = decode_canonical(&rows(&reversed)).unwrap();
        assert!(matches!(
            input.resolve(&mut identity_closure(output)),
            Err(hir::DefaultSourceTemplateTableResolutionError::Table(
                hir::DefaultSourceTemplateTableBuildError::NonCanonicalOrder { index: 1 }
            ))
        ));
        let duplicate = vec![original.records()[0].clone(), original.records()[0].clone()];
        assert!(matches!(
            Table::try_new(duplicate.clone()),
            Err(hir::DefaultSourceTemplateTableBuildError::Duplicate(_))
        ));
        let input: Decoded = decode_canonical(&rows(&duplicate)).unwrap();
        assert!(matches!(
            input.resolve(&mut identity_closure(output)),
            Err(hir::DefaultSourceTemplateTableResolutionError::Table(
                hir::DefaultSourceTemplateTableBuildError::Duplicate(_)
            ))
        ));
    });
}
#[test]
fn source_table_wire_preserves_explicit_empty_and_rejects_non_array() {
    let empty = Table::try_new(vec![]).unwrap();
    assert_eq!(bytes(&empty), [0x80]);
    assert!(decode_canonical::<Decoded>(&[0xa0]).is_err());
    with_hir_source(SOURCE, |output, _| {
        let production = Production::from_dependency_hir(output).unwrap();
        let input: Decoded = decode_canonical(&bytes(production.templates())).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            input.resolve(&mut empty),
            Err(hir::DefaultSourceTemplateTableResolutionError::Template {
                index: 0,
                error: hir::DefaultSourceTemplateResolutionError::Key(_)
            })
        ));
    });
}
