use super::*;

fn source<'a>(
    source: &'a ast::SourceFile,
    identity: scoop_identity::SourceIdentity,
    provider: hir::IntrinsicProviderId,
    name: &'a str,
) -> ProviderSource<'a> {
    ProviderSource {
        source,
        identity,
        provider,
        name,
        source_text: "",
    }
}

fn lower_user_sources(
    core: &ast::SourceFile,
    first: &ast::SourceFile,
    remaining: &[(&ast::SourceFile, &str)],
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let core_provider = hir::IntrinsicProviderId::from_raw(17);
    let user_provider = hir::IntrinsicProviderId::from_raw(29);
    let parsed = identified_test_sources(
        std::iter::once(first.clone())
            .chain(remaining.iter().map(|(source, _)| (*source).clone()))
            .collect(),
    );
    let input = LegacyCombinedSources::try_new(
        vec![source(
            core,
            core_source_identity("src/core.scoop"),
            core_provider,
            "core.scoop",
        )],
        user_provider,
        parsed,
        |_| CurrentSourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn set_private(declaration: &mut ast::Decl) {
    let ast::Decl::Function(function) = declaration else {
        panic!("the test declaration is a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
}

fn with_package(mut source: ast::SourceFile, segments: &[&str]) -> ast::SourceFile {
    let Some((first, rest)) = segments.split_first() else {
        source.package = ast::PackageSyntax::RootPackage;
        return source;
    };
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: ast::QualifiedNameSyntax {
            first: ident(first),
            rest: rest
                .iter()
                .map(|segment| ast::QualifiedNameTailSyntax {
                    dot_span: sp(),
                    identifier: ident(segment),
                })
                .collect(),
            span: sp(),
        },
        span: sp(),
    };
    source
}

fn exact_import(segments: &[&str]) -> ast::ImportSyntax {
    let selector = ast::QualifiedNameSyntax {
        first: ident(segments[0]),
        rest: segments[1..]
            .iter()
            .map(|segment| ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(segment),
            })
            .collect(),
        span: sp(),
    };
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector,
        alias: None,
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn qualified_type(path: &[&str]) -> ast::TypeRef {
    ast::TypeRef {
        kind: ast::TypeRefKind::Qualified {
            path: path.iter().map(|segment| ident(segment)).collect(),
            arguments: Vec::new(),
        },
        span: sp(),
    }
}

fn private_struct(name: &str) -> ast::Decl {
    let ast::Decl::Struct(mut declaration) = struct_decl(name, Vec::new()) else {
        panic!("the struct builder returns a struct")
    };
    declaration.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    ast::Decl::Struct(declaration)
}

#[test]
fn multiple_user_sources_share_one_current_unit_and_keep_file_private_domains() {
    let core = core_file();
    let mut private = fun("privateHelper", Vec::new());
    set_private(&mut private);
    let first = file(vec![
        fun("sharedHelper", Vec::new()),
        private,
        fun("sameFile", vec![stmt(call("privateHelper", Vec::new()))]),
    ]);
    let second = file(vec![fun(
        "main",
        vec![
            stmt(call("sharedHelper", Vec::new())),
            stmt(call("sameFile", Vec::new())),
        ],
    )]);

    let output = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("all user sources are one current-unit declaration side");
    let module = output.export;
    let user_provider = hir::IntrinsicProviderId::from_raw(29);
    let user_cone = test_source_identity("src/first.scoop").cone();
    let shared = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "sharedHelper").then_some(function))
        .expect("the cross-source helper is present");
    assert_eq!(
        shared.access.lookup.0.constraints(),
        &[hir::AccessConstraint::Cone(user_cone)]
    );
    let private = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "privateHelper").then_some(function))
        .expect("the private helper is present");
    assert_eq!(
        private.access.lookup.0.constraints(),
        &[
            hir::AccessConstraint::Cone(user_cone),
            hir::AccessConstraint::File(test_source_identity("src/first.scoop")),
        ]
    );
    assert_eq!(module.source_files[1].provider, user_provider);
    assert_eq!(module.source_files[2].provider, user_provider);
}

