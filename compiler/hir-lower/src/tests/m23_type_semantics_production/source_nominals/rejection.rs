use super::*;

#[test]
fn nominal_source_contracts_reject_foreign_required_owner() {
    with_source(DECLARATIONS, |output, _| {
        let hir::CoreProtocols::Imported(core) = &output.output().export.core_protocols else {
            panic!("imported core")
        };
        let foreign = hir::SourceNominalId::Concrete(core.fundamental_types().unit().persistent());
        let required =
            hir::CanonicalSourceNominalIdsV1::try_new(vec![foreign], &mut meter()).unwrap();
        assert!(matches!(
            Table::from_export_hir(&output.output().export, &required, &mut meter()),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
        ));
    });
}

#[test]
fn nominal_source_contract_projection_and_replay_share_budgets() {
    with_source(NESTED, |output, _| {
        let required = hir::CanonicalSourceNominalIdsV1::try_new(
            sources(output.output().export.module())
                .into_keys()
                .collect(),
            &mut meter(),
        )
        .unwrap();
        let table = table(output);
        let mut identities = source_inventory::identity_closure(output);
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
        ] {
            let result = Table::from_export_hir(
                &output.output().export,
                &required,
                &mut BudgetMeter::new(limits),
            );
            assert!(
                matches!(
                    result,
                    Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                        hir::SourceInventoryError::Resource(_)
                    ))
                ),
                "{result:?}"
            );
            let decoded: Decoded =
                decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
            assert!(matches!(
                decoded.resolve(&mut identities, &mut BudgetMeter::new(limits)),
                Err(hir::SourceInventoryError::Resource(_))
            ));
        }
    });
}
