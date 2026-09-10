use scoop_ast as ast;
use scoop_hir as hir;

use crate::tests::{
    core_file, core_source_identity, file, ident, sp, test_source_identity, ty_named,
};

fn alias(name: &str, target: &str, private: bool) -> ast::Decl {
    ast::Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: if private {
            ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Private,
                span: sp(),
            }
        } else {
            ast::VisibilitySyntax::Omitted
        },
        name: ident(name),
        target: ty_named(target),
        span: sp(),
    })
}

fn package(mut source: ast::SourceFile, name: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: ast::QualifiedNameSyntax {
            first: ident(name),
            rest: Vec::new(),
            span: sp(),
        },
        span: sp(),
    };
    source
}

fn identity(
    path: &str,
    package_name: &str,
    target: &str,
    private: bool,
    locator: &str,
) -> scoop_identity::PersistentTypeAliasId {
    let source = package(file(vec![alias("Alias", target, private)]), package_name);
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity(path), source),
        Vec::new(),
    ))
    .unwrap();
    let core = core_file();
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
            display_locator: locator,
            source_text: "",
        },
    )
    .unwrap();
    let output =
        crate::lower_legacy_combined_sources(&input, crate::IntrinsicDeclarationPolicy::CoreOnly)
            .expect("the type-alias fixture lowers");
    let (id, _) = output
        .export
        .type_aliases
        .iter()
        .find(|(_, alias)| alias.name == "Alias")
        .unwrap();
    output.export.type_alias_identities[id].id()
}

#[test]
fn identity_uses_package_and_private_source_but_not_target_or_locator() {
    let base = identity(
        "src/first.scoop",
        "models",
        "Int",
        false,
        "/old/tree/alias.scoop",
    );
    assert_eq!(
        base,
        identity(
            "src/renamed.scoop",
            "models",
            "String",
            false,
            "/new/tree/moved.scoop",
        )
    );
    assert_ne!(
        base,
        identity(
            "src/first.scoop",
            "other",
            "Int",
            false,
            "/old/tree/alias.scoop",
        )
    );

    let private = identity(
        "src/first.scoop",
        "models",
        "Int",
        true,
        "/old/tree/alias.scoop",
    );
    assert_ne!(
        private,
        identity(
            "src/second.scoop",
            "models",
            "Int",
            true,
            "/old/tree/alias.scoop",
        )
    );
}
