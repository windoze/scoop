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
    strong: &scoop_mir::SingleConeStrongMirInput,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    graph: &scoop_identity::ValidatedIdentityGraph,
    table: &CanonicalParamFreeMirTypeExportsV1,
) {
    let produce = |graph, meter: &mut BudgetMeter| {
        scoop_mir_lower::lower_source_type_exports(source, strong, graph, meter)
    };
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    assert_eq!(produce(graph, &mut measured).unwrap(), *table);
    let usage = measured.usage();
    assert!(usage.validation_work_units > 0);
    assert!(usage.owned_bytes > 0);
    let limits = DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    };
    let mut shared = BudgetMeter::new(limits);
    produce(graph, &mut shared).unwrap();
    assert!(produce(graph, &mut shared).is_err());
    let limits = DecodeLimits {
        owned_bytes: 0,
        ..DecodeLimits::default()
    };
    assert!(produce(graph, &mut BudgetMeter::new(limits)).is_err());
    let empty = PendingIdentityValidation::new().finish().unwrap();
    assert!(matches!(
        produce(&empty, &mut BudgetMeter::new(DecodeLimits::default())),
        Err(scoop_mir_lower::SourceMirTypeProductionError::Bridge(_))
    ));
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    let complete =
        scoop_mir_lower::lower_type_exports(source, strong, graph, &mut measured).unwrap();
    assert!(complete.records().len() > table.records().len());
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    assert_eq!(
        scoop_mir_lower::lower_type_exports(source, strong, graph, &mut shared).unwrap(),
        complete
    );
    assert!(scoop_mir_lower::lower_type_exports(source, strong, graph, &mut shared).is_err());
}
