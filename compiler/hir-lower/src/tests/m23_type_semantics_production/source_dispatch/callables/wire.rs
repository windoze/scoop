use super::*;
use hir::InheritanceSourceCallableResolutionError as Error;
use scoop_wire::{Encoder, WireEncode};

mod resources;

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
fn source_callable_wire_has_four_required_fields_and_preserves_all_roles() {
    with_hir_source(CALLABLES, |output, _| {
        let table = table(output);
        for record in table.records() {
            let expected = [
                vec![0xa4, 1],
                encode(&record.declaration()).unwrap(),
                vec![2],
                encode(record.signature()).unwrap(),
                vec![3],
                encode(&record.modality()).unwrap(),
                vec![4],
                encode(record.declaration_access()).unwrap(),
            ]
            .concat();
            assert_eq!(encode(record).unwrap(), expected);
            let decoded: hir::DecodedInheritanceSourceCallableV1 =
                decode_canonical(&expected, DecodeLimits::default()).unwrap();
            assert_eq!(encode(&decoded).unwrap(), expected);
            let mut malformed = expected;
            malformed[0] = 0xa3;
            assert!(
                decode_canonical::<hir::DecodedInheritanceSourceCallableV1>(
                    &malformed,
                    DecodeLimits::default()
                )
                .is_err()
            );
        }
        for role in [0, 1, 2] {
            assert!(
                table
                    .records()
                    .iter()
                    .any(|record| match record.declaration() {
                        Declaration::Function(_) => role == 0,
                        Declaration::Getter(_) => role == 1,
                        Declaration::Setter(_) => role == 2,
                    })
            );
        }
    });
}

#[test]
fn source_callable_reader_rejects_duplicate_and_unsorted_declarations_without_repair() {
    with_hir_source(CALLABLES, |output, _| {
        let table = table(output);
        let first = table.records()[0].clone();
        let duplicate = vec![first.clone(), first];
        assert!(matches!(
            Table::try_new(duplicate.clone(), &mut meter()),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
        let mut reversed = table.records().to_vec();
        reversed.reverse();
        for invalid in [duplicate, reversed] {
            let decoded: DecodedTable = decode_canonical(
                &encode(&Records(&invalid)).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            let mut identities = super::super::super::source_inventory::identity_closure(output);
            assert!(matches!(
                decoded.resolve(&mut identities, &mut meter()),
                Err(Error::Inventory(
                    hir::SourceInventoryError::NonCanonicalOrder { .. }
                ))
            ));
        }
    });
}

#[test]
fn source_callable_reader_rejects_an_unpublished_typed_declaration() {
    with_hir_source(CALLABLES, |output, _| {
        use scoop_identity::*;
        let unknown =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                SourceDeclarationSite::new(
                    output.output().export.cone,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new("unpublished").unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let table = table(output);
        let first = &table.records()[0];
        let invalid = Table::try_new(
            vec![Record::new(
                Declaration::Function(unknown),
                first.signature().clone(),
                first.modality(),
                first.declaration_access().clone(),
            )],
            &mut meter(),
        )
        .unwrap();
        let decoded: DecodedTable =
            decode_canonical(&encode(&invalid).unwrap(), DecodeLimits::default()).unwrap();
        let mut identities = super::super::super::source_inventory::identity_closure(output);
        assert!(matches!(
            decoded.resolve(&mut identities, &mut meter()),
            Err(Error::Identity(_))
        ));
    });
}
