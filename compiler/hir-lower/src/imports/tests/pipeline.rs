use super::*;

#[test]
fn pipeline_materializes_importable_nominals_aliases_properties_variants_and_static_members() {
    use crate::tests::{
        class_decl, enum_decl, fun, interface_decl, struct_decl, ty_named, variant_unit,
    };
    let ast::Decl::Struct(nested) = struct_decl("Nested", Vec::new()) else {
        unreachable!()
    };
    let object = ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident("Tools"),
        supertypes: Vec::new(),
        members: vec![
            ast::ClassMember::Function(method("answer")),
            ast::ClassMember::StoredProperty(constant("version")),
            ast::ClassMember::Nested(Box::new(ast::NestedNominalDecl::Struct(Box::new(nested)))),
        ],
        span: sp(),
    });
    let ast::Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        vec![method("instanceOnly")],
    ) else {
        unreachable!()
    };
    host.members.push(ast::ClassMember::Companion(Box::new(
        ast::CompanionObjectDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            name: ast::CompanionNameSyntax::Named(ident("Factory")),
            supertypes: Vec::new(),
            members: vec![
                ast::ClassMember::Function(method("make")),
                ast::ClassMember::StoredProperty(constant("revision")),
                ast::ClassMember::Companion(Box::new(ast::CompanionObjectDecl {
                    annotations: Vec::new(),
                    visibility: ast::VisibilitySyntax::Omitted,
                    name: ast::CompanionNameSyntax::Named(ident("Inner")),
                    supertypes: Vec::new(),
                    members: vec![ast::ClassMember::Function(method("deep"))],
                    span: sp(),
                })),
            ],
            span: sp(),
        },
    )));
    let mut extension = method("extension");
    extension.receiver_ty = Some(ty_named("Int"));
    let declarations = package(
        file(vec![
            struct_decl("Item", Vec::new()),
            interface_decl("Contract", Vec::new()),
            enum_decl("State", Vec::new(), vec![variant_unit("Ready")]),
            ast::Decl::TypeAlias(ast::TypeAliasDecl {
                visibility: ast::VisibilitySyntax::Omitted,
                name: ident("Alias"),
                target: ty_named("Item"),
                span: sp(),
            }),
            ast::Decl::Global(constant("answer")),
            fun("helper", Vec::new()),
            ast::Decl::Function(extension),
            object,
            ast::Decl::Class(host),
        ]),
        &["api"],
    );
    let mut consumer = file(vec![fun("main", Vec::new())]);
    consumer.imports = [
        vec!["api", "Item"],
        vec!["api", "Contract"],
        vec!["api", "State"],
        vec!["api", "State", "Ready"],
        vec!["api", "Alias"],
        vec!["api", "answer"],
        vec!["api", "helper"],
        vec!["api", "extension"],
        vec!["api", "Tools"],
        vec!["api", "Tools", "answer"],
        vec!["api", "Tools", "version"],
        vec!["api", "Tools", "Nested"],
        vec!["api", "Host", "make"],
        vec!["api", "Host", "revision"],
        vec!["api", "Host", "Factory", "make"],
        vec!["api", "Host", "Companion", "make"],
        vec!["api", "Host", "Factory", "deep"],
    ]
    .iter()
    .map(|selector| exact(selector, None, false))
    .collect();
    consumer.imports.extend([
        star(&["api", "Tools"], false),
        star(&["api", "Host"], false),
        star(&["api", "State"], false),
    ]);
    lower_sources(vec![declarations.clone(), consumer.clone()])
        .expect("all importable source categories materialize typed targets");
    lower_sources(vec![consumer.clone(), declarations.clone()])
        .expect("source order does not affect binding materialization");
    consumer.imports = vec![exact(&["api", "Host", "deep"], None, false)];
    let errors = lower_sources(vec![declarations.clone(), consumer.clone()])
        .expect_err("companion forwarding is one typed namespace edge");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "import target is not available in the current compilation unit"
    );
    consumer.imports = vec![exact(&["api", "Host", "instanceOnly"], None, false)];
    let errors = lower_sources(vec![declarations, consumer])
        .expect_err("instance methods are not importable static members");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "import target is not available in the current compilation unit"
    );
}

#[test]
fn cross_file_ordinary_imports_accept_public_bindings() {
    use crate::tests::{call, fun, stmt};

    let declarations = package(
        file(vec![declared_function(
            "published",
            ast::DeclaredVisibility::Public,
        )]),
        &["api"],
    );
    for import in [
        exact(&["api", "published"], None, false),
        star(&["api"], false),
    ] {
        let mut consumer = file(vec![fun(
            "usePublished",
            vec![stmt(call("published", Vec::new()))],
        )]);
        consumer.imports.push(import);
        lower_sources(vec![declarations.clone(), consumer])
            .expect("ordinary exact and star imports may cross files to a public binding");
    }
}

