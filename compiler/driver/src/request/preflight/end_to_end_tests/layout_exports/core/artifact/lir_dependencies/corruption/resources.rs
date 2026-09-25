use super::*;

pub(super) fn check(
    metadata: hir::SharedTypeMetadataV1<'_>,
    layout: &lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    core: &lir::LayoutAbiExportConstituentsV1,
) {
    let replay = |meter: &mut BudgetMeter| {
        scoop_slib::replay_shared_lir_dependency_graph(metadata, layout, &[core], meter)
    };
    let mut measured = meter();
    replay(&mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    let mut inclusive = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    replay(&mut inclusive).unwrap();
    assert!(replay(&mut inclusive).is_err());
    for limits in [
        DecodeLimits {
            validation_work_units: required - 1,
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
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_edges: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(replay(&mut BudgetMeter::new(limits)).is_err());
    }
}
