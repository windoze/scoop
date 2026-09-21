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
            Table::from_export_hir(
                &output.output().export,
                &BTreeSet::from([subject]),
                &mut meter()
            ),
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
                Table::from_export_hir(
                    &output.output().export,
                    &BTreeSet::from([subject]),
                    &mut meter()
                ),
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
            matches!(Table::from_export_hir(&output.output().export, &BTreeSet::from([field]), &mut meter()),
            Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(hir::SourceInventoryError::InvalidDefaultAccessSubject(id))) if id == field)
        );
        assert!(
            matches!(Record::try_new(field, table(output).records()[0].declaration_access().clone()),
            Err(hir::SourceInventoryError::InvalidDefaultAccessSubject(id)) if id == field)
        );
    });
}

#[test]
fn default_access_projection_and_resolution_use_shared_resource_budgets() {
    for source in [SOURCE, COMBINATIONS] {
        with_hir_source(source, |output, _| {
            let required = required(output.output().export.module());
            let mut measured = meter();
            let table =
                Table::from_export_hir(&output.output().export, &required, &mut measured).unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: work * 2 - 1,
                ..DecodeLimits::default()
            });
            Table::from_export_hir(&output.output().export, &required, &mut shared).unwrap();
            assert!(
                Table::from_export_hir(&output.output().export, &required, &mut shared).is_err()
            );
            let decoded: Decoded =
                decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
            let mut ids = identity_closure(output);
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
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_leaf_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    Table::from_export_hir(
                        &output.output().export,
                        &required,
                        &mut BudgetMeter::new(limits)
                    )
                    .is_err(),
                    "{limits:?}"
                );
                assert!(
                    decoded
                        .clone()
                        .resolve(&mut ids, &mut BudgetMeter::new(limits))
                        .is_err(),
                    "{limits:?}"
                );
            }
        });
    }
}
