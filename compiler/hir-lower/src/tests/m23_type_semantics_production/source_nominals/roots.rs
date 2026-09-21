use super::*;

const ROOTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/roots.scoop"
));

fn names(
    output: &hir::DependencyHirOutput,
    roots: &hir::CanonicalSourceNominalIdsV1,
) -> Vec<String> {
    let identities = sources(output.output().export.module());
    let mut names = roots
        .values()
        .iter()
        .map(|owner| name(identities[owner]).to_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn nominal_source_roots_close_protected_support_without_unrelated_private_types() {
    with_source(ROOTS, |output, _| {
        let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(
            &output.output().export,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(
            names(output, &roots),
            [
                "Cache",
                "ConcreteLeaf",
                "Derived",
                "Entry",
                "GenericChild",
                "Helper",
                "HiddenChoice",
                "Host",
                "InheritedNested",
                "Last",
                "LeftRole",
                "Node",
                "PublicChild",
                "PublicChildSupport",
                "PublicSibling",
                "PublicSiblingEntry",
                "RightRole",
                "RootRole",
                "SupportBase",
                "Value",
            ]
        );
        let table = Table::from_export_hir(&output.output().export, &roots, &mut meter()).unwrap();
        assert_eq!(table.records().len(), roots.values().len());
        let identities = sources(output.output().export.module());
        for record in table.records() {
            if name(identities[&record.owner()]) == "GenericChild" {
                assert!(matches!(
                    record.owner(),
                    hir::SourceNominalId::GenericTemplate(_)
                ));
            }
        }
        let bytes = encode(&roots).unwrap();
        let decoded: hir::DecodedCanonicalSourceNominalIdsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let mut identities = super::super::source_inventory::identity_closure(output);
        let restored = decoded.resolve(&mut identities, &mut meter()).unwrap();
        assert_eq!(restored, roots);
        assert_eq!(encode(&restored).unwrap(), bytes);
    });
}

#[test]
fn nominal_source_roots_keep_generic_and_static_nested_support_distinct() {
    with_source(NESTED, |output, _| {
        let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(
            &output.output().export,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(
            names(output, &roots),
            [
                "Base",
                "Box",
                "Cache",
                "Choice",
                "ConcreteChild",
                "Derived",
                "Envelope",
                "Gap",
                "GenericBase",
                "GenericChild",
                "GenericOuter",
                "HiddenChild",
                "Leaf",
                "Marker",
                "Pair",
                "View",
            ]
        );
        let table = Table::from_export_hir(&output.output().export, &roots, &mut meter()).unwrap();
        let identities = sources(output.output().export.module());
        let concrete = table
            .records()
            .iter()
            .find(|record| name(identities[&record.owner()]) == "ConcreteChild")
            .unwrap();
        assert!(matches!(
            concrete.owner(),
            hir::SourceNominalId::Concrete(_)
        ));
        assert!(concrete.type_parameters().binders().is_empty());
        assert_eq!(table.records().len(), 16);
    });
}

#[test]
fn nominal_source_roots_can_be_empty_despite_private_protected_declarations() {
    with_source(
        "private open class Hidden { protected class Nested {} }",
        |output, _| {
            let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(
                &output.output().export,
                &mut meter(),
            )
            .unwrap();
            assert!(roots.values().is_empty());
        },
    );
}

#[test]
fn nominal_source_root_discovery_obeys_shared_resource_limits() {
    with_source(ROOTS, |output, _| {
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
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
        ] {
            let result = hir::CanonicalSourceNominalIdsV1::from_export_hir(
                &output.output().export,
                &mut BudgetMeter::new(limits),
            );
            assert!(
                matches!(
                    result,
                    Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                        hir::SourceInventoryError::Resource(_)
                    ))
                ),
                "{limits:?}: {result:?}"
            );
        }
    });
}

#[test]
fn source_root_budget_includes_object_scans_for_each_class_base() {
    let mut source = String::from("public open class Base {}\n");
    const COUNT: u64 = 40;
    for index in 0..COUNT {
        source.push_str(&format!(
            "public class Derived{index} : Base() {{}}\nprivate object Hidden{index} {{}}\n"
        ));
    }
    let usage = |source: &str| {
        with_source(source, |output, _| {
            let mut budget = meter();
            let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(
                &output.output().export,
                &mut budget,
            )
            .unwrap();
            assert_eq!(roots.values().len() as u64, COUNT + 1);
            budget.usage().validation_work_units
        })
    };
    let with_bases = usage(&source);
    let without_bases = usage(&source.replace(" : Base()", ""));
    assert!(with_bases - without_bases >= COUNT * COUNT);
}
