use super::*;
use mir::MirTypeBridgeLocalInputV1;

pub(super) fn check<'a>(
    name: &str,
    fixtures: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'a>,
    exports: mir::MirTypeBridgeExportConstituentsV1,
) -> mir::CrossConeMirTypeBridgeSectionV1<'a> {
    let source = scoop_mir_lower::lower_type_bridge_dependencies(input).unwrap();
    let section = mir::CrossConeMirTypeBridgeSectionV1::try_new(
        MirTypeBridgeLocalInputV1 {
            provider: input.mir.module().cone,

            production: (input.mir).production(),
            ordinary: input.ordinary,
        },
        exports,
        scoop_mir_lower::lower_type_bridge_initialization_units(input.mir).unwrap(),
        &[],
        &source,
        input.identities,
    )
    .unwrap_or_else(|error| panic!("{name} complete MIR section: {error}"));
    if name == "base" {
        reject_missing_application(input, &section, &source);
    }
    let bytes = encode(&section).unwrap();
    let wire: mir::DecodedCrossConeMirTypeBridgeSectionV1 = decoded(&section);
    assert_eq!(encode(&wire).unwrap(), bytes);
    let mut graph = identity_graph(input.hir, input.mir, None);
    let local = MirTypeBridgeLocalInputV1 {
        provider: input.mir.module().cone,

        production: input.mir.production(),
        ordinary: input.ordinary,
    };
    let replayed = wire
        .resolve_types(
            local.provider,
            input.mir.foundation(),
            std::iter::empty(),
            &mut graph,
        )
        .unwrap()
        .resolve_callables(
            input.mir.foundation(),
            &[input.ordinary],
            std::iter::empty(),
            &mut graph,
        )
        .unwrap()
        .resolve_dependencies(local, &mut graph)
        .unwrap();
    replayed
        .replay_dependency_closure(section.initialization_units(), &[], &source, &graph)
        .unwrap();
    assert_eq!(
        section.initialization_units().len(),
        input.mir.materialization().initialization_roots().len()
    );
    assert!(section.selected().is_empty());
    let mut units = Vec::new();
    for unit in section.initialization_units() {
        let root = input
            .mir
            .materialization()
            .initialization_roots()
            .iter()
            .find(|root| root.identity() == unit.unit())
            .unwrap();
        let mir = input.mir.module();
        assert_eq!(unit.unit(), root.identity());
        assert_eq!(
            mir::CallableSignatureSubject::Strong(unit.initializer().callable_owner()),
            root.initializer().subject()
        );
        assert_eq!(
            mir::CallableSignatureSubject::Strong(unit.ensure().callable_owner()),
            root.ensure().subject()
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

fn reject_missing_application(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    source: &[mir::MirTypeBridgeDependencyV1],
) {
    let missing = section
        .types()
        .records()
        .iter()
        .find(|record| matches!(record.origin(), mir::MirTypeOriginV1::NominalApplication(_)))
        .unwrap()
        .exact();
    let types = mir::CanonicalParamFreeMirTypeExportsV1::try_new(
        section
            .types()
            .records()
            .iter()
            .filter(|record| record.exact() != missing)
            .cloned()
            .collect(),
    )
    .unwrap();
    let exports = mir::MirTypeBridgeExportConstituentsV1::new(
        types,
        section.callables().clone(),
        section.dispatch().clone(),
        section.object_values().clone(),
        section.shape_support().clone(),
        section.initialization_uses().clone(),
    );
    let error = mir::CrossConeMirTypeBridgeSectionV1::try_new(
        MirTypeBridgeLocalInputV1 {
            provider: input.mir.module().cone,
            production: input.mir.production(),
            ordinary: input.ordinary,
        },
        exports,
        section.initialization_units().to_vec(),
        &[],
        source,
        input.identities,
    )
    .err()
    .expect("actual application references require their MIR type record");
    assert!(
        matches!(error,
        mir::MirTypeBridgeSectionError::MissingDependency(mir::MirTypeBridgeTargetV1::Type(exact))
        if exact == missing),
        "{error:?}"
    );
}
