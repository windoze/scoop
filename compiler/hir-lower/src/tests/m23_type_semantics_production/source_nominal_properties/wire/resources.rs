use super::*;

#[test]
fn nominal_property_source_production_and_reading_share_resource_limits() {
    with_source(SOURCE, |output, _| {
        let required = required(output);
        let source = table(output);
        let bytes = encode(&source).unwrap();
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
        ] {
            assert!(matches!(
                Table::from_export_hir(
                    &output.output().export,
                    &required,
                    &mut BudgetMeter::new(limits)
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
            let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert!(matches!(
                decoded.resolve(&mut identities, &mut BudgetMeter::new(limits)),
                Err(Error::Inventory(hir::SourceInventoryError::Resource(_)))
            ));
        }
    });
}

#[test]
fn nominal_const_strings_are_metered_before_copying_the_evaluated_value() {
    let source = format!(
        "public object Tokens {{ private const val payload: String = {:?} }}",
        "x".repeat(4096)
    );
    with_source(&source, |output, _| {
        let required = required(output);
        for limits in [
            DecodeLimits {
                semantic_leaf_bytes: 64,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: 128,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                Table::from_export_hir(
                    &output.output().export,
                    &required,
                    &mut BudgetMeter::new(limits)
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
        }
    });
}
