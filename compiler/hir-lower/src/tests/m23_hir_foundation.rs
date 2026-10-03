use std::collections::HashSet;

use super::*;

fn reference(name: &str) -> Expr {
    Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident(name),
        span: sp(),
    }
}

#[test]
fn complete_hir_output_projects_one_canonical_foundation() {
    let output = lower_user_output(file(vec![
        fun_expr(
            "identity",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun_sig(
            "copy",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            vec![ret(Some(var("value")))],
        ),
        fun(
            "main",
            vec![
                stmt(call("copy", vec![int_lit(1)])),
                stmt(call("copy", vec![str_lit("value")])),
                val_ty(
                    "operation",
                    Some(ty_function(false, vec![ty_named("Int")], ty_named("Int"))),
                    reference("identity"),
                ),
            ],
        ),
    ]))
    .expect("the HIR foundation fixture must lower");
    let export = output.export.module();
    let local = output.local.module();

    let foundation =
        hir::CanonicalHirFoundation::from_modules(export, local, &output.native_boundary_types)
            .expect("validated HIR products must project atomically");
    let counts = foundation.counts();

    assert_eq!(counts.sources, export.source_files.len());
    assert_eq!(
        counts.export_bindings,
        export.export_binding_identities.iter().count()
    );
    assert_eq!(
        counts.callable_applications,
        local.callable_applications.records().len()
    );
    assert_eq!(
        counts.dispatch_slots,
        export.dispatch_slot_identities.records().count()
    );
    assert_eq!(
        counts.initialization_units,
        export.initialization_unit_identities.records().len()
    );
    assert_eq!(
        counts.source_contexts,
        export.source_context_identities.len()
    );
    assert_eq!(counts.local_bindings, export.local_binding_identities.len());
    assert_eq!(
        counts.local_values,
        local.local_value_identities.records().len()
    );
    assert_eq!(
        counts.callback_registrations,
        export.callback_registration_identities.records().len()
    );
    assert_eq!(
        counts.source_native_contracts,
        export.source_native_contracts.len()
    );
    assert_eq!(
        counts.odr_groups,
        local
            .callable_applications
            .odr_group_records()
            .iter()
            .chain(local.exact_type_identities.nominal_specialization_records())
            .map(|record| record.id())
            .collect::<HashSet<_>>()
            .len()
    );
    assert!(
        !local
            .exact_type_identities
            .nominal_specialization_records()
            .is_empty()
    );
    assert_eq!(
        counts.odr_members,
        local.callable_applications.odr_member_records().len()
    );
    let mut definition_subjects: HashSet<_> = export
        .export_definition_origins
        .records()
        .iter()
        .chain(local.local_value_identities.definition_origins().records())
        .map(|record| record.subject())
        .collect();
    definition_subjects.extend(local.callable_references.iter().map(|(_, reference)| {
        scoop_identity::DefinitionOriginSubject::GeneratedCallable(
            reference.identity.callable_record().id(),
        )
    }));
    assert_eq!(counts.definition_origins, definition_subjects.len());
    assert_eq!(
        counts.native_boundary_types,
        output.native_boundary_types.records().len()
    );

    let mut exact_types = HashSet::new();
    for (ty, _) in export.types.iter() {
        if let Some(record) = export.type_identities[ty].exact() {
            exact_types.insert(record.id());
        }
    }
    exact_types.extend(
        local
            .types
            .iter()
            .map(|(ty, _)| local.exact_type_identities[ty].id()),
    );
    assert_eq!(counts.exact_types, exact_types.len());
    assert!(counts.generic_functions > 0);
    assert!(counts.generated_callables > 0);

    let direct_public = hir::CanonicalDirectPublicSurfaceV1::from_export_hir(export)
        .expect("checked export bindings must form one canonical direct-public surface");
    assert_eq!(direct_public.bindings().len(), counts.export_bindings);

    assert!(matches!(
        hir::OdrFreeHirFoundation::from_modules(export, local, &output.native_boundary_types,),
        Err(hir::OdrFreeHirFoundationProjectionError::Odr(
            hir::OdrFreeHirFoundationError::CallableApplication(_)
        ))
    ));
}

#[test]
fn exact_nominal_applications_publish_groups_without_physical_members() {
    let output = lower_user_output(file(vec![fun("main", Vec::new())]))
        .expect("the parameter-free HIR fixture must lower");
    let export = output.export.module();
    let local = output.local.module();

    assert!(local.callable_applications.is_empty());
    assert!(export.types.iter().any(|(ty, _)| matches!(
            export.type_identities[ty]
                .exact()
                .map(|record| record.key()),
            Some(scoop_identity::ExactTypeKey::NominalApplication { .. })
        )));

    let foundation =
        hir::CanonicalHirFoundation::from_modules(export, local, &output.native_boundary_types)
            .expect("nominal applications publish their existing source groups");
    let groups: HashSet<_> = local
        .exact_type_identities
        .nominal_specialization_records()
        .iter()
        .map(|record| record.id())
        .collect();
    assert!(!groups.is_empty());
    assert_eq!(foundation.counts().odr_groups, groups.len());
    assert_eq!(foundation.counts().odr_members, 0);
    assert!(matches!(
        hir::OdrFreeHirFoundation::try_new(foundation),
        Err(hir::OdrFreeHirFoundationError::OdrGroup(group)) if groups.contains(&group)
    ));
}
