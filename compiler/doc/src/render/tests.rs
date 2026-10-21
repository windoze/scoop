use super::*;

mod catalog;
mod content;
mod links;
mod relations;
mod sources;

fn item(key: &str, name: &str, kind: ApiKind, parent: Option<&str>) -> ApiItem {
    ApiItem {
        key: SymbolKey(key.into()),
        alternate_keys: Vec::new(),
        parent: parent.map(|parent| SymbolKey(parent.into())),
        package: vec!["sample".into()],
        kind,
        name: name.into(),
        visibility: Visibility::Public,
        public: true,
        hidden: false,
        aliases: Vec::new(),
        synthesized: false,
        signature: vec![SignatureToken {
            text: format!("public {name}"),
            link: None,
        }],
        receiver: None,
        equality_requirements: None,
        location: SourceLocation {
            source: SourceKey("source".into()),
            range: TextRange { start: 0, end: 1 },
            line: 1,
        },
        comment: None,
        property: None,
        relations: Vec::new(),
    }
}

fn cone(key: &str, items: Vec<ApiItem>) -> ConeInput {
    ConeInput {
        key: ConeKey(key.into()),
        name: format!("test:{key}:1.0.0"),
        executable: false,
        sources: vec![SourceInput {
            key: SourceKey("source".into()),
            path: "src/用户.scoop".into(),
            display_name: "src/用户.scoop".into(),
            text: "// <script>alert('source')</script>\r\npublic class Box\r\n".into(),
        }],
        items,
        bindings: Vec::new(),
        warnings: Vec::new(),
    }
}

fn comment(text: &str) -> Comment {
    Comment {
        body: Markdown {
            text: text.into(),
            definitions: Vec::new(),
            references: Vec::new(),
        },
        tags: Vec::new(),
    }
}

fn render(catalog: &SiteCatalog<'_>) -> BTreeMap<String, String> {
    let renderer = SiteRenderer::new(catalog).unwrap();
    let mut files = renderer.render_shared().unwrap();
    for key in catalog.cones.keys() {
        files.extend(renderer.render_cone(key, &ConeOverview::default()).unwrap());
    }
    files
        .into_iter()
        .map(|file| (file.path, String::from_utf8(file.contents).unwrap()))
        .collect()
}

fn search(files: &BTreeMap<String, String>) -> Vec<serde_json::Value> {
    serde_json::from_str(
        files["assets/index.js"]
            .strip_prefix("globalThis.SCOOP_DOC_SEARCH=")
            .unwrap()
            .strip_suffix(";\n")
            .unwrap(),
    )
    .unwrap()
}

fn assert_target_exists(files: &BTreeMap<String, String>, url: &str) {
    let (page, anchor) = url
        .split_once('#')
        .map_or((url, None), |(page, anchor)| (page, Some(anchor)));
    let html = files
        .get(page)
        .unwrap_or_else(|| panic!("missing page: {page}"));
    if let Some(anchor) = anchor {
        assert!(
            html.contains(&format!("id=\"{anchor}\"")),
            "missing anchor {url}"
        );
    }
}
