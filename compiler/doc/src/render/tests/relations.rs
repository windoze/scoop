use super::*;

#[test]
fn inheritance_links_keep_substitution_origin_and_missing_dependency_text() {
    let base = item("base", "Base", ApiKind::Class, None);
    let member = item("member", "read", ApiKind::Function, Some("base"));
    let mut child = item("child", "Child", ApiKind::Class, None);
    child.relations = vec![ApiRelation {
        kind: RelationKind::Inherited,
        target: member.key.clone(),
        signature: "fun read(value: Int): Int".into(),
        origin: "Base<Int>".into(),
        public: true,
    }];
    let inputs = [
        cone("consumer", vec![child.clone()]),
        cone("provider", vec![base, member]),
    ];
    let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
    let files = render(&catalog);
    let page = &catalog.routes[&child.key].page;
    assert!(files[page].contains("fun read(value: Int): Int"));
    assert!(files[page].contains("Base&lt;Int&gt;"));
    let origin = catalog
        .declaration_url(&SymbolKey("member".into()))
        .unwrap();
    assert!(
        files[page].contains(&routes::relative(page, &origin).replace('/', "&#x2f;")),
        "{}",
        files[page]
    );
    assert_target_exists(&files, &origin);
    let without = [inputs[0].clone()];
    let no_deps = SiteCatalog::new(&without, SiteOptions::default()).unwrap();
    let unavailable = render(&no_deps);
    assert!(unavailable[page].contains("no generated documentation"));
    assert!(unavailable[page].contains("fun read(value: Int): Int"));
    assert_ne!(
        serde_json::to_value(catalog.link_inputs(&inputs[0].key).unwrap()).unwrap(),
        serde_json::to_value(no_deps.link_inputs(&inputs[0].key).unwrap()).unwrap()
    );
}
