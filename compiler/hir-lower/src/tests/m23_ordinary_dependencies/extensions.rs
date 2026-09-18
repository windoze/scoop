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
        assert_eq!(dump.matches("ImportedDependencyCall").count(), 2, "{dump}");
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
        assert!(!dump.contains("ImportedDependencyCall"), "{dump}");
    });
}

#[test]
fn unsupported_exact_dependency_extension_falls_through_to_current_package() {
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
        let output = output.expect("an unsupported exact extension must permit lower fallback");
        assert!(output.imported_dependencies().is_empty());
        let dump = scoop_hir::dump(&output.output().export);
        assert!(!dump.contains("ImportedDependencyCall"), "{dump}");
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
    inspect: impl FnOnce(Result<scoop_hir::OrdinaryHirOutput<'_>, Vec<scoop_ast::Diagnostic>>) -> R,
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
            (coordinate, fingerprint, foundation, interface)
        })
        .collect::<Vec<_>>();
    let aliases = empty_alias_expansions();
    let core_interface = empty_interface();
    let ordinary = parsed_ordinary(consumer);
    let direct = prepared
        .iter()
        .map(|(coordinate, fingerprint, foundation, interface)| {
            scoop_hir::DirectImportedProviderInput::from_validated(
                certificate(coordinate, *fingerprint),
                foundation,
                interface,
                &aliases,
            )
        })
        .collect();
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        ordinary.cone(),
        Some(scoop_hir::TrustedCoreImportedProviderInput::from_validated(
            certificate(&scoop_identity::ConeCoordinate::reserved_core(), 41),
            &core.foundation,
            &core_interface,
            &aliases,
        )),
        direct,
        Vec::new(),
    )
    .unwrap();
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    inspect(lower_ordinary(
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