#[test]
fn private_declaration_does_not_cross_user_source_slots() {
    let core = core_file();
    let mut private = fun("privateHelper", Vec::new());
    set_private(&mut private);
    let first = file(vec![private]);
    let second = file(vec![fun(
        "main",
        vec![stmt(call("privateHelper", Vec::new()))],
    )]);

    let errors = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect_err("file-private lookup must use the exact user source slot");
    assert!(errors.iter().any(|error| {
        error.file == 2 && error.message == "function `privateHelper` is not accessible here"
    }));
}

#[test]
fn definition_origins_use_semantic_source_identity_when_spans_are_identical() {
    let core = core_file();
    let first = file(vec![fun("firstSource", Vec::new())]);
    let second = file(vec![fun("main", Vec::new())]);

    let output = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("both source declarations must lower");
    let module = output.export;
    for (name, expected_source) in [
        ("firstSource", test_source_identity("src/first.scoop")),
        ("main", test_source_identity("src/second.scoop")),
    ] {
        let (function, _) = module
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap_or_else(|| panic!("missing function {name}"));
        let hir::HirSourceFunctionIdentity::Plain(identity) = module.function_identities[function]
            .source_identity()
            .expect("the function is source-defined")
        else {
            panic!("the function is non-generic")
        };
        let subject = scoop_identity::DefinitionOriginSubject::Function(identity.id());
        let origin = module
            .export_definition_origins
            .get(subject)
            .expect("every source function has one definition origin")
            .origin();
        assert_eq!(origin.source(), &expected_source);
        assert_eq!(
            origin.span(),
            scoop_identity::SourceSpan::new(0, 0).unwrap()
        );
    }
}

#[test]
fn a_later_user_source_does_not_gain_core_intrinsic_authority() {
    let core = core_file();
    let first = file(Vec::new());
    let ast::Decl::Function(mut intrinsic) =
        intrinsic_fun("wipe", "rt_gc_collect", Vec::new(), None)
    else {
        panic!("the intrinsic builder returns a function")
    };
    intrinsic.body = ast::FunctionBody::None;
    let second = file(vec![
        ast::Decl::Function(intrinsic),
        fun("main", Vec::new()),
    ]);

    let errors = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect_err("source position must not grant core intrinsic authority");
    assert!(errors.iter().any(|error| {
        error.file == 2 && error.message == "`@Intrinsic` is only allowed in the core library"
    }));
}

