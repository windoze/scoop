use super::*;
use scoop_mir::MirTypeRepresentationV1 as Repr;

pub(super) fn source_shapes(
    output: &hir::DependencyHirOutput,
    table: &CanonicalParamFreeMirTypeExportsV1,
) {
    let mut seen = BTreeSet::new();
    for exact in source_dispatch::owners(output).values() {
        let record = table.get(*exact).unwrap();
        assert!(matches!(record.origin(), MirTypeOriginV1::SourceNominal(_)));
        seen.insert(*exact);
        if let Repr::Object { backing } = record.representation() {
            let backing = table.get(*backing).unwrap();
            seen.insert(backing.exact());
            assert!(matches!(
                backing.representation(),
                Repr::ObjectBacking { .. }
            ));
            assert_eq!(backing.base_and_interfaces(), record.base_and_interfaces());
        } else {
            assert!(matches!(
                record.representation(),
                Repr::Struct { .. } | Repr::Enum { .. } | Repr::Class { .. } | Repr::Interface
            ));
        }
    }
    assert_eq!(seen.len(), table.records().len());
}

pub(super) fn rejections(
    local: &hir::LocalConcreteHir,
    strong: &scoop_mir::ConeMirInput,
    source: &hir::CrossConeTypeSemanticsSectionV1,
    graph: &scoop_identity::ValidatedIdentityGraph,
    table: &CanonicalParamFreeMirTypeExportsV1,
) {
    let produce = |graph| scoop_mir_lower::lower_source_type_exports(source, strong, graph);

    assert_eq!(produce(graph).unwrap(), *table);

    produce(graph).unwrap();

    let empty = PendingIdentityValidation::new().finish().unwrap();
    assert!(matches!(
        produce(&empty),
        Err(scoop_mir_lower::SourceMirTypeProductionError::Bridge(_))
    ));

    let complete = scoop_mir_lower::lower_type_exports(local, source, strong, graph).unwrap();
    assert!(complete.records().len() > table.records().len());

    assert_eq!(
        scoop_mir_lower::lower_type_exports(local, source, strong, graph).unwrap(),
        complete
    );
}
