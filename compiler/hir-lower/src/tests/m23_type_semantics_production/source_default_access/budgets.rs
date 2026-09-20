use super::*;

#[test]
fn source_access_projection_and_resolution_share_remaining_resources() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let body = hir::DefaultSourceBodyProductionV1::from_ordinary_hir(
            output,
            function(export, "fileDefault"),
            0,
            &mut meter(),
        )
        .unwrap();
        let source = witnesses(body.source_references()).next().unwrap();
        let mut measured = meter();
        let snapshot = Witness::from_export_hir(export, source, &mut measured).unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: 2 * work - 1,
            ..DecodeLimits::default()
        });
        Witness::from_export_hir(export, source, &mut shared).unwrap();
        assert!(matches!(
            Witness::from_export_hir(export, source, &mut shared),
            Err(hir::DefaultSourceAccessProductionError::Resource(_))
        ));
        let bytes = encode(&snapshot).unwrap();
        let mut identities = identity_closure(output);
        for limits in [
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
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
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                Witness::from_export_hir(export, source, &mut BudgetMeter::new(limits)),
                Err(hir::DefaultSourceAccessProductionError::Resource(_))
            ));
            let decoded: DecodedWitness =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let error = decoded
                .resolve(&mut identities, &mut BudgetMeter::new(limits))
                .unwrap_err();
            assert!(error.resource_error().is_some(), "{error:?}");
        }
        let decoded: DecodedWitness = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let mut measured = meter();
        decoded
            .clone()
            .resolve(&mut identities, &mut measured)
            .unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        decoded
            .clone()
            .resolve(&mut identities, &mut shared)
            .unwrap();
        assert!(
            decoded
                .resolve(&mut identities, &mut shared)
                .unwrap_err()
                .resource_error()
                .is_some()
        );
    });
}
