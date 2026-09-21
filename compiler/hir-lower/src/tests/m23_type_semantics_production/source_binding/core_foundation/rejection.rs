use super::*;

#[test]
fn core_source_foundation_does_not_grant_odr_execution() {
    let output = support::lower_extra(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/core-source-odr.scoop"
    )));
    Production::from_core_bootstrap(&output, &mut meter()).unwrap();
    let canonical = hir::CanonicalHirFoundation::from_modules(
        &output.export,
        &output.local,
        &output.native_boundary_types,
    )
    .unwrap();
    assert!(matches!(
        hir::OdrFreeHirFoundation::try_new(canonical),
        Err(hir::OdrFreeHirFoundationError::CallableApplication(_))
    ));
}

#[test]
fn core_source_foundation_rejects_a_missing_core_shape_plan() {
    let mut output = lower_minimal();
    output.local = hir::LocalConcreteHirOutput::try_new(
        output.local.module().clone(),
        output.local.output_kind().clone(),
        hir::LocalConcreteMaterializationContract::Ordinary,
    )
    .unwrap();
    assert!(matches!(
        Production::from_core_bootstrap(&output, &mut meter()),
        Err(hir::CrossConeTypeSemanticsProductionError::InvalidCoreSourcePair(_))
    ));
}

#[test]
fn core_source_foundation_respects_shared_source_projection_resources() {
    let output = lower_minimal();
    let mut measured = meter();
    Production::from_core_bootstrap(&output, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units * 2 - 1,
        ..DecodeLimits::default()
    });
    Production::from_core_bootstrap(&output, &mut shared).unwrap();
    assert_resource(Production::from_core_bootstrap(&output, &mut shared));
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
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
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_leaf_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert_resource(Production::from_core_bootstrap(
            &output,
            &mut BudgetMeter::new(limits),
        ));
    }
}
fn assert_resource(result: Result<Production, hir::CrossConeTypeSemanticsProductionError>) {
    assert!(
        matches!(
            result,
            Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                hir::SourceInventoryError::Resource(_)
            ))
        ),
        "{result:?}"
    );
}
