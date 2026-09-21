use super::*;

#[test]
fn default_access_rejects_object_initializers_despite_actual_constructor_keys_and_origins() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/access-internal-initializers.scoop"
    ));
    for source in [source, COMBINATIONS] {
        with_sources(source, |output, fixture, required, table| {
            let export = output.output().export.module();
            let foundation = fixture.bind().unwrap();
            let mut checked = 0;
            for (id, object) in export.objects.iter() {
                let owner = export.nominal_identities[id].source().unwrap();
                if owner.declaration().origin() != export.cone {
                    continue;
                }
                let owner = match owner {
                    hir::HirSourceNominalIdentity::Concrete(record) => {
                        hir::SourceNominalId::Concrete(record.id())
                    }
                    hir::HirSourceNominalIdentity::Generic(record) => {
                        hir::SourceNominalId::GenericTemplate(record.id())
                    }
                };
                let constructor = export.classes[object.backing_class].constructors[0];
                let identity = export.constructor_identities[constructor]
                    .source_record()
                    .unwrap();
                let subject = Subject::Constructor(identity.id());
                assert!(!required.contains(&subject));
                assert!(table.get(subject).is_none());
                assert!(matches!(
                    Table::from_export_hir(
                        &output.output().export,
                        &BTreeSet::from([subject]),
                        &mut meter()
                    ),
                    Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
                ));
                let owner_access = table
                    .get(nominal_subject(owner))
                    .unwrap()
                    .declaration_access();
                let mut owners = owner_access.lexical_owners().to_vec();
                owners.push(owner);
                let access = hir::DeclarationAccessSourceV1::try_new(
                    hir::DeclaredVisibilityV1::Private,
                    owners,
                    hir::ExportDefinitionSourceV1::new(
                        export
                            .export_definition_origins
                            .get(subject)
                            .unwrap()
                            .origin()
                            .clone(),
                    ),
                )
                .unwrap();
                let mut records = table.records().to_vec();
                records.push(Record::try_new(subject, access).unwrap());
                records.sort_by_key(Record::subject);
                let forged = Table::try_new(records, &mut meter()).unwrap();
                let mut demand = required.clone();
                demand.insert(subject);
                assert!(matches!(
                    foundation.bind_default_access_declarations(&forged, &demand, &mut meter()),
                    Err(Error::ConstructorOwner(id)) if id == subject
                ));
                checked += 1;
            }
            assert!(checked > 0);
        });
    }
}
