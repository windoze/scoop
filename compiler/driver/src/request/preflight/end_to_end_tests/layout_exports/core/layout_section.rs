use super::*;
use lir::LayoutAbiSectionSourceAuthorityV1;
use scoop_lir_lower::{LayoutAbiSourceInventoryV1, LayoutAbiSourceProjectionError as Error};

pub(super) fn check(
    mir_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    exports: lir::LayoutAbiExportConstituentsV1,
) -> lir::CrossConeLayoutAbiSectionV1<'static> {
    let source = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
        input,
        scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
        &mut meter(),
    )
    .unwrap();
    let missing = lir::LayoutAbiExportConstituentsV1::try_new(
        lir::CanonicalExactLayoutExportsV1::try_new(
            input.lir.module().meta.target_profile,
            input.lir.foundation(),
            vec![],
            &mut meter(),
        )
        .unwrap(),
        exports.descriptors().clone(),
        exports.dispatch().clone(),
        exports.callables().clone(),
        exports.shape_support().clone(),
    )
    .unwrap();
    assert!(matches!(
        source.validate_local_exports(&missing, &mut meter()),
        Err(Error::Inventory(LayoutAbiSourceInventoryV1::Layouts))
    ));
    assert!(matches!(
        source.validate_local_exports(
            &exports,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(Error::Resource(_))
    ));
    let section = lir::CrossConeLayoutAbiSectionV1::try_new(
        exports.clone(),
        &[],
        vec![],
        &source,
        &mut meter(),
    )
    .unwrap();
    let bytes = encode(&section).unwrap();
    let wire: lir::DecodedCrossConeLayoutAbiSectionV1 = decoded(&section);
    assert_eq!(encode(&wire).unwrap(), bytes);
    let mut identities = identity_graph(mir_input.hir, mir_input.mir, Some(input.lir));
    let replayed = wire
        .validate(
            &exports,
            &[],
            vec![],
            &source,
            &mut identities,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
    let missing_callables = lir::LayoutAbiExportConstituentsV1::try_new(
        exports.layouts().clone(),
        exports.descriptors().clone(),
        exports.dispatch().clone(),
        lir::CanonicalExactCallableAbiExportsV1::try_new(
            input.lir.module().meta.target_profile,
            input.lir.foundation(),
            vec![],
            &mut meter(),
        )
        .unwrap(),
        exports.shape_support().clone(),
    )
    .unwrap();
    let missing_dispatch = lir::LayoutAbiExportConstituentsV1::try_new(
        exports.layouts().clone(),
        exports.descriptors().clone(),
        lir::CanonicalExactDispatchExportsV1::try_new(
            input.lir.module().meta.target_profile,
            input.lir.foundation(),
            vec![],
            &mut meter(),
        )
        .unwrap(),
        exports.callables().clone(),
        exports.shape_support().clone(),
    )
    .unwrap();
    for (index, altered) in [&missing, &missing_callables, &missing_dispatch]
        .into_iter()
        .enumerate()
    {
        let wire: lir::DecodedCrossConeLayoutAbiSectionV1 = decoded(&section);
        let checked = wire
            .validate_layouts(exports.layouts(), &mut meter())
            .unwrap()
            .validate_callables(exports.callables(), &mut meter())
            .unwrap()
            .validate_dispatch(exports.dispatch(), &mut meter())
            .unwrap();
        let error = checked
            .validate(altered, &[], vec![], &source, &mut identities, &mut meter())
            .err()
            .expect("a checked constituent cannot be replaced before final validation");
        match index {
            0 => assert!(matches!(
                error,
                lir::LayoutAbiSectionError::LayoutReplayChanged
            )),
            1 => assert!(matches!(
                error,
                lir::LayoutAbiSectionError::CallableReplayChanged
            )),
            2 => assert!(matches!(
                error,
                lir::LayoutAbiSectionError::DispatchReplayChanged
            )),
            _ => unreachable!(),
        }
    }
    assert!(section.selected().is_empty());
    assert!(section.selected().physical_imports().records().is_empty());
    section
}

pub(super) fn snapshot(
    name: &str,
    fixtures: &Path,
    section: &lir::CrossConeLayoutAbiSectionV1<'_>,
    objects: &scoop_codegen::EmittedStrongObjectSetV2,
) {
    assert!(!objects.members().is_empty());
    let production = objects.production();
    let dump = format!(
        "layouts={} descriptors={} dispatch={} callables={} shapes={} selected={} physical={}\nregistrations: types={} callables={} initialization={}\nobject members={}\n",
        section.layouts().records().len(),
        section.descriptors().records().len(),
        section.dispatch().records().len(),
        section.callables().records().len(),
        section.shape_support().records().len(),
        section.selected().len(),
        section.selected().physical_imports().records().len(),
        production.type_registrations().registrations().len(),
        production.callable_registrations().registrations().len(),
        production
            .initialization_registrations()
            .registrations()
            .len(),
        objects.members().len(),
    );
    let snapshot = fixtures.join(format!("{name}.lir-section.snap"));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
}
