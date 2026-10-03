use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId, SourceDeclarationKey};

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
                    hir::SourceNominalId::Concrete(owner) => {
                        hir_types.representation_support().get(owner).is_some()
                    }
                    hir::SourceNominalId::GenericTemplate(_) => false,
                };
                assert_eq!(mir_policy.is_some(), materialized);
            }
        });
    }
}
