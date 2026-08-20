use super::*;

#[test]
fn generic_storage_retains_private_source_dependencies_and_shape_roots() {
    for name in ["private-storage", "private-storage-combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-source-mir-types");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        with_production(&source, |output, input, hir_types, graph, sources| {
            let section = public_interface(output);
            let export = &output.output().export;
            let local = &output.output().local;
            let owner = |name: &str| {
                let (id, _) = export
                    .classes
                    .iter()
                    .find(|(_, class)| class.name == name)
                    .unwrap();
                export.nominal_identities[id].concrete_type_id().unwrap()
            };
            let number = owner("Number");
            let declaration = section
                .nominal_interfaces()
                .declaration(hir::SourceNominalId::Concrete(number))
                .unwrap();
            assert_eq!(
                declaration.declaration_details().declared_visibility(),
                hir::DeclaredVisibilityV1::Private
            );
            assert!(
                section
                    .nominal_interfaces()
                    .declaration(hir::SourceNominalId::Concrete(owner("Unused")))
                    .is_none()
            );
            assert_eq!(
                section.public_bindings().records().len(),
                if name == "private-storage" { 4 } else { 2 }
            );
            let root = local
                .materialization()
                .roots()
                .iter()
                .find(|root| root.source() == number)
                .unwrap();
            assert!(
                matches!(sources.get(root.exact()).unwrap().origin(), MirTypeOriginV1::SourceNominal(id) if *id == number)
            );
            assert_eq!(
                local.materialization().roots().len(),
                if name == "private-storage" { 1 } else { 3 }
            );

            let types =
                scoop_mir_lower::lower_type_exports(local.module(), hir_types, input, graph)
                    .unwrap();
            assert!(types.records().iter().any(|record| {
                record.representation().variants().iter().any(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| field.value == root.exact())
                }) || record
                    .representation()
                    .fields()
                    .iter()
                    .any(|field| field.value == root.exact())
            }));
            let restored: scoop_mir::DecodedCanonicalParamFreeMirTypeExportsV1 = decoded(&types);
            assert_eq!(restored.validate(graph, input.foundation()).unwrap(), types);

            if name == "private-storage-combined" {
                let (id, _) = export
                    .structs
                    .iter()
                    .find(|(_, structure)| structure.name == "Dormant")
                    .unwrap();
                let dormant = export.nominal_identities[id].generic_type_id().unwrap();
                assert!(
                    section
                        .nominal_interfaces()
                        .declaration(hir::SourceNominalId::GenericTemplate(dormant))
                        .is_some()
                );
                assert!(
                    !types
                        .records()
                        .iter()
                        .any(|record| record.origin()
                            == &MirTypeOriginV1::NominalApplication(dormant))
                );
            }
        });
    }
}
