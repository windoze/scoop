use super::*;
use scoop_mir::{MirObjectValueProductionV1 as Production, MirTypeBridgeTypeIndexV1};

mod assertions;
mod rejections;

#[test]
fn actual_object_values_and_initialization_callables_share_source_identities() {
    for name in ["standalone", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-mir-object-production");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, projection) = with_production(&source, |output, input, hir, graph, _| {
            let types = scoop_mir_lower::lower_type_exports(hir, input, graph).unwrap();
            let unit = dependencies::unit(input, graph);
            assert!(
                unit.records()
                    .iter()
                    .all(|record| types.get(record.exact()).is_none())
            );
            let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit]).unwrap();
            let product = Production::from_strong_input(input, &types, graph, &index).unwrap();
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
                let types = scoop_mir_lower::lower_type_exports(hir, input, graph).unwrap();
                let unit = dependencies::unit(input, graph);
                let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit]).unwrap();
                let product = Production::from_strong_input(input, &types, graph, &index).unwrap();
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
fn actual_object_production_rejects_missing_dependencies() {
    with_production("public object Registry {}", |_, input, hir, graph, _| {
        let types = scoop_mir_lower::lower_type_exports(hir, input, graph).unwrap();
        let unit = dependencies::unit(input, graph);
        rejections::check(input, graph, &types, &unit);
    });
    with_production("public val number: Int = 3", |_, input, hir, graph, _| {
        let types = scoop_mir_lower::lower_type_exports(hir, input, graph).unwrap();
        let unit = dependencies::unit(input, graph);
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit]).unwrap();
        let product = Production::from_strong_input(input, &types, graph, &index).unwrap();
        assert!(product.objects().records().is_empty());
        assert!(product.callables().entries().is_empty());
    });
}
