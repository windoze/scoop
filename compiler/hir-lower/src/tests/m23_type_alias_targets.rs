use super::*;

fn type_alias(name: &str, target: TypeRef) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target,
        span: sp(),
    })
}

fn lowered_alias<'module>(
    module: &'module hir::Module,
    name: &str,
) -> (hir::ExportTypeAliasId, &'module hir::TypeAliasDecl) {
    module
        .type_aliases
        .iter()
        .find(|(_, declaration)| declaration.name == name)
        .unwrap_or_else(|| panic!("missing lowered typealias `{name}`"))
}

fn path(segments: &[&str]) -> ast::QualifiedNameSyntax {
    let first = ident(segments[0]);
    let rest = segments[1..]
        .iter()
        .map(|segment| ast::QualifiedNameTailSyntax {
            dot_span: sp(),
            identifier: ident(segment),
        })
        .collect();
    ast::QualifiedNameSyntax {
        first,
        rest,
        span: sp(),
    }
}

fn package(mut source: ast::SourceFile, segments: &[&str]) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: path(segments),
        span: sp(),
    };
    source
}

fn exact_import(segments: &[&str], alias: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: path(segments),
        alias: Some(ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(alias),
            span: sp(),
        }),
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn lower_current_sources(
    sources: Vec<ast::SourceFile>,
) -> Result<hir::ExportHirOutput, Vec<ast::Diagnostic>> {
    let core = core_file();
    let parsed = identified_test_sources(sources);
    let input = crate::DefinedTestSources::try_new(
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
            display_locator: "test.scoop",
            source_text: "",
        },
    )
    .expect("explicit test source identities are valid");
    crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .map(|output| output.export)
}

#[test]
fn export_hir_preserves_only_the_direct_outer_alias_edge() {
    let module = lower_user(file(vec![
        type_alias("Direct", ty_named("Leaf")),
        type_alias("Composite", ty_generic("Box", vec![ty_named("Leaf")])),
        type_alias("Leaf", ty_named("Int")),
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        fun("main", Vec::new()),
    ]))
    .expect("forward alias edges and aliases nested in applications must lower");

    let (_, direct) = lowered_alias(&module, "Direct");
    let (_, composite) = lowered_alias(&module, "Composite");
    let (leaf_id, leaf) = lowered_alias(&module, "Leaf");

    assert_eq!(direct.target, leaf.target);
    assert_eq!(
        direct.source_target,
        hir::TypeAliasSourceTarget::Alias(leaf_id)
    );
    assert_eq!(
        composite.source_target,
        hir::TypeAliasSourceTarget::Expanded
    );
    assert_eq!(leaf.source_target, hir::TypeAliasSourceTarget::Expanded);
    assert_eq!(hir::type_name(&module, composite.target), "Box<Int>");
}

#[test]
fn exact_import_rename_preserves_the_selected_alias_identity() {
    let provider = package(
        file(vec![type_alias("ProviderAlias", ty_named("Int"))]),
        &["api"],
    );
    let mut consumer = file(vec![type_alias("Facade", ty_named("Renamed"))]);
    consumer.imports = vec![exact_import(&["api", "ProviderAlias"], "Renamed")];

    let module = lower_current_sources(vec![provider, consumer])
        .expect("an exact-import alias can name a typealias target");
    let (provider_id, provider) = lowered_alias(&module, "ProviderAlias");
    let (_, facade) = lowered_alias(&module, "Facade");

    assert_eq!(provider.source_target, hir::TypeAliasSourceTarget::Expanded);
    assert_eq!(facade.target, provider.target);
    assert_eq!(
        facade.source_target,
        hir::TypeAliasSourceTarget::Alias(provider_id)
    );
}
