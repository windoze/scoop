use super::*;
use scoop_mir::MirTypeRepresentationV1 as Repr;
use std::fmt::Write;

pub(super) fn dump(
    output: &hir::DependencyHirOutput,
    table: &CanonicalParamFreeMirTypeExportsV1,
) -> String {
    let names = source_dispatch::owners(output);
    let mut text = String::new();
    let mut seen = BTreeSet::new();
    for (name, exact) in &names {
        let record = table.get(*exact).unwrap();
        assert!(matches!(record.origin(), MirTypeOriginV1::SourceNominal(_)));
        seen.insert(*exact);
        let bases = record.base_and_interfaces();
        writeln!(
            text,
            "{name}: {:?}/{:?} base={} interfaces={}",
            record.facts().kind(),
            record.facts().gc(),
            matches!(bases.base, scoop_mir::MirBaseClassV1::Base(_)),
            bases.interfaces.len()
        )
        .unwrap();
        match record.representation() {
            Repr::Struct {
                fields,
                c_layout,
                interior_mutable,
            } => writeln!(
                text,
                "  struct fields={} {c_layout:?} interior_mutable={interior_mutable}",
                fields.len()
            )
            .unwrap(),
            Repr::Enum { variants } => {
                for (index, variant) in variants.iter().enumerate() {
                    writeln!(
                        text,
                        "  variant {index} fields={} {:?}",
                        variant.fields.len(),
                        variant.gc
                    )
                    .unwrap();
                }
            }
            Repr::Class {
                kind,
                declared_fields,
            } => writeln!(
                text,
                "  class {kind:?} own_fields={}",
                declared_fields.len()
            )
            .unwrap(),
            Repr::Interface => writeln!(text, "  interface").unwrap(),
            Repr::Object { backing } => {
                let backing = table.get(*backing).unwrap();
                seen.insert(backing.exact());
                let Repr::ObjectBacking { declared_fields } = backing.representation() else {
                    panic!("object backing")
                };
                assert_eq!(backing.base_and_interfaces(), record.base_and_interfaces());
                writeln!(text, "  object backing_fields={}", declared_fields.len()).unwrap();
            }
            other => panic!("unexpected source shape {other:?}"),
        }
    }
    assert_eq!(seen.len(), table.records().len());
    text
}

pub(super) fn rejections(
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

    let complete = scoop_mir_lower::lower_type_exports(source, strong, graph).unwrap();
    assert!(complete.records().len() > table.records().len());

    assert_eq!(
        scoop_mir_lower::lower_type_exports(source, strong, graph).unwrap(),
        complete
    );
}
