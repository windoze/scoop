use super::*;

#[test]
fn html_escapes_source_signatures_markdown_and_renders_property_capabilities() {
    let mut owner = item("box", "Box<\"T\">", ApiKind::Class, None);
    owner.aliases = vec!["holder".into()];
    owner.comment = Some(comment("A **box**.\n\n<script>alert('docs')</script>"));
    let mut property = item("value", "value", ApiKind::Property, Some("box"));
    property.signature = vec![SignatureToken {
        text: "var value: T<Other>".into(),
        link: None,
    }];
    property.property = Some(PropertyCapability {
        getter: AccessorCapability {
            key: SymbolKey("get".into()),
            visibility: Visibility::Public,
            hidden: false,
        },
        setter: Some(AccessorCapability {
            key: SymbolKey("set".into()),
            visibility: Visibility::Private,
            hidden: false,
        }),
    });
    let mut parameter = item("parameter", "T", ApiKind::TypeParameter, Some("box"));
    parameter.comment = Some(comment("Element type."));
    let mut equality = item("equals", "equals", ApiKind::Function, Some("box"));
    equality.equality_requirements = Some(EqualityRequirements {
        components: vec![SignatureToken {
            text: "T".into(),
            link: Some(parameter.key.clone()),
        }],
    });
    let mut extension = item("extension", "describe", ApiKind::Function, None);
    extension.receiver = Some(SignatureToken {
        text: owner.name.clone(),
        link: Some(owner.key.clone()),
    });
    let inputs = [cone(
        "a",
        vec![owner, property, parameter, equality, extension],
    )];
    let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
    let files = render(&catalog);
    let page = &catalog.routes[&SymbolKey("box".into())].page;
    let html = &files[page];
    assert!(!html.contains("<script>alert"));
    assert!(!html.contains("Box<\"T\">"));
    assert!(html.contains("<strong>box</strong>"));
    assert!(html.contains("get: public"));
    assert!(html.contains("set: private"));
    assert!(html.contains("Element type."));
    assert!(html.contains("same-type equality"));
    assert!(html.contains("<h3>Extensions</h3>"));
    let extension_page = &catalog.routes[&SymbolKey("extension".into())].page;
    assert!(files[extension_page].contains("Extension receiver:"));
    for key in ["box", "value", "get", "set", "parameter"] {
        assert_target_exists(
            &files,
            &catalog.declaration_url(&SymbolKey(key.into())).unwrap(),
        );
    }
    let source = &files[&routes::source(&inputs[0].key, &inputs[0].sources[0].key)];
    assert!(!source.contains("<script>alert"));
    assert!(source.contains("id=\"L2\""));
    assert!(source.contains("src&#x2f;用户.scoop"));
    if let Some(directory) = std::env::var_os("SCOOP_DOC_RENDER_PREVIEW") {
        for (path, contents) in &files {
            let path = std::path::Path::new(&directory).join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }
}

#[test]
fn markdown_urls_raw_html_definitions_and_readme_resources_have_distinct_behavior() {
    let inputs = [];
    let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
    let mut docs = comment("[ok][ref] [bad](JaVaScRiPt:alert%281%29) <b>literal</b> `code`");
    docs.body
        .definitions
        .push("[ref]: https://example.org/api?q=1&x=2".into());
    let html = markdown::render(&docs.body, "index.html", &catalog);
    assert!(html.contains("https://example.org/api?q=1&amp;x=2"));
    assert!(!html.contains("href=\"JaVaScRiPt"));
    assert!(!html.contains("<b>literal</b>"));
    assert!(html.contains("<code>code</code>"));
    let readme = ReadmeInput {
        text: "![diagram](chart.svg) [notes](notes.txt) [missing](gone.txt) [Undefined]".into(),
        resources: [
            ("chart.svg".into(), Some("c/a/resources/chart.svg".into())),
            ("notes.txt".into(), Some("c/a/resources/notes.txt".into())),
            ("gone.txt".into(), None),
        ]
        .into(),
    };
    let html = markdown::readme(&readme, "c/a/index.html");
    assert!(html.contains("src=\"resources/chart.svg\""));
    assert!(html.contains("href=\"resources/notes.txt\""));
    assert!(!html.contains("href=\"gone.txt\""));
    assert!(html.contains("[Undefined]"));
    for url in [
        "javascript:alert(1)",
        "java\nscript:alert(1)",
        "data:text/html,a",
        "vbscript:foo",
    ] {
        assert!(markdown::safe_url(url).is_none());
    }
}
