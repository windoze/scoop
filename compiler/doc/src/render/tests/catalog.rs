use super::*;

#[test]
fn catalog_groups_overloads_and_keeps_cone_package_and_declaration_identity() {
    let mut first = item("f-first", "parse", ApiKind::Function, None);
    first.comment = Some(comment("First overload."));
    let second = item("f-second", "parse", ApiKind::Function, None);
    let mut separate = item("f-separate", "parse", ApiKind::Function, None);
    separate.package = vec!["sample.part".into()];
    let mut nested_package = item("f-nested", "parse", ApiKind::Function, None);
    nested_package.package = vec!["sample".into(), "part".into()];
    let target = item("c-type", "Box", ApiKind::Class, None);
    let mut use_site = item("c-use", "Consumer", ApiKind::Class, None);
    let mut docs = comment("[parse] and [Box]");
    docs.body.references = vec![
        SymbolReference {
            range: TextRange { start: 0, end: 7 },
            target: LinkTarget::Symbols(vec![first.key.clone(), second.key.clone()]),
        },
        SymbolReference {
            range: TextRange { start: 12, end: 17 },
            target: LinkTarget::Symbols(vec![target.key.clone()]),
        },
    ];
    use_site.comment = Some(docs);
    let inputs = vec![
        cone("a", vec![first, second, separate, nested_package]),
        cone("b", vec![target, use_site]),
    ];
    let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
    let files = render(&catalog);
    let overload = catalog
        .symbol_link(&[SymbolKey("f-first".into()), SymbolKey("f-second".into())])
        .unwrap();
    assert_target_exists(&files, &overload);
    assert_eq!(
        catalog.routes[&SymbolKey("f-first".into())].page,
        catalog.routes[&SymbolKey("f-second".into())].page
    );
    assert_ne!(
        catalog.routes[&SymbolKey("f-separate".into())].page,
        catalog.routes[&SymbolKey("f-nested".into())].page
    );
    let use_page = &catalog.routes[&SymbolKey("c-use".into())].page;
    assert!(files[use_page].contains(&routes::relative(use_page, &overload)));
    assert!(
        files[use_page].contains(&routes::relative(
            use_page,
            &catalog
                .declaration_url(&SymbolKey("c-type".into()))
                .unwrap()
        ))
    );
    for route in catalog.routes.values() {
        assert_target_exists(&files, &route.url());
    }
    for route in catalog.groups.values() {
        assert_target_exists(&files, &route.url());
    }
    let mut reverse = inputs.clone();
    reverse.reverse();
    assert_eq!(
        render(&SiteCatalog::new(&reverse, SiteOptions::default()).unwrap()),
        files
    );
}

#[test]
fn hidden_containers_reexports_and_private_items_follow_the_display_selection() {
    let mut hidden = item("hidden", "Hidden", ApiKind::Class, None);
    hidden.hidden = true;
    let child = item("hidden-child", "member", ApiKind::Function, Some("hidden"));
    let mut private = item("private", "Private", ApiKind::Class, None);
    private.public = false;
    private.visibility = Visibility::Private;
    let mut visible = item("visible", "Visible", ApiKind::Class, None);
    visible.aliases = vec!["find-me".into()];
    let mut docs = comment("[Hidden] [External]");
    docs.body.references = vec![
        SymbolReference {
            range: TextRange { start: 0, end: 8 },
            target: LinkTarget::Symbols(vec![hidden.key.clone()]),
        },
        SymbolReference {
            range: TextRange { start: 9, end: 19 },
            target: LinkTarget::Symbols(vec![SymbolKey("prebuilt".into())]),
        },
    ];
    visible.comment = Some(docs);
    let mut input = cone("a", vec![hidden, child, private, visible]);
    input.bindings = vec![
        ExportBinding {
            package: vec!["alias".into()],
            name: "PublicAlias".into(),
            target: SymbolKey("visible".into()),
            reexport: true,
        },
        ExportBinding {
            package: vec!["alias".into()],
            name: "HiddenAlias".into(),
            target: SymbolKey("hidden".into()),
            reexport: true,
        },
    ];
    for private_items in [false, true] {
        let inputs = [input.clone()];
        let catalog = SiteCatalog::new(
            &inputs,
            SiteOptions {
                document_private_items: private_items,
            },
        )
        .unwrap();
        let files = render(&catalog);
        assert!(
            catalog
                .declaration_url(&SymbolKey("hidden-child".into()))
                .is_none()
        );
        assert_eq!(
            catalog
                .declaration_url(&SymbolKey("private".into()))
                .is_some(),
            private_items
        );
        let rows = search(&files);
        assert!(rows.iter().any(|row| row["name"] == "PublicAlias"));
        assert!(!rows.iter().any(|row| row["name"] == "HiddenAlias"));
        assert!(
            rows.iter()
                .any(|row| row["aliases"] == serde_json::json!(["find-me"]))
        );
        let body = &files[&catalog.routes[&SymbolKey("visible".into())].page];
        assert!(body.contains(">Hidden</span>"));
        assert!(body.contains(">External</span>"));
        assert!(!body.contains("href=\"prebuilt"));
    }
}

#[test]
fn references_to_overloads_from_distinct_declaring_scopes_have_a_complete_group() {
    let first = item("first", "parse", ApiKind::Function, None);
    let mut second = item("second", "parse", ApiKind::Function, None);
    second.package = vec!["other".into()];
    let mut consumer = item("consumer", "use", ApiKind::Function, None);
    let mut docs = comment("[parse]");
    docs.body.references.push(SymbolReference {
        range: TextRange { start: 0, end: 7 },
        target: LinkTarget::Symbols(vec![first.key.clone(), second.key.clone()]),
    });
    consumer.comment = Some(docs);
    let inputs = [cone("a", vec![first, second, consumer])];
    let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
    let files = render(&catalog);
    assert_eq!(catalog.overloads.len(), 1);
    let group = catalog.overloads.values().next().unwrap();
    assert!(files[&group.page].contains("Resolved declarations"));
    for key in ["first", "second"] {
        assert!(
            files[&group.page].contains(
                &routes::relative(
                    &group.page,
                    &catalog.declaration_url(&SymbolKey(key.into())).unwrap()
                )
                .replace('/', "&#x2f;")
            )
        );
    }
}
