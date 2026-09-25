use super::*;

#[test]
fn default_access_sources_reject_local_callable_owner_chains() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/access-local-declaration.scoop"
    ));
    with_hir_source(source, |output, _| {
        let export = output.output().export.module();
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
        assert!(matches!(
            Table::from_export_hir(&output.output().export, &BTreeSet::from([subject])),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidLexicalOwner(_))
        ));
    });
}

#[test]
fn default_access_declarations_reject_foreign_missing_roles_and_const_accessors() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let foreign = Subject::Type(
            scoop_identity::CoreBuiltinNominal::Any
                .identity_record()
                .id(),
        );
        let missing = with_hir_source("private fun absent(): Int = 0", |other, _| {
            function(other.output().export.module(), "absent")
        });
        let (_, constant) = export
            .properties
            .iter()
            .find(|(_, p)| p.name == "fixed")
            .unwrap();
        let constant = Subject::PropertyAccessor(
            export.property_accessor_identities[constant.capability.getter()].id(),
        );
        for subject in [foreign, missing, constant] {
            assert!(matches!(
                Table::from_export_hir(&output.output().export, &BTreeSet::from([subject])),
                Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(_))
            ));
        }
        let field = export
            .export_definition_origins
            .records()
            .iter()
            .find(|r| matches!(r.subject(), Subject::Field(_)))
            .unwrap()
            .subject();
        assert!(
            matches!(Table::from_export_hir(&output.output().export, &BTreeSet::from([field])),
            Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(hir::SourceInventoryError::InvalidDefaultAccessSubject(id))) if id == field)
        );
        assert!(
            matches!(Record::try_new(field, table(output).records()[0].declaration_access().clone()),
            Err(hir::SourceInventoryError::InvalidDefaultAccessSubject(id)) if id == field)
        );
    });
}
