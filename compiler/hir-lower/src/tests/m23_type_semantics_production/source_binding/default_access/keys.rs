use super::*;

#[test]
fn default_access_accessor_keys_require_the_actual_logical_property_owner_key() {
    with_sources(OUTSIDE_ROOTS, |output, fixture, _, _| {
        let export = output.output().export.module();
        for name in ["global", "extra"] {
            let (id, property) = export
                .properties
                .iter()
                .find(|(_, p)| p.name == name)
                .unwrap();
            let subject = Subject::PropertyAccessor(
                export.property_accessor_identities[property.capability.getter()].id(),
            );
            let required = BTreeSet::from([subject]);
            let table = Table::from_export_hir(&output.output().export, &required).unwrap();
            assert_eq!(table.records().len(), 1);
            let mut canonical = fixture.foundation.as_canonical().clone();
            let owner = match &export.property_identities[id] {
                hir::HirPropertyIdentity::Ordinary(record) => {
                    canonical.set_properties(vec![]).unwrap();
                    Subject::Property(record.id())
                }
                hir::HirPropertyIdentity::Extension(record) => {
                    canonical.set_extension_properties(vec![]).unwrap();
                    Subject::ExtensionProperty(record.id())
                }
            };
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&incomplete, &fixture.identities)
                .unwrap();
            assert!(
                matches!(foundation.bind_default_access_declarations(&table, &required),
                Err(Error::MissingKey(id)) if id == owner)
            );
        }
    });
}

#[test]
fn default_access_binding_rejects_local_callable_owner_even_with_forged_source_record() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/access-local-declaration.scoop"
    ));
    with_hir_source(source, |output, _| {
        let export = output.output().export.module();
        let fixture = Fixture::from_output(output);
        let (_, local) = export
            .functions
            .iter()
            .find(|(id, _)| {
                let hir::HirFunctionIdentity::Source(identity) = &export.function_identities[*id]
                else {
                    return false;
                };
                identity.declaration().owners().owners().iter().any(|o| {
                    !matches!(
                        o,
                        scoop_identity::DefinitionOwnerAtom::Type(_)
                            | scoop_identity::DefinitionOwnerAtom::GenericType(_)
                    )
                })
            })
            .unwrap();
        let subject = function(export, &local.name);
        let outer = function(export, "outer");
        let origin = export
            .export_definition_origins
            .get(outer)
            .unwrap()
            .origin()
            .clone();
        let access = hir::DeclarationAccessSourceV1::try_new(
            hir::DeclaredVisibilityV1::Private,
            vec![],
            hir::ExportDefinitionSourceV1::new(origin),
        )
        .unwrap();
        let table = Table::try_new(vec![Record::try_new(subject, access).unwrap()]).unwrap();
        assert!(
            matches!(fixture.bind().unwrap().bind_default_access_declarations(&table, &BTreeSet::from([subject])),
            Err(Error::NonNominalOwner(id)) if id == subject)
        );
    });
}
