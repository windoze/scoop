use super::*;
use scoop_identity::DefinitionOwnerAtom;

#[test]
fn default_target_queries_follow_dependency_ordered_artifact_declarations() {
    with_hir_source(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/target-dependency-order.scoop"
        )),
        |output, _| {
            let export = output.output().export.module();
            // At least one child sorts before its parent in the same ID domain.
            // Canonical foundation ordering must nevertheless place parents first.
            let mut concrete_inversion = false;
            let mut generic_inversion = false;
            let identities = export
                .structs
                .iter()
                .map(|(id, _)| &export.nominal_identities[id])
                .chain(
                    export
                        .classes
                        .iter()
                        .map(|(id, _)| &export.nominal_identities[id]),
                )
                .chain(
                    export
                        .enums
                        .iter()
                        .map(|(id, _)| &export.nominal_identities[id]),
                )
                .chain(
                    export
                        .objects
                        .iter()
                        .map(|(id, _)| &export.nominal_identities[id]),
                );
            for source in identities.filter_map(hir::HirNominalIdentity::source) {
                match (source, source.declaration().owners().owners().last()) {
                    (
                        hir::HirSourceNominalIdentity::Concrete(record),
                        Some(DefinitionOwnerAtom::Type(parent)),
                    ) => concrete_inversion |= record.id() < *parent,
                    (
                        hir::HirSourceNominalIdentity::Generic(record),
                        Some(DefinitionOwnerAtom::GenericType(parent)),
                    ) => generic_inversion |= record.id() < *parent,
                    _ => continue,
                }
            }
            assert!(
                concrete_inversion || generic_inversion,
                "fixture must contain an ID order inversion on a nominal dependency"
            );
            let fixture = Fixture::from_output(output);
            let foundation = fixture.bind().unwrap();
            for (target, expected, _) in expected::targets(export) {
                assert_eq!(
                    foundation
                        .default_indirect_access_subject(target, &mut meter())
                        .unwrap(),
                    expected
                );
            }
            for (name, position) in [
                ("Container.alpha", 0),
                ("Container.cell", 0),
                ("Container.choice", 0),
                ("GenericContainer.alpha", 1),
                ("GenericContainer.cell", 1),
                ("GenericContainer.choice", 1),
            ] {
                let record = constructors::reference(output, name, position);
                foundation
                    .default_constructor_access_subject(record.target(), &mut meter())
                    .unwrap();
            }
        },
    );
}
