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
        let roots =
            hir::CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export).unwrap();
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
        let table = Table::from_export_hir(&output.output().export, &roots).unwrap();
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
        let decoded: hir::DecodedCanonicalSourceNominalIdsV1 = decode_canonical(&bytes).unwrap();
        let mut identities = super::super::source_inventory::identity_closure(output);
        let restored = decoded.resolve(&mut identities).unwrap();
        assert_eq!(restored, roots);
        assert_eq!(encode(&restored).unwrap(), bytes);
    });
}

#[test]
fn nominal_source_roots_keep_generic_and_static_nested_support_distinct() {
    with_source(NESTED, |output, _| {
        let roots =
            hir::CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export).unwrap();
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
        let table = Table::from_export_hir(&output.output().export, &roots).unwrap();
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
            let roots =
                hir::CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export).unwrap();
            assert!(roots.values().is_empty());
        },
    );
}

#[test]
fn nominal_source_roots_close_private_storage_dependencies_without_private_siblings() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-lir-export-assembly/private-support.scoop"
    ));
    with_source(
        &format!("private struct Unrelated() {{}}\n{source}"),
        |output, _| {
            let roots =
                hir::CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export).unwrap();
            assert_eq!(
                names(output, &roots),
                ["Deep", "Exposed", "Hidden", "Token"]
            );
            let source = Table::from_export_hir(&output.output().export, &roots).unwrap();
            assert_eq!(source.records().len(), roots.values().len());
        },
    );
}

#[test]
fn nominal_source_roots_close_object_enum_and_compound_storage() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/storage-roots.scoop"
    ));
    // Pointer and Option declarations are local here; this checks source
    // closure without claiming ordinary imported generic capability.
    let output = crate::tests::lower_core_with_additional_declarations(
        scoop_parser::parse(source).unwrap().declarations,
    );
    let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(&output).unwrap();
    let identities = sources(output.module());
    let expected = [
        "Box",
        "Fields",
        "Payload",
        "Result",
        "Selection",
        "Storage",
        "Target",
    ];
    let mut actual = roots
        .values()
        .iter()
        .map(|owner| name(identities[owner]))
        .filter(|name| *name == "Unrelated" || expected.contains(name))
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(actual, expected);
    let source = Table::from_export_hir(&output, &roots).unwrap();
    assert_eq!(source.records().len(), roots.values().len());
}
