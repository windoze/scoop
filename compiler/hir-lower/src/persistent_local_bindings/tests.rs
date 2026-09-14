use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    BindableEntity, BindingRole, CanonicalIdentifier, LocalBindingKey, LocalBindingRole,
    PackagePath, PersistentLocalBindingId,
};

use crate::tests::{
    core_file, core_source_identity, enum_decl, file, fun, fun_expr, ident,
    identified_test_sources, int_lit, sp, test_source_identity, ty_named, variant_unit,
};

fn path(parts: &[&str]) -> ast::QualifiedNameSyntax {
    ast::QualifiedNameSyntax {
        first: ident(parts[0]),
        rest: parts[1..]
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
        path: path(&[name]),
        span: sp(),
    };
    source
}

fn exact(parts: &[&str], alias: Option<&str>, start: u32) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: path(parts),
        alias: alias.map(|name| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(name),
            span: sp(),
        }),
        import_keyword_span: sp(),
        span: ast::Span::new(start, start + 5),
    }
}

fn star(parts: &[&str], start: u32) -> ast::ImportSyntax {
    ast::ImportSyntax::Star {
        exposure: ast::ImportExposureSyntax::Local,
        namespace: path(parts),
        import_keyword_span: sp(),
        terminal_dot_span: sp(),
        star_span: sp(),
        span: ast::Span::new(start, start + 5),
    }
}

fn object(name: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn alias(name: &str) -> ast::Decl {
    ast::Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target: ty_named("Int"),
        span: sp(),
    })
}

fn publisher(unrelated_prefix: bool) -> ast::SourceFile {
    let mut declarations = vec![
        object("Registry"),
        fun_expr(
            "ping",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        alias("Number"),
        enum_decl("Status", Vec::new(), vec![variant_unit("Ready")]),
    ];
    if unrelated_prefix {
        declarations.insert(0, fun("unrelated", Vec::new()));
    }
    package(file(declarations), "api")
}

fn consumer() -> ast::SourceFile {
    let mut source = package(file(Vec::new()), "app");
    source.imports = vec![
        exact(&["api", "ping"], None, 40),
        exact(&["api", "ping"], None, 20),
        exact(&["api", "ping"], Some("pong"), 30),
        exact(&["api", "Number"], Some("Count"), 50),
        exact(&["api", "Status", "Ready"], None, 60),
        star(&["api"], 70),
        star(&["api", "Status"], 80),
    ];
    source
}

fn lower(unrelated_prefix: bool) -> hir::Output {
    let core = core_file();
    let parsed = identified_test_sources(vec![publisher(unrelated_prefix), consumer()]);
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "/checkout/source.scoop",
            source_text: "",
        },
    )
    .expect("local binding test sources are valid");
    crate::lower_combined_sources(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("local binding fixture lowers")
}

fn records(
    output: &hir::Output,
) -> Vec<(
    PersistentLocalBindingId,
    LocalBindingKey,
    scoop_identity::DefinitionOrigin,
)> {
    output
        .export
        .local_binding_identities
        .iter()
        .map(|identity| {
            (
                identity.record().id(),
                identity.record().key().clone(),
                identity.origin().clone(),
            )
        })
        .collect()
}

#[test]
fn declaration_and_import_bindings_keep_typed_roles_and_canonical_origins() {
    let output = lower(false);
    let records = records(&output);
    let publisher = test_source_identity("src/first.scoop");
    let consumer = test_source_identity("src/second.scoop");
    let api = PackagePath::from_segments(vec![CanonicalIdentifier::new("api").unwrap()]);
    let app = PackagePath::from_segments(vec![CanonicalIdentifier::new("app").unwrap()]);

    let declarations = records
        .iter()
        .filter(|(_, key, _)| key.source() == &publisher)
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 6);
    assert!(declarations.iter().all(|(_, key, _)| {
        key.package() == &api && key.source_role() == LocalBindingRole::Declaration
    }));
    assert_eq!(
        declarations
            .iter()
            .filter(|(_, key, _)| key.local_name().as_str() == "Registry")
            .count(),
        2
    );
    assert!(declarations.iter().any(|(_, key, _)| {
        key.local_name().as_str() == "Registry"
            && key.binding_role() == BindingRole::TypeName
            && matches!(
                key.target(),
                BindableEntity::Type(_) | BindableEntity::GenericType(_)
            )
    }));
    assert!(declarations.iter().any(|(_, key, _)| {
        key.local_name().as_str() == "Registry"
            && key.binding_role() == BindingRole::ObjectValue
            && matches!(key.target(), BindableEntity::ObjectValue(_))
    }));

    let imports = records
        .iter()
        .filter(|(_, key, _)| key.source() == &consumer)
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 10);
    assert!(imports.iter().all(|(_, key, _)| key.package() == &app));

    let exact_ping = imports
        .iter()
        .find(|(_, key, _)| {
            key.local_name().as_str() == "ping"
                && key.source_role() == LocalBindingRole::ExactImport
        })
        .expect("deduplicated exact ping binding");
    assert_eq!(exact_ping.2.span().start_byte(), 20);
    assert_eq!(exact_ping.2.span().end_byte(), 25);
    assert_eq!(
        imports
            .iter()
            .filter(|(_, key, _)| {
                key.local_name().as_str() == "ping"
                    && key.source_role() == LocalBindingRole::ExactImport
            })
            .count(),
        1
    );
    assert!(imports.iter().any(|(_, key, _)| {
        key.local_name().as_str() == "pong"
            && key.source_role() == LocalBindingRole::AliasImport
            && key.binding_role() == BindingRole::Function
    }));
    assert!(imports.iter().any(|(_, key, _)| {
        key.local_name().as_str() == "Count"
            && key.source_role() == LocalBindingRole::AliasImport
            && key.binding_role() == BindingRole::TypeAlias
            && matches!(key.target(), BindableEntity::TypeAlias(_))
    }));
    assert!(imports.iter().any(|(_, key, _)| {
        key.local_name().as_str() == "Ready"
            && key.source_role() == LocalBindingRole::StarImport
            && key.binding_role() == BindingRole::EnumVariant
    }));
}

#[test]
fn unrelated_declaration_does_not_renumber_existing_bindings() {
    let baseline = records(&lower(false));
    let mut with_prefix = records(&lower(true));
    with_prefix.retain(|(_, key, _)| key.local_name().as_str() != "unrelated");

    assert_eq!(baseline, with_prefix);
}
