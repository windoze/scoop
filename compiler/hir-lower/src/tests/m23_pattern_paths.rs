use super::*;

fn qualified(parts: &[&str]) -> ast::QualifiedNameSyntax {
    let (first, rest) = parts
        .split_first()
        .expect("test qualified names are nonempty");
    ast::QualifiedNameSyntax {
        first: ident(first),
        rest: rest
            .iter()
            .map(|part| ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(part),
            })
            .collect(),
        span: sp(),
    }
}

fn package(mut source: ast::SourceFile, name: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: qualified(&[name]),
        span: sp(),
    };
    source
}

fn exact(package: &str, name: &str, alias: Option<&str>) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: qualified(&[package, name]),
        alias: alias.map(|alias| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(alias),
            span: sp(),
        }),
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn star(package: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Star {
        exposure: ast::ImportExposureSyntax::Local,
        namespace: qualified(&[package]),
        import_keyword_span: sp(),
        terminal_dot_span: sp(),
        star_span: sp(),
        span: sp(),
    }
}

fn type_alias(name: &str, target: TypeRef) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target,
        span: sp(),
    })
}

fn private_struct(name: &str) -> Decl {
    let Decl::Struct(mut declaration) = struct_decl(name, Vec::new()) else {
        panic!("struct helper creates a struct")
    };
    declaration.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    Decl::Struct(declaration)
}

fn lower_sources(sources: Vec<ast::SourceFile>) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let parsed = identified_test_sources(sources);
    let core = core_file();
    let input = LegacyCombinedSources::try_new(
        vec![ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core.scoop",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| CurrentSourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    lower_combined_sources(
        scoop_identity::RequestedConeKind::Library,
        &input,
        IntrinsicDeclarationPolicy::CoreOnly,
    )
}

fn destructure(path: &str, value: Expr) -> Statement {
    val_pat(false, pat_pos(&[path], Vec::new(), None), None, value)
}

#[test]
fn exact_import_alias_of_typealias_preserves_the_full_struct_application() {
    let api = package(
        file(vec![
            generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
            type_alias("TextBox", ty_generic("Box", vec![ty_named("String")])),
        ]),
        "api",
    );
    let mut user = file(vec![fun(
        "main",
        vec![val_pat(
            false,
            pat_pos(&["Payload"], vec![pat_bind("text")], None),
            None,
            call("Payload", vec![str_lit("value")]),
        )],
    )]);
    user.imports.push(exact("api", "TextBox", Some("Payload")));

    lower_sources(vec![api, user])
        .expect("an exact-import alias keeps its typed typealias application in patterns");
}

#[test]
fn typealias_pattern_requires_the_exact_generic_application() {
    let api = package(
        file(vec![
            generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
            type_alias("TextBox", ty_generic("Box", vec![ty_named("String")])),
        ]),
        "api",
    );
    let mut user = file(vec![fun(
        "main",
        vec![val_pat(
            false,
            pat_pos(&["Payload"], vec![pat_bind("value")], None),
            None,
            call("Box", vec![int_lit(1)]),
        )],
    )]);
    user.imports.extend([
        exact("api", "TextBox", Some("Payload")),
        exact("api", "Box", None),
    ]);

    let errors = lower_sources(vec![api, user])
        .expect_err("a typealias pattern cannot match another application of its struct");
    assert!(
        errors.iter().any(|error| {
            error.message.contains("pattern `Payload`")
                && error.message.contains("subject of type Box<Int>")
        }),
        "{errors:?}"
    );
}

#[test]
fn current_exact_and_star_nominals_resolve_as_struct_pattern_targets() {
    let current = file(vec![
        struct_decl("Point", Vec::new()),
        fun(
            "main",
            vec![destructure("Point", call("Point", Vec::new()))],
        ),
    ]);
    lower_sources(vec![current]).expect("a current-package struct is a direct pattern target");

    let api = package(
        file(vec![struct_decl("Point", Vec::new())]),
        "exact_geometry",
    );
    let mut user = file(vec![fun(
        "main",
        vec![destructure("Point", call("Point", Vec::new()))],
    )]);
    user.imports.push(exact("exact_geometry", "Point", None));
    lower_sources(vec![api, user]).expect("an exact-imported struct is a pattern target");

    let api = package(file(vec![struct_decl("Point", Vec::new())]), "geometry");
    let mut user = file(vec![fun(
        "main",
        vec![destructure("Point", call("Point", Vec::new()))],
    )]);
    user.imports.push(star("geometry"));
    lower_sources(vec![api, user]).expect("a star-imported struct is a pattern target");
}

#[test]
fn an_unimported_subject_declaration_name_is_not_a_pattern_target() {
    let api = package(file(vec![struct_decl("Point", Vec::new())]), "geometry");
    let mut user = file(vec![fun(
        "main",
        vec![destructure("Point", call("VisiblePoint", Vec::new()))],
    )]);
    user.imports
        .push(exact("geometry", "Point", Some("VisiblePoint")));

    let errors = lower_sources(vec![api, user])
        .expect_err("the subject's declaration spelling is not an implicit import");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("pattern `Point` does not match")),
        "{errors:?}"
    );
}

#[test]
fn a_wrong_exact_target_blocks_the_correct_current_package_target() {
    let wrong = package(file(vec![struct_decl("Wrong", Vec::new())]), "wrong");
    let producer = package(
        file(vec![
            struct_decl("Point", Vec::new()),
            fun_expr(
                "makePoint",
                Vec::new(),
                Vec::new(),
                Some(ty_named("Point")),
                call("Point", Vec::new()),
            ),
        ]),
        "app",
    );
    let mut consumer = package(
        file(vec![fun(
            "main",
            vec![destructure("Point", call("makePoint", Vec::new()))],
        )]),
        "app",
    );
    consumer
        .imports
        .push(exact("wrong", "Wrong", Some("Point")));

    let errors = lower_sources(vec![wrong, producer, consumer])
        .expect_err("a selected wrong exact target cannot retry the current-package type");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("pattern `Point` does not match")),
        "{errors:?}"
    );
}

#[test]
fn ambiguous_star_pattern_types_use_the_typed_lookup_diagnostic() {
    let left = package(file(vec![struct_decl("Pattern", Vec::new())]), "left");
    let right = package(file(vec![struct_decl("Pattern", Vec::new())]), "right");
    let mut user = file(vec![
        struct_decl("Actual", Vec::new()),
        fun(
            "main",
            vec![destructure("Pattern", call("Actual", Vec::new()))],
        ),
    ]);
    user.imports.extend([star("left"), star("right")]);

    let errors = lower_sources(vec![left, right, user])
        .expect_err("different typed star origins are ambiguous");
    assert!(
        errors.iter().any(|error| {
            error.message == "type `Pattern` is ambiguous in the star import layer"
                && error.notes.len() == 2
        }),
        "{errors:?}"
    );
}

#[test]
fn inaccessible_current_package_pattern_type_uses_the_access_diagnostic() {
    let declaration = package(file(vec![private_struct("Secret")]), "app");
    let consumer = package(
        file(vec![
            struct_decl("Actual", Vec::new()),
            fun(
                "main",
                vec![destructure("Secret", call("Actual", Vec::new()))],
            ),
        ]),
        "app",
    );

    let errors = lower_sources(vec![declaration, consumer])
        .expect_err("a private type in another source remains inaccessible to patterns");
    assert!(
        errors.iter().any(|error| {
            error.message == "type `Secret` is not accessible from this source location"
        }),
        "{errors:?}"
    );
}
