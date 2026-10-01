use super::*;

#[test]
fn initialization_and_object_ids_follow_materialized_entities_after_skipped_declarations() {
    with_hir_source(&fixture("initialization-demand"), |output, _| {
        let export = output.output().export.module();
        let local = output.output().local.module();
        assert_eq!(export.objects.len(), 3);
        assert_eq!(local.objects.len(), 2);
        assert_eq!(local.companion_relations.len(), 1);
        assert_eq!(local.initialization_units.len(), 3);
        assert_eq!(local.initialization_failure_roots.len(), 3);
        assert!(!nominal_names(local).contains("DeferredHost"));
        for (_, unit) in local.initialization_units.iter() {
            assert!(!local.functions[unit.initializer].is_suspend);
            assert!(!local.functions[unit.ensure].is_suspend);
        }
        for (id, object) in local.objects.iter() {
            let (source_id, source) = export
                .objects
                .iter()
                .find(|(source, _)| export.nominal_identities[*source] == object.origin)
                .unwrap();
            assert_ne!(source_id.into_raw(), id.into_raw());
            let object_type = local.object_types[object.object_type];
            let value = local.singleton_values[object.singleton_value];
            let root = &local.singleton_published_roots[value.published_root];
            assert_eq!(object_type.declaration, id);
            assert_eq!(object_type.representation, object.backing_class);
            assert_eq!(
                object_type.canonical_type,
                local.classes[object.backing_class].canonical_type
            );
            assert_eq!(
                value.identity,
                export.object_value_identities[source.singleton_value].id()
            );
            assert_eq!(value.declaration, id);
            assert_eq!(value.object_type, object.object_type);
            assert_eq!(root.value, object.singleton_value);
            assert_eq!(root.ty, object_type.canonical_type);
            let unit = &local.initialization_units[value.initialization];
            let source_unit = export.singleton_values[source.singleton_value].initialization;
            assert_eq!(
                unit.identity,
                export.initialization_unit_identities[source_unit]
            );
            assert_eq!(
                unit.kind,
                concrete::InitializationUnitKind::LazySingleton {
                    value: object.singleton_value,
                    published_root: value.published_root,
                }
            );
            if let concrete::ObjectKind::Companion(relation) = object.kind {
                let relation = &local.companion_relations[relation];
                assert_eq!(relation.object, id);
                assert!(matches!(relation.host, concrete::NominalOwner::Class(_)));
            }
        }
        for (id, unit) in local.initialization_units.iter() {
            let (source_id, source) = export
                .initialization_units
                .iter()
                .find(|(source, _)| export.initialization_unit_identities[*source] == unit.identity)
                .unwrap();
            assert_eq!(
                local.initialization_failure_roots[unit.failure_root].unit,
                id
            );
            assert_eq!(
                local.functions[unit.initializer].name,
                export.functions[source.initializer].name
            );
            assert_eq!(
                local.functions[unit.ensure].name,
                export.functions[source.ensure].name
            );
            assert!(matches!(
                export.functions[source.initializer].kind,
                hir::FunctionKind::User(_)
            ));
            assert!(matches!(
                export.functions[source.ensure].kind,
                hir::FunctionKind::InitializationEnsure
            ));
            assert!(matches!(
                local.functions[unit.initializer].kind,
                concrete::FunctionKind::User(_)
            ));
            assert!(matches!(
                local.functions[unit.ensure].kind,
                concrete::FunctionKind::InitializationEnsure
            ));
            let expected = source
                .dependencies
                .iter()
                .map(|edge| export.initialization_unit_identities[edge.unit].id())
                .collect::<Vec<_>>();
            let actual = unit
                .dependencies
                .iter()
                .map(|edge| local.initialization_units[edge.unit].identity.id())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{source_id:?}");
            if let concrete::InitializationUnitKind::EagerTopLevel { storage } = unit.kind {
                assert!(
                    matches!(local.globals[storage].storage, concrete::GlobalStorage::Managed {
                    state: concrete::HirStaticInitialState::ZeroedForRuntimeUnit { unit },
                } if unit == id)
                );
            }
        }
    });
}
