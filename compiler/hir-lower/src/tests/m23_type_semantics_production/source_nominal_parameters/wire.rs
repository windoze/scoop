use super::*;
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
fn nominal_parameter_source_bytes_restore_and_are_deterministic() {
    for source in [SOURCE, CALLABLES] {
        let produce = || {
            with_source(source, |output, _| {
                let table = table(&output.output().export);
                let bytes = encode(&table).unwrap();
                let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                assert_eq!(encode(&decoded).unwrap(), bytes);
                let restored = decoded
                    .resolve(
                        &mut source_inventory::identity_closure(output),
                        &mut meter(),
                    )
                    .unwrap();
                assert_eq!(restored, table);
                bytes
            })
        };
        assert_eq!(produce(), produce());
    }
}

#[test]
fn nominal_parameter_reader_rejects_duplicate_reordered_unknown_and_extra_fields() {
    with_source(SOURCE, |output, _| {
        let table = table(&output.output().export);
        let duplicate = vec![table.records()[0].clone(); 2];
        assert!(Table::try_new(duplicate.clone(), &mut meter()).is_err());
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
                Err(hir::SourceInventoryError::NonCanonicalOrder { .. })
            ));
        }
        let decoded: Decoded =
            decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(matches!(
            decoded.resolve(&mut empty, &mut meter()),
            Err(hir::SourceInventoryError::Reference(_))
        ));
        let mut extra = encode(&Records(&table.records()[..1])).unwrap();
        assert_eq!(extra[1], 0xa2);
        extra[1] = 0xa3;
        extra.extend([3, 0]);
        assert!(decode_canonical::<Decoded>(&extra, DecodeLimits::default()).is_err());
    });
}

#[test]
fn complete_variant_parameter_records_do_not_expand_legacy_wire_roles() {
    with_source(SOURCE, |output, _| {
        let table = table(&output.output().export);
        let variants = table
            .records()
            .iter()
            .filter(|r| matches!(r.owner(), CallableTemplateOrigin::VariantConstructor(_)))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(variants.len(), 4);
        for variant in &variants {
            assert!(hir::InheritanceSourceParameterProtocolV1::try_from(variant.clone()).is_err());
        }
        let decoded: hir::DecodedCanonicalInheritanceSourceParameterProtocolsV1 = decode_canonical(
            &encode(&Records(&variants)).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            decoded.resolve(
                &mut source_inventory::identity_closure(output),
                &mut meter()
            ),
            Err(hir::SourceInventoryError::Reference(_))
        ));
    });
}

#[test]
fn nominal_parameter_producer_and_reader_enforce_shared_resource_limits() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let required = required(export);
        let table = table(export);
        let decoded: Decoded =
            decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
        let mut identities = source_inventory::identity_closure(output);
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
            DecodeLimits {
                semantic_recursion: 4,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                Table::from_export_hir(export, &required, &mut BudgetMeter::new(limits)).is_err(),
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