#[test]
fn combined_source_input_rejects_duplicate_semantic_identities() {
    let core = file(Vec::new());
    let duplicate = core_source_identity("src/duplicate.scoop");
    let parsed = identified_test_sources(vec![file(vec![fun("main", Vec::new())])]);
    let error = LegacyCombinedSources::try_new(
        vec![
            source(
                &core,
                duplicate.clone(),
                hir::IntrinsicProviderId::from_raw(17),
                "first-core.scoop",
            ),
            source(
                &core,
                duplicate.clone(),
                hir::IntrinsicProviderId::from_raw(17),
                "second-core.scoop",
            ),
        ],
        hir::IntrinsicProviderId::from_raw(29),
        parsed,
        |_| CurrentSourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
    .err()
    .expect("duplicate identities cannot enter HIR lowering");

    assert_eq!(
        error,
        LegacyCombinedSourcesError::DuplicateSourceIdentity {
            first_index: 0,
            duplicate_index: 1,
            identity: duplicate,
        }
    );
}

#[test]
fn same_qualified_package_shares_internal_declarations_across_sources() {
    let core = core_file();
    let first = with_package(file(vec![fun("shared", Vec::new())]), &["dev", "app"]);
    let second = with_package(
        file(vec![fun("main", vec![stmt(call("shared", Vec::new()))])]),
        &["dev", "app"],
    );

    lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("files in one typed package share internal declarations");
}

#[test]
fn same_package_cross_source_non_overloadable_declaration_is_duplicate() {
    let core = core_file();
    let first = with_package(
        file(vec![struct_decl("Duplicate", Vec::new())]),
        &["shared"],
    );
    let second = with_package(
        file(vec![struct_decl("Duplicate", Vec::new())]),
        &["shared"],
    );

    let errors = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect_err("a non-overloadable declaration is unique across its whole package");
    assert!(
        errors
            .iter()
            .any(|error| { error.file == 2 && error.message == "duplicate struct `Duplicate`" })
    );
}

#[test]
fn different_packages_do_not_share_short_names() {
    let core = core_file();
    let first = with_package(file(vec![fun("hidden", Vec::new())]), &["first"]);
    let second = with_package(
        file(vec![fun("main", vec![stmt(call("hidden", Vec::new()))])]),
        &["second"],
    );

    let errors = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect_err("a short name cannot cross a package boundary");
    assert!(errors.iter().any(|error| {
        error.file == 2 && error.message.starts_with("unknown function `hidden`")
    }));
}

#[test]
fn different_packages_may_declare_the_same_short_name() {
    let core = core_file();
    let first = with_package(file(vec![fun("helper", Vec::new())]), &["first"]);
    let second = with_package(
        file(vec![
            fun("helper", Vec::new()),
            fun("main", vec![stmt(call("helper", Vec::new()))]),
        ]),
        &["second"],
    );

    lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("same-spelled declarations in distinct packages have distinct arena identities");
}

#[test]
fn qualified_type_path_uses_the_longest_typed_package_prefix() {
    let core = core_file();
    let declaration = with_package(file(vec![struct_decl("Value", Vec::new())]), &["a", "b"]);
    let consumer = file(vec![
        fun_sig(
            "consume",
            Vec::new(),
            vec![("value", qualified_type(&["a", "b", "Value"]))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]);

    lower_user_sources(&core, &declaration, &[(&consumer, "root.scoop")])
        .expect("a qualified type path resolves through typed package nodes");
}

#[test]
fn private_same_name_types_are_isolated_by_source_inside_one_package() {
    let core = core_file();
    let mut first_use = fun_sig(
        "firstUse",
        Vec::new(),
        vec![("value", ty_named("Secret"))],
        None,
        Vec::new(),
    );
    set_private(&mut first_use);
    let first = with_package(file(vec![private_struct("Secret"), first_use]), &["shared"]);
    let mut second_use = fun_sig(
        "secondUse",
        Vec::new(),
        vec![("value", ty_named("Secret"))],
        None,
        Vec::new(),
    );
    set_private(&mut second_use);
    let second = with_package(
        file(vec![
            private_struct("Secret"),
            second_use,
            fun("main", Vec::new()),
        ]),
        &["shared"],
    );

    lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("each file-private type owns a source-local binding");
}

#[test]
fn core_bodies_cannot_see_current_package_declarations() {
    let mut core = core_file();
    core.declarations
        .push(fun("coreProbe", vec![stmt(call("userOnly", Vec::new()))]));
    let user = file(vec![fun("userOnly", Vec::new()), fun("main", Vec::new())]);

    let errors = lower_user_sources(&core, &user, &[])
        .expect_err("the current package must not leak into core lookup");
    assert!(errors.iter().any(|error| {
        error.file == 0 && error.message.starts_with("unknown function `userOnly`")
    }));
}

#[test]
fn source_permutation_does_not_change_the_current_package_winner() {
    let core = core_file();
    let first = with_package(
        file(vec![fun_expr(
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )]),
        &["first"],
    );
    let second = with_package(
        file(vec![
            fun_expr(
                "choose",
                Vec::new(),
                Vec::new(),
                Some(ty_named("String")),
                str_lit("second"),
            ),
            fun("main", vec![val("selected", call("choose", Vec::new()))]),
        ]),
        &["second"],
    );

    let forward = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("forward source order lowers");
    let reverse = lower_user_sources(&core, &second, &[(&first, "first.scoop")])
        .expect("reverse source order lowers");
    assert!(hir::dump(&forward.export).contains("Call choose : String"));
    assert!(hir::dump(&reverse.export).contains("Call choose : String"));
}

fn sparse_source_identity(source_key: u32) -> scoop_identity::SourceIdentity {
    let path = match source_key {
        4 => "src/004-four.scoop",
        7 => "src/007-seven.scoop",
        12 => "src/012-twelve.scoop",
        33 => "src/033-thirty-three.scoop",
        41 => "src/041-forty-one.scoop",
        91 => "src/091-ninety-one.scoop",
        97 => "src/097-ninety-seven.scoop",
        99 => "src/099-ninety-nine.scoop",
        _ => panic!("test source key {source_key} needs an explicit logical path"),
    };
    test_source_identity(path)
}

fn validated_sources(
    first: (u32, &ast::SourceFile),
    remaining: &[(u32, &ast::SourceFile)],
) -> ast::AllParsedSources {
    let parsed = |(index, source): (u32, &ast::SourceFile)| {
        ast::IdentifiedParsedSource::new(sparse_source_identity(index), source.clone())
    };
    ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        parsed(first),
        remaining.iter().copied().map(parsed).collect(),
    ))
    .expect("the test supplies distinct source identities")
}

fn lower_sparse_sources(
    core: &ast::SourceFile,
    ordered: &[(u32, &ast::SourceFile)],
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let (&first, remaining) = ordered.split_first().expect("test sources are nonempty");
    let input = LegacyCombinedSources::try_new(
        vec![source(
            core,
            core_source_identity("src/core.scoop"),
            hir::IntrinsicProviderId::from_raw(17),
            "core.scoop",
        )],
        hir::IntrinsicProviderId::from_raw(29),
        validated_sources(first, remaining),
        |_| CurrentSourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

#[test]
fn source_identity_permutation_preserves_the_import_winner() {
    let core = core_file();
    let imported = with_package(
        file(vec![fun_expr(
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("String")),
            str_lit("imported"),
        )]),
        &["api"],
    );
    let mut consumer = with_package(
        file(vec![
            fun_expr(
                "choose",
                Vec::new(),
                Vec::new(),
                Some(ty_named("Int")),
                int_lit(1),
            ),
            fun("main", vec![val("selected", call("choose", Vec::new()))]),
        ]),
        &["app"],
    );
    consumer.imports.push(exact_import(&["api", "choose"]));
    for ordered in [
        vec![(91, &imported), (4, &consumer)],
        vec![(4, &consumer), (91, &imported)],
    ] {
        let output = lower_sparse_sources(&core, &ordered)
            .expect("container order cannot change exact-import precedence");
        assert!(hir::dump(&output.export).contains("Call choose : String"));
    }
}

#[test]
fn source_identity_permutation_preserves_ambiguity_origins() {
    let core = core_file();
    let left = with_package(
        file(vec![struct_decl(
            "Chosen",
            vec![("leftMarker", ty_named("Unit"))],
        )]),
        &["left"],
    );
    let right = with_package(
        file(vec![struct_decl(
            "Chosen",
            vec![("rightMarker", ty_named("Unit"))],
        )]),
        &["right"],
    );
    let mut consumer = file(vec![
        fun_sig(
            "consume",
            Vec::new(),
            vec![("value", ty_named("Chosen"))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]);
    consumer.imports = vec![
        exact_import(&["left", "Chosen"]),
        exact_import(&["right", "Chosen"]),
    ];
    let mut observed = Vec::new();

    for ordered in [
        vec![(41, &left), (7, &right), (99, &consumer)],
        vec![(99, &consumer), (7, &right), (41, &left)],
    ] {
        let errors = lower_sparse_sources(&core, &ordered)
            .expect_err("the two exact type origins remain ambiguous");
        let diagnostic = errors
            .iter()
            .find(|error| error.message.contains("type `Chosen` is ambiguous"))
            .expect("type ambiguity diagnostic");
        assert_eq!(ordered[diagnostic.file - 1].0, 99);
        let mut origins = diagnostic
            .notes
            .iter()
            .map(|note| ordered[note.file - 1].0)
            .collect::<Vec<_>>();
        origins.sort_unstable();
        observed.push(origins);
    }
    assert_eq!(observed, [vec![7, 41], vec![7, 41]]);
}

#[test]
fn source_identity_permutation_preserves_duplicate_signature_origin() {
    let core = core_file();
    let earlier = with_package(
        file(vec![fun_expr(
            "clash",
            Vec::new(),
            vec![("left", ty_named("Int"))],
            Some(ty_named("Int")),
            var("left"),
        )]),
        &["shared"],
    );
    let later = with_package(
        file(vec![
            fun_expr(
                "clash",
                Vec::new(),
                vec![("right", ty_named("Int"))],
                Some(ty_named("Int")),
                var("right"),
            ),
            fun("main", Vec::new()),
        ]),
        &["shared"],
    );
    let mut origins = Vec::new();

    for ordered in [
        vec![(4, &earlier), (97, &later)],
        vec![(97, &later), (4, &earlier)],
    ] {
        let errors = lower_sparse_sources(&core, &ordered)
            .expect_err("same-package normalized signatures are duplicate");
        let duplicate = errors
            .iter()
            .find(|error| {
                error.message == "function `clash` is already declared with the same signature"
            })
            .expect("duplicate-signature diagnostic");
        origins.push(ordered[duplicate.file - 1].0);
    }

    assert_eq!(origins, [97, 97]);
}

#[test]
fn validated_input_retains_source_identities_independently_of_dense_file_indices() {
    let core = core_file();
    let mut private = fun("privateHelper", Vec::new());
    set_private(&mut private);
    let first = with_package(
        file(vec![
            private,
            fun("usePrivate", vec![stmt(call("privateHelper", Vec::new()))]),
        ]),
        &["shared"],
    );
    let second = with_package(file(vec![fun("main", Vec::new())]), &["shared"]);
    let user_provider = hir::IntrinsicProviderId::from_raw(29);
    let expected_source = sparse_source_identity(97);
    let mut private_domains = Vec::new();

    for (parsed, dense_file, locator) in [
        (
            validated_sources((97, &first), &[(4, &second)]),
            1,
            "/one/unrelated.scoop",
        ),
        (
            validated_sources((4, &second), &[(97, &first)]),
            2,
            "/elsewhere/unrelated.scoop",
        ),
    ] {
        let input = LegacyCombinedSources::try_new(
            vec![source(
                &core,
                core_source_identity("src/core.scoop"),
                hir::IntrinsicProviderId::from_raw(17),
                "core.scoop",
            )],
            user_provider,
            parsed,
            |_| CurrentSourceDetails {
                display_locator: locator,
                source_text: "",
            },
        )
        .expect("explicit test source identities are valid");
        let output = lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
            .expect("source order and display locator do not affect private lookup");
        let module = output.export;
        assert_eq!(
            module.source_files[dense_file].identity,
            expected_source.clone()
        );
        assert_eq!(module.source_files[dense_file].name, locator);
        let consumer = module
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == "usePrivate").then_some(function))
            .expect("the private consumer is present");
        let hir::FunctionKind::User(body) = &consumer.kind else {
            panic!("the consumer has a source body")
        };
        let hir::StatementKind::Expr(call) = &body.statements[0].kind else {
            panic!("the consumer starts with the private call")
        };
        assert_eq!(call.origin.definition().file as usize, dense_file);
        let private = module
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == "privateHelper").then_some(function))
            .expect("the private helper is present");
        private_domains.push(private.access.lookup.0.clone());
        assert!(
            private
                .access
                .lookup
                .0
                .constraints()
                .contains(&hir::AccessConstraint::File(expected_source.clone()))
        );
    }
    assert_eq!(private_domains[0], private_domains[1]);
}

#[test]
fn shared_display_locator_does_not_merge_distinct_private_sources() {
    let core = core_file();
    let mut private = fun("privateHelper", Vec::new());
    set_private(&mut private);
    let first = file(vec![private]);
    let second = file(vec![fun(
        "main",
        vec![stmt(call("privateHelper", Vec::new()))],
    )]);
    let input = LegacyCombinedSources::try_new(
        vec![source(
            &core,
            core_source_identity("src/core.scoop"),
            hir::IntrinsicProviderId::from_raw(17),
            "same.scoop",
        )],
        hir::IntrinsicProviderId::from_raw(29),
        validated_sources((12, &first), &[(33, &second)]),
        |_| CurrentSourceDetails {
            display_locator: "same.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");

    let errors = lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
        .expect_err("equal diagnostic labels cannot grant file-private access");
    assert!(errors.iter().any(|error| error.file == 2
        && error.message == "function `privateHelper` is not accessible here"));
}
