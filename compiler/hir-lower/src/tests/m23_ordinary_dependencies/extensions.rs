use super::*;

#[test]
fn dependency_extension_functions_support_explicit_and_implicit_receivers() {
    let provider = extension_provider(
        "extension-provider",
        extension_expr(
            ty_named("Int"),
            "identity",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            this_expr(),
        ),
        &["Int"],
        61,
    );
    let mut consumer = file(vec![
        fun_expr(
            "explicit",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            method_call(int_lit(7), "identity", Vec::new()),
        ),
        extension_expr(
            ty_named("Int"),
            "implicit",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            call("identity", Vec::new()),
        ),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "identity"]));

    inspect_extensions(vec![provider], consumer, |output| {
        let output = output.expect("core-closed dependency extensions must lower");
        assert_eq!(output.imported_dependencies().callable_count(), 1);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("Call external").count(), 2, "{dump}");
    });
}

#[test]
fn dependency_extension_operator_is_visible_from_every_scope_layer() {
    for (scope, fingerprint) in [
        (ExtensionScope::Exact, 68),
        (ExtensionScope::CurrentPackage, 69),
        (ExtensionScope::Star, 70),
    ] {
        let provider = extension_provider(
            "operator-extension-provider",
            operator(extension_expr(
                ty_named("Int"),
                "plus",
                Vec::new(),
                vec![("other", ty_named("Boolean"))],
                Some(ty_named("Int")),
                this_expr(),
            )),
            &["Int", "Boolean"],
            fingerprint,
        );
        let mut consumer = file(vec![fun_expr(
            "fallback",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            binary(scoop_ast::BinOp::Add, int_lit(1), bool_lit(true)),
        )]);
        consumer.declarations.push(fun_expr(
            "member",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            binary(scoop_ast::BinOp::Add, int_lit(1), int_lit(2)),
        ));
        match scope {
            ExtensionScope::Exact => {
                let mut import = exact_import(&["dependency", "api", "plus"]);
                let scoop_ast::ImportSyntax::Exact { alias, .. } = &mut import else {
                    unreachable!("exact_import builds an exact import")
                };
                *alias = Some(scoop_ast::ImportAliasSyntax {
                    as_keyword_span: sp(),
                    name: ident("renamedPlus"),
                    span: sp(),
                });
                consumer.imports.push(import);
            }
            ExtensionScope::CurrentPackage => {
                consumer = in_package(consumer, &["dependency", "api"]);
            }
            ExtensionScope::Star => consumer.imports.push(star_import(&["dependency", "api"])),
        }

        inspect_extensions(vec![provider], consumer, |output| {
            let output = output.expect("dependency operators are selected by typed role");
            assert_eq!(output.imported_dependencies().callable_count(), 1);
            let dump = scoop_hir::dump(&output.output().export);
            assert_eq!(dump.matches("Call external").count(), 1, "{dump}");
            assert!(dump.contains("IntegerOperation int.add <no-gc>"), "{dump}");
        });
    }
}

#[test]
fn dependency_extension_get_and_set_use_typed_operator_roles() {
    let get = extension_provider(
        "get-extension-provider",
        operator(extension_expr(
            ty_named("Int"),
            "get",
            Vec::new(),
            vec![("index", ty_named("Int"))],
            Some(ty_named("Int")),
            this_expr(),
        )),
        &["Int"],
        71,
    );
    let set = extension_provider(
        "set-extension-provider",
        operator(extension_expr(
            ty_named("Int"),
            "set",
            Vec::new(),
            vec![("index", ty_named("Int")), ("value", ty_named("Int"))],
            None,
            unit_lit(),
        )),
        &["Int"],
        72,
    );
    let mut consumer = file(vec![fun(
        "consumer",
        vec![
            val("read", subscript(int_lit(1), int_lit(2))),
            assign_index(int_lit(1), int_lit(2), int_lit(3)),
        ],
    )]);
    consumer.imports.extend([
        exact_import(&["dependency", "api", "get"]),
        exact_import(&["dependency", "api", "set"]),
    ]);

    inspect_extensions(vec![get, set], consumer, |output| {
        let output = output.expect("dependency get/set operators lower through their typed roles");
        assert_eq!(output.imported_dependencies().callable_count(), 2);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("Call external").count(), 2, "{dump}");
    });
}

