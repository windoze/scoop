use super::*;
use scoop_wire::{Encoder, WireEncode};

fn columns(e: &hir::TypeDeclarationSourceEntriesV1) -> [Vec<u8>; 9] {
    [
        encode(&e.required_protected).unwrap(),
        encode(&e.nominals).unwrap(),
        encode(&e.constructors).unwrap(),
        encode(&e.properties).unwrap(),
        encode(&e.callables).unwrap(),
        encode(&e.inheritance).unwrap(),
        encode(&e.interfaces).unwrap(),
        encode(&e.selections).unwrap(),
        encode(&e.dispatch_callables).unwrap(),
    ]
}
fn product(columns: [Vec<u8>; 9]) -> Vec<u8> {
    let mut bytes = vec![0xa9];
    for (index, column) in columns.into_iter().enumerate() {
        bytes.push(index as u8 + 1);
        bytes.extend(column);
    }
    bytes
}

#[test]
fn declaration_domain_empty_wire_still_contains_all_nine_fields() {
    with_domain("fun unrelated(): Int = 1", |_, _, _, domain, _| {
        let expected: Vec<_> = std::iter::once(0xa9)
            .chain((1..=9).flat_map(|field| [field, 0x80]))
            .collect();
        assert_eq!(encode(domain).unwrap(), expected);
        for header in [0xa0, 0xa8, 0xaa] {
            let mut bytes = expected.clone();
            bytes[0] = header;
            assert!(
                decode_canonical::<hir::DecodedTypeDeclarationSourceAuthorityV1>(
                    &bytes,
                    DecodeLimits::default()
                )
                .is_err()
            );
        }
        let mut bytes = expected;
        bytes[17] = 10;
        assert!(
            decode_canonical::<hir::DecodedTypeDeclarationSourceAuthorityV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    });
}

fn unordered<T: WireEncode>(records: &[T], duplicate: bool) -> Vec<u8> {
    assert!(records.len() >= 2);
    struct Rows<'a, T>(&'a [T], bool);
    impl<T: WireEncode> WireEncode for Rows<'_, T> {
        fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            e.array(self.0.len() as u64)?;
            if self.1 {
                self.0[0].encode(e)?;
                for row in &self.0[..self.0.len() - 1] {
                    row.encode(e)?;
                }
            } else {
                for row in self.0.iter().rev() {
                    row.encode(e)?;
                }
            }
            Ok(())
        }
    }
    encode(&Rows(records, duplicate)).unwrap()
}
#[test]
fn declaration_domain_reader_preserves_canonical_order_and_rejects_unknown_refs() {
    with_domain(SOURCE, |_, fixture, _, domain, _| {
        let e = domain.entries();
        for duplicate in [false, true] {
            let malformed = [
                (1, unordered(e.required_protected.values(), duplicate)),
                (2, unordered(e.nominals.records(), duplicate)),
                (3, unordered(e.constructors.records(), duplicate)),
                (4, unordered(e.properties.records(), duplicate)),
                (5, unordered(e.callables.records(), duplicate)),
                (6, unordered(e.inheritance.records(), duplicate)),
                (7, unordered(e.interfaces.records(), duplicate)),
                (8, unordered(e.selections.records(), duplicate)),
                (9, unordered(e.dispatch_callables.records(), duplicate)),
            ];
            for (field, malformed) in malformed {
                let mut columns = columns(e);
                columns[field - 1] = malformed;
                let decoded: hir::DecodedTypeDeclarationSourceAuthorityV1 =
                    decode_canonical(&product(columns), DecodeLimits::default()).unwrap();
                assert!(
                    decoded
                        .resolve(&mut fixture.identities, &mut meter())
                        .is_err(),
                    "field {field}, duplicate={duplicate}"
                );
            }
        }
        let decoded: hir::DecodedTypeDeclarationSourceAuthorityV1 =
            decode_canonical(&encode(domain).unwrap(), DecodeLimits::default()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(decoded.resolve(&mut empty, &mut meter()).is_err());
    });
}

#[test]
fn declaration_domain_resolver_uses_one_budget_for_every_child_table() {
    with_domain(SOURCE, |_, fixture, _, source, _| {
        let bytes = encode(source).unwrap();
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
        ] {
            let decoded: hir::DecodedTypeDeclarationSourceAuthorityV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert!(
                decoded
                    .resolve(&mut fixture.identities, &mut BudgetMeter::new(limits))
                    .is_err(),
                "{limits:?}"
            );
        }
    });
}
