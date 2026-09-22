use super::*;
use scoop_mir::{MirObjectValueProductionV1 as Production, MirTypeBridgeTypeIndexV1};

mod assertions;
mod rejections;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

// Unit is a language builtin. This dependency fixture retains the actual
// imported identity present in MIR; it does not mint a local source type.
fn unit_dependency(
    input: &scoop_mir::SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
) -> CanonicalParamFreeMirTypeExportsV1 {
    use scoop_mir::*;
    let Some(unit) = input.module().meta.source_exact_types.get(&Type::Unit) else {
        return CanonicalParamFreeMirTypeExportsV1::default();
    };
    let scoop_identity::ExactTypeKey::Nominal(nominal) = *unit.identity_record().key() else {
        panic!("Unit is nominal")
    };
    CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        ParamFreeMirTypeExportV1::try_new(
            MirTypeBridgeAuthority {
                identities: graph,
                foundation: input.foundation(),
            },
            unit.identity_record().id(),
            MirTypeOriginV1::SourceNominal(nominal),
            MirTypeFactsV1::try_new(MirValueKindV1::ZeroSizedValue, MirGcKindV1::GcFree).unwrap(),
            MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit),
            MirBaseAndInterfacesV1 {
                base: MirBaseClassV1::None,
                interfaces: vec![],
            },
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn actual_object_values_and_initialization_callables_share_source_identities() {
    for name in ["standalone", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-mir-object-production");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, projection) = with_production(&source, |output, input, hir, graph, _| {
            let types =
                scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
            let unit = unit_dependency(input, graph);
            assert!(
                unit.records()
                    .iter()
                    .all(|record| types.get(record.exact()).is_none())
            );
            let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit], &mut meter()).unwrap();
            let product =
                Production::from_strong_input(input, &types, graph, &index, &mut meter()).unwrap();
            assertions::actual(output, input, &product);
            assertions::wire(input, graph, &index, &product);
            if name == "combined" {
                assertions::private_and_property(input, &product);
            }
            (
                (
                    encode(product.callables()).unwrap(),
                    encode(product.objects()).unwrap(),
                ),
                assertions::projection(output, &product),
            )
        });
        with_production(
            &format!("private object Unrelated {{}}\n{source}"),
            |_, input, hir, graph, _| {
                let types =
                    scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
                let unit = unit_dependency(input, graph);
                let index =
                    MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit], &mut meter()).unwrap();
                let product =
                    Production::from_strong_input(input, &types, graph, &index, &mut meter())
                        .unwrap();
                assert_eq!(
                    (
                        encode(product.callables()).unwrap(),
                        encode(product.objects()).unwrap()
                    ),
                    bytes
                );
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_OBJECT_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                std::path::Path::new(&path).join(format!("{name}.snap")),
                projection,
            )
            .unwrap();
        } else {
            assert_eq!(
                projection,
                std::fs::read_to_string(directory.join(format!("{name}.snap"))).unwrap()
            );
        }
    }
}

#[test]
fn actual_object_production_rejects_missing_dependencies_and_exhausted_budgets() {
    with_production("public object Registry {}", |_, input, hir, graph, _| {
        let types = scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
        let unit = unit_dependency(input, graph);
        rejections::check(input, graph, &types, &unit);
    });
    with_production("public val number: Int = 3", |_, input, hir, graph, _| {
        let types = scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
        let unit = unit_dependency(input, graph);
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit], &mut meter()).unwrap();
        let product =
            Production::from_strong_input(input, &types, graph, &index, &mut meter()).unwrap();
        assert!(product.objects().records().is_empty());
        assert!(product.callables().entries().is_empty());
    });
}
