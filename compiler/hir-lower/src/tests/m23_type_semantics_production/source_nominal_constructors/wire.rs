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
            let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let restored = decoded
                .resolve(
                    &mut source_inventory::identity_closure(output),
                    &mut meter(),
                )
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
                    decode_canonical::<hir::DecodedNominalSupportConstructorInterfaceV1>(
                        &extra,
                        DecodeLimits::default()
                    )
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
            Table::try_new(duplicate.clone(), &mut meter()),
            Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
        ));
        let mut reverse = table.records().to_vec();
        reverse.reverse();
        for records in [duplicate, reverse] {
            let decoded: Decoded = decode_canonical(
                &encode(&Records(&records)).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert!(matches!(
                decoded.resolve(
                    &mut source_inventory::identity_closure(output),
                    &mut meter()
                ),
                Err(Error::Inventory(
                    hir::SourceInventoryError::NonCanonicalOrder { .. }
                ))
            ));
        }
        let decoded: Decoded =
            decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            decoded.resolve(&mut empty, &mut meter()),
            Err(Error::Contract(
                hir::ProtectedCallableInterfaceResolutionError::Identity(_)
            ))
        ));
    });
}

#[test]
fn nominal_constructor_projection_and_reader_share_resource_limits() {
    with_source(SOURCE, |output, _| {
        let required = required(output);
        let table = table(output);
        let decoded: Decoded =
            decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
        let mut identities = source_inventory::identity_closure(output);
        let leaf_limits = DecodeLimits {
            semantic_leaf_bytes: 0,
            ..DecodeLimits::default()
        };
        assert!(
            Table::from_export_hir(
                &output.output().export,
                &required,
                &mut BudgetMeter::new(leaf_limits)
            )
            .is_err()
        );
        assert!(decode_canonical::<Decoded>(&encode(&table).unwrap(), leaf_limits).is_err());
        // Resolution moves strings already admitted by the decoder; it does not
        // allocate another semantic leaf or reapply the per-leaf decoding cap.
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                Table::from_export_hir(
                    &output.output().export,
                    &required,
                    &mut BudgetMeter::new(limits)
                )
                .is_err(),
                "{limits:?}"
            );
            assert!(
                decoded
                    .clone()
                    .resolve(&mut identities, &mut BudgetMeter::new(limits))
                    .is_err(),
                "{limits:?}"
            );
        }
    });
}