#[test]
fn dependency_extension_invoke_applies_to_a_current_property_value() {
    let provider = extension_provider(
        "invoke-extension-provider",
        operator(extension_expr(
            ty_named("Int"),
            "invoke",
            Vec::new(),
            vec![("flag", ty_named("Boolean"))],
            Some(ty_named("Int")),
            this_expr(),
        )),
        &["Boolean", "Int"],
        67,
    );
    let mut consumer = file(vec![
        computed_int_property("choose", 7),
        fun_expr(
            "consumer",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            call("choose", vec![bool_lit(true)]),
        ),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "invoke"]));

    inspect_extensions(vec![provider], consumer, |output| {
        let output = output.expect("dependency extension invoke applies after the property read");
        assert_eq!(output.imported_dependencies().callable_count(), 1);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("Call external").count(), 1, "{dump}");
    });
}

#[test]
fn inapplicable_exact_dependency_extension_falls_through_to_current_package() {
    let provider = extension_provider(
        "string-extension-provider",
        extension_expr(
            ty_named("String"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        &["Int", "String"],
        62,
    );
    let mut consumer = file(vec![
        extension_expr(
            ty_named("Int"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun_expr(
            "consumer",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            method_call(int_lit(1), "choose", Vec::new()),
        ),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "choose"]));

    inspect_extensions(vec![provider], consumer, |output| {
        let output = output.expect("an inapplicable exact extension must not shadow current code");
        assert!(output.imported_dependencies().is_empty());
        let dump = scoop_hir::dump(&output.output().export);
        assert!(!dump.contains("Call external"), "{dump}");
    });
}

#[test]
fn exact_dependency_extension_result_does_not_select_a_lower_scope() {
    let provider = extension_provider(
        "layout-extension-provider",
        extension_expr(
            ty_named("Int"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_tuple(vec![ty_named("Int"), ty_named("Int")])),
            tuple_lit(vec![int_lit(1), int_lit(2)]),
        ),
        &["Int"],
        63,
    );
    let mut consumer = file(vec![
        extension_expr(
            ty_named("Int"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun_expr(
            "consumer",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            method_call(int_lit(1), "choose", Vec::new()),
        ),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "choose"]));

    inspect_extensions(vec![provider], consumer, |output| {
        let errors = output
            .err()
            .expect("the selected exact-import candidate retains its result type");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].message,
            "body of `consumer` must be of type Int, found (Int, Int)"
        );
        assert_eq!(errors[0].span, Some(sp()));
    });
}

#[test]
fn dependency_extensions_share_one_msc_partition_with_their_exact_layer() {
    let broad = extension_provider(
        "broad-extension-provider",
        extension_expr(
            ty_named("Any"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        &["Int"],
        64,
    );
    let exact = extension_provider(
        "exact-extension-provider",
        extension_expr(
            ty_named("Int"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        ),
        &["Int"],
        65,
    );
    let expected_provider = exact.coordinate.identity().unwrap();
    let mut consumer = file(vec![fun_expr(
        "consumer",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        method_call(int_lit(1), "choose", Vec::new()),
    )]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "choose"]));

    inspect_extensions(vec![broad, exact], consumer, |output| {
        let output = output.expect("MSC must choose the narrower dependency receiver");
        let selected = output
            .imported_dependencies()
            .callables()
            .next()
            .expect("one extension callable is selected");
        assert_eq!(selected.provider(), expected_provider);
        assert_eq!(output.imported_dependencies().callable_count(), 1);
    });
}

#[test]
fn current_and_dependency_extensions_share_the_split_package_msc_partition() {
    let dependency = extension_provider(
        "split-extension-provider",
        extension_expr(
            ty_named("Int"),
            "choose",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        ),
        &["Int"],
        66,
    );
    let consumer = in_package(
        file(vec![
            extension_expr(
                ty_named("Any"),
                "choose",
                Vec::new(),
                Vec::new(),
                Some(ty_named("Int")),
                int_lit(1),
            ),
            fun_expr(
                "consumer",
                Vec::new(),
                Vec::new(),
                Some(ty_named("Int")),
                method_call(int_lit(1), "choose", Vec::new()),
            ),
        ]),
        &["dependency", "api"],
    );
    let expected_provider = dependency.coordinate.identity().unwrap();

    inspect_extensions(vec![dependency], consumer, |output| {
        let output = output.expect("split-package extensions share one MSC partition");
        let selected = output
            .imported_dependencies()
            .callables()
            .next()
            .expect("the narrower dependency extension is selected");
        assert_eq!(selected.provider(), expected_provider);
        assert_eq!(output.imported_dependencies().callable_count(), 1);
    });
}

struct DependencyExtensionProvider {
    coordinate: scoop_identity::ConeCoordinate,
    declaration: Decl,
    core_types: &'static [&'static str],
    fingerprint: u8,
}

#[derive(Clone, Copy)]
enum ExtensionScope {
    Exact,
    CurrentPackage,
    Star,
}

fn extension_provider(
    name: &str,
    declaration: Decl,
    core_types: &'static [&'static str],
    fingerprint: u8,
) -> DependencyExtensionProvider {
    DependencyExtensionProvider {
        coordinate: scoop_identity::ConeCoordinate::new("test", name, "1.0.0").unwrap(),
        declaration,
        core_types,
        fingerprint,
    }
}

fn inspect_extensions<R>(
    providers: Vec<DependencyExtensionProvider>,
    consumer: scoop_ast::SourceFile,
    inspect: impl FnOnce(Result<scoop_hir::DependencyHirOutput, Vec<scoop_ast::Diagnostic>>) -> R,
) -> R {
    let mut core = trusted_core();
    let prepared = providers
        .into_iter()
        .map(|provider| {
            let DependencyExtensionProvider {
                coordinate,
                declaration,
                core_types,
                fingerprint,
            } = provider;
            let mut source = file(vec![declaration]);
            make_core_public(&mut source);
            source.package = scoop_ast::PackageSyntax::QualifiedPackage {
                package_keyword_span: sp(),
                path: qualified(&["dependency", "api"]),
                span: sp(),
            };
            let (foundation, interface) =
                project_dependency_without_default_roles(&core, &coordinate, source, core_types);
            let foundation =
                core.import_dependency_foundation(&coordinate, &foundation, fingerprint);
            (foundation, interface)
        })
        .collect::<Vec<_>>();
    let aliases = empty_alias_expansions();
    let ordinary = parsed_ordinary(consumer);
    let direct: Vec<_> = prepared
        .iter()
        .map(|(foundation, interface)| scoop_hir::ImportedProviderInput {
            foundation,
            interface,
            alias_expansions: &aliases,
        })
        .collect();
    let world = scoop_hir::ImportedSemanticWorld::from_dependencies(
        ordinary.cone(),
        std::iter::once(core.provider()).chain(direct).collect(),
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    inspect(lower_current_cone(
        scoop_identity::RequestedConeKind::Library,
        &input,
    ))
}

fn qualified(parts: &[&str]) -> scoop_ast::QualifiedNameSyntax {
    let (first, rest) = parts.split_first().expect("qualified names are nonempty");
    scoop_ast::QualifiedNameSyntax {
        first: ident(first),
        rest: rest
            .iter()
            .map(|part| scoop_ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(part),
            })
            .collect(),
        span: sp(),
    }
}

fn star_import(parts: &[&str]) -> scoop_ast::ImportSyntax {
    scoop_ast::ImportSyntax::Star {
        exposure: scoop_ast::ImportExposureSyntax::Local,
        namespace: qualified(parts),
        import_keyword_span: sp(),
        terminal_dot_span: sp(),
        star_span: sp(),
        span: sp(),
    }
}

fn operator(mut declaration: Decl) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("the test operator declaration is a function")
    };
    function.operator = Some(scoop_ast::OperatorModifier { span: sp() });
    declaration
}

fn computed_int_property(name: &str, value: i64) -> Decl {
    Decl::Global(scoop_ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: scoop_ast::VisibilitySyntax::Omitted,
        modifier: scoop_ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: scoop_ast::PropertyBodySyntax::Computed(scoop_ast::AccessorSyntax {
            getter: Some(scoop_ast::GetterDecl {
                annotations: Vec::new(),
                body: scoop_ast::AccessorBodySyntax::Expr(Box::new(int_lit(value))),
                span: sp(),
            }),
            setter: None,
        }),
        span: sp(),
    })
}
