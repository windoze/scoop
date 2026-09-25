use super::*;
use mir::{MirInitializationUnitProofKindV1, MirTypeBridgeLocalAuthorityV1};

pub(super) fn check<'a>(
    name: &str,
    fixtures: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'a>,
    exports: mir::MirTypeBridgeExportConstituentsV1,
) -> mir::CrossConeMirTypeBridgeSectionV1<'a> {
    let source = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
        input,
        scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
            types: &[],
            callables: &[],
            dispatch: &[],
        },
        &mut meter(),
    )
    .unwrap();
    let incomplete = mir::MirTypeBridgeExportConstituentsV1::new(
        mir::CanonicalParamFreeMirTypeExportsV1::try_new(vec![]).unwrap(),
        exports.callables().clone(),
        exports.dispatch().clone(),
        exports.objects().clone(),
        exports.shapes().clone(),
        exports.initialization_uses().clone(),
    );
    assert!(matches!(
        incomplete.validate_sources(
            input.mir.module().cone,
            input.identities,
            &source,
            &mut meter()
        ),
        Err(mir::MirTypeBridgeSourceJoinError::Inventory(
            mir::MirTypeBridgeSourceInventoryV1::Types
        ))
    ));
    let section = mir::CrossConeMirTypeBridgeSectionV1::try_new(
        MirTypeBridgeLocalAuthorityV1::Producer {
            provider: input.mir.module().cone,
            input: input.mir,
            ordinary: input.ordinary,
        },
        exports,
        &[],
        &source,
        input.identities,
        &mut meter(),
    )
    .unwrap_or_else(|error| panic!("{name} complete MIR section: {error}"));
    let bytes = encode(&section).unwrap();
    let wire: mir::DecodedCrossConeMirTypeBridgeSectionV1 = decoded(&section);
    assert_eq!(encode(&wire).unwrap(), bytes);
    let mut graph = identity_graph(input.hir, input.mir, None);
    let replayed = wire
        .validate(
            MirTypeBridgeLocalAuthorityV1::Reader {
                provider: input.mir.module().cone,
                foundation: input.mir.foundation(),
                production: input.mir.production(),
                ordinary: input.ordinary,
            },
            &[],
            &source,
            &mut graph,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
    assert_eq!(
        section.initialization_units().len(),
        input.hir.output().local.module().initialization_units.len()
    );
    assert!(section.selected().is_empty());
    assert!(replayed.initialization_units().iter().all(|unit|
        unit.proof_kind() == MirInitializationUnitProofKindV1::ReaderSemanticReplay
    ));
    let mut units = Vec::new();
    for unit in section.initialization_units() {
        let MirInitializationUnitProofKindV1::ProducerEmitted(root) = unit.proof_kind() else {
            panic!("the producer retains the actual initialization roots")
        };
        let mir = input.mir.module();
        assert_eq!(unit.unit(), root.identity());
        assert_eq!(
            unit.initializer().callable_owner(),
            root.initializer().implementation()
        );
        assert_eq!(
            unit.ensure().callable_owner(),
            root.ensure().implementation()
        );
        units.push(format!(
            "unit {}: {:?} {:?}\n",
            mir.initialization_units[root.unit()].display_name,
            unit.signature().exact().effect(),
            unit.signature().gc_effect(),
        ));
    }
    units.sort();
    let mut dump = format!(
        "types={} callables={} dispatch={} objects={} shapes={} units={} selected={}\n",
        section.types().records().len(),
        section.callables().entries().len(),
        section.dispatch().records().len(),
        section.object_values().records().len(),
        section.shape_support().records().len(),
        section.initialization_units().len(),
        section.selected().len(),
    );
    dump.extend(units);
    let snapshot = fixtures.join(format!("{name}.mir-section.snap"));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
    section
}