#[test]
fn import_failure_classes_return_no_hir_output() {
    let public_api = package(
        file(vec![declared_function(
            "published",
            ast::DeclaredVisibility::Public,
        )]),
        &["api"],
    );
    let private_api = package(
        file(vec![declared_function(
            "secret",
            ast::DeclaredVisibility::Private,
        )]),
        &["privateApi"],
    );
    let empty_api = package(file(Vec::new()), &["emptyApi"]);
    let mut own_private = file(vec![declared_function(
        "ownSecret",
        ast::DeclaredVisibility::Private,
    )]);
    own_private.imports.push(exact(&["ownSecret"], None, true));

    let cases = vec![
        (
            "public exact of a public current-unit target",
            vec![
                public_api.clone(),
                with_import(exact(&["api", "published"], None, true)),
            ],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "public star of an accessible nonempty current-unit namespace",
            vec![public_api, with_import(star(&["api"], true))],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "public star of an empty current-unit namespace",
            vec![empty_api, with_import(star(&["emptyApi"], true))],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "public exact of a same-file private target",
            vec![own_private],
            "public import requires a direct dependency target",
            0,
        ),
        (
            "ordinary exact of a cross-file private target",
            vec![
                private_api,
                with_import(exact(&["privateApi", "secret"], None, false)),
            ],
            "import target is not accessible from this source location",
            1,
        ),
        (
            "ordinary star of an unavailable namespace",
            vec![with_import(star(&["unknown", "space"], false))],
            "import target is not available in the current compilation unit",
            0,
        ),
        (
            "public star of an unavailable namespace",
            vec![with_import(star(&["unknown", "space"], true))],
            "import target is not available in the current compilation unit",
            0,
        ),
        (
            "public star of the explicit core namespace",
            vec![with_import(star(&["scoop", "core"], true))],
            "import target is not available in the current compilation unit",
            0,
        ),
    ];

    for (case, sources, expected_message, expected_notes) in cases {
        let errors = lower_sources(sources).expect_err(case);
        assert_eq!(errors.len(), 1, "{case}: {errors:#?}");
        assert_eq!(errors[0].message, expected_message, "{case}");
        assert_eq!(errors[0].notes.len(), expected_notes, "{case}");
    }
}

#[test]
fn non_prelude_core_variant_is_not_a_bare_name() {
    use crate::tests::{enum_decl, fun, val, var, variant_unit};

    let mut core = crate::tests::core_file();
    core.declarations.push(enum_decl(
        "CoreOnlyState",
        Vec::new(),
        vec![variant_unit("CoreOnlyReady")],
    ));
    crate::tests::make_core_public(&mut core);
    let user = file(vec![fun(
        "probe",
        vec![val("chosen", var("CoreOnlyReady"))],
    )]);

    let errors = lower_sources_with_core(vec![user], core)
        .expect_err("only typed core-prelude variants may be used as bare names");
    assert!(errors.iter().any(|error| {
        error
            .message
            .starts_with("unknown variable `CoreOnlyReady`")
    }));
}

#[test]
fn failed_import_stops_before_unrelated_body_lowering() {
    use crate::tests::{call, fun, stmt};
    let declarations = package(file(vec![fun("helper", Vec::new())]), &["api"]);
    let mut consumer = file(vec![fun(
        "main",
        vec![stmt(call("unrelatedMissingFunction", Vec::new()))],
    )]);
    consumer
        .imports
        .push(exact(&["api", "helper"], Some("alias"), true));
    let errors = lower_sources(vec![declarations, consumer])
        .expect_err("failed public import cannot produce HIR");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "public import requires a direct dependency target"
    );
}

#[test]
fn public_import_pipeline_does_not_downgrade_to_an_ordinary_import() {
    let public_api = package(
        file(vec![declared_function(
            "published",
            ast::DeclaredVisibility::Public,
        )]),
        &["publicApi"],
    );
    let private_api = package(
        file(vec![declared_function(
            "secret",
            ast::DeclaredVisibility::Private,
        )]),
        &["privateApi"],
    );

    let aliased_import = exact(&["publicApi", "published"], Some("renamed"), true);
    let aliased_span = selector_span(&aliased_import);
    let mut aliased_consumer = file(vec![fun(
        "useAliased",
        vec![stmt(call("renamed", Vec::new()))],
    )]);
    aliased_consumer.imports.push(aliased_import);

    let private_exact_import = exact(&["privateApi", "secret"], None, true);
    let private_exact_span = selector_span(&private_exact_import);
    let mut private_exact_consumer = file(vec![fun(
        "usePrivateExact",
        vec![stmt(call("secret", Vec::new()))],
    )]);
    private_exact_consumer.imports.push(private_exact_import);

    let private_star_import = star(&["privateApi"], true);
    let private_star_span = selector_span(&private_star_import);
    let mut private_star_consumer = file(vec![fun(
        "usePrivateStar",
        vec![stmt(call("secret", Vec::new()))],
    )]);
    private_star_consumer.imports.push(private_star_import);

    for (case, sources, expected_span) in [
        (
            "public exact alias",
            vec![public_api, aliased_consumer],
            aliased_span,
        ),
        (
            "cross-file private public exact",
            vec![private_api.clone(), private_exact_consumer],
            private_exact_span,
        ),
        (
            "private-only public star",
            vec![private_api, private_star_consumer],
            private_star_span,
        ),
    ] {
        let errors = lower_sources(sources).expect_err(case);
        assert_eq!(errors.len(), 1, "{case}: {errors:#?}");
        assert_eq!(
            errors[0].message, "public import requires a direct dependency target",
            "{case}: the public form must neither bind locally nor degrade to ordinary lookup"
        );
        assert_eq!(errors[0].span, Some(expected_span), "{case}");
    }
}
