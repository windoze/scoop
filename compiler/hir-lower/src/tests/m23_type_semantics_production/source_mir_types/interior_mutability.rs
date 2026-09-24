use super::*;
use scoop_identity::{DeclarationName, ExactTypeKey, PersistentExactTypeId, SourceDeclarationKey};

#[test]
fn shared_struct_policy_matches_source_attributes_and_actual_mir_representation() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-interior-mutability");
    for case in ["standalone", "combined"] {
        let source = std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
        with_production(&source, |output, _, hir_types, graph, mir| {
            let interface = public_interface(output);
            let original = interface.nominal_interfaces();
            let restored: hir::DecodedCanonicalNominalInterfacesV1 = decoded(original);
            let restored = restored.resolve(graph).unwrap();
            assert_eq!(&restored, original);
            let export = output.output().export.module();
            let mut rows = Vec::new();
            for record in restored.all_records() {
                let hir::NominalSourceShapeV1::Struct(shape) = record.source_shape() else {
                    continue;
                };
                let (key, exact) = match record.declaration() {
                    hir::SourceNominalId::Concrete(id) => (
                        graph.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
                        Some(PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id)).unwrap()),
                    ),
                    hir::SourceNominalId::GenericTemplate(id) => (
                        graph.canonical_key::<_, SourceDeclarationKey>(id).unwrap(),
                        None,
                    ),
                };
                let (_, declaration) = export
                    .structs
                    .iter()
                    .find(|(id, _)| {
                        export.nominal_identities[*id]
                            .source()
                            .is_some_and(|identity| identity.declaration() == key.as_ref())
                    })
                    .unwrap();
                assert_eq!(
                    shape.interior_mutable(),
                    declaration.attributes.interior_mutable
                );
                let mir_policy = exact
                    .and_then(|exact| mir.get(exact))
                    .map(|representation| {
                        let scoop_mir::MirTypeRepresentationV1::Struct {
                            interior_mutable, ..
                        } = representation.representation()
                        else {
                            panic!("the source fixture declares a struct");
                        };
                        assert_eq!(*interior_mutable, shape.interior_mutable());
                        *interior_mutable
                    });
                let materialized = match record.declaration() {
                    hir::SourceNominalId::Concrete(owner) => hir_types
                        .section()
                        .representation_support()
                        .get(owner)
                        .is_some(),
                    hir::SourceNominalId::GenericTemplate(_) => false,
                };
                assert_eq!(mir_policy.is_some(), materialized);
                let DeclarationName::Named(name) = key.name() else {
                    panic!("named source struct");
                };
                rows.push(format!(
                    "{}: public={} generic={} interior_mutable={} mir={mir_policy:?}\n",
                    name.as_str(),
                    restored.get(record.declaration()).is_some(),
                    exact.is_none(),
                    shape.interior_mutable(),
                ));
            }
            rows.sort();
            if std::env::var_os("SCOOP_UPDATE_SHARED_INTERIOR_MUTABILITY").is_some() {
                std::fs::write(fixtures.join(format!("{case}.snap")), rows.concat()).unwrap();
            }
            assert_eq!(
                rows.concat(),
                std::fs::read_to_string(fixtures.join(format!("{case}.snap"))).unwrap()
            );
        });
    }
}
