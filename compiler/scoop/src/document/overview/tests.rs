use super::*;

fn source(root: &Path) -> plan::Source {
    plan::Source {
        identity: ConeIdentity::CORE,
        key: String::new(),
        root: root.to_owned(),
        sources: Vec::new(),
        dependencies: Vec::new(),
    }
}

#[test]
fn readme_resources_follow_actual_file_states_and_encoded_urls() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = source(root);
    let cache = ArtifactCacheRoot::new(root.join("cache")).unwrap();
    let readme = "![Diagram](picture%20one.svg#shape)\n[Later](later.txt)\n[Web](https://example.test/image.png)\n";
    std::fs::write(root.join("README.md"), readme).unwrap();
    std::fs::write(root.join("picture one.svg"), "<svg/>").unwrap();
    let first = select(&source, &cache).unwrap();
    assert_eq!(first.warnings.len(), 1);
    assert_eq!(first.copies.len(), 1);
    let resources = &first.view.readme.as_ref().unwrap().resources;
    assert!(
        resources["picture%20one.svg#shape"]
            .as_ref()
            .unwrap()
            .ends_with(".svg#shape")
    );
    assert_eq!(resources["later.txt"], None);
    assert!(!resources.contains_key("https://example.test/image.png"));
    assert_eq!(first.key, select(&source, &cache).unwrap().key);

    std::fs::write(root.join("later.txt"), "available now").unwrap();
    let available = select(&source, &cache).unwrap();
    assert!(available.warnings.is_empty());
    assert_eq!(available.copies.len(), 2);
    assert_ne!(first.key, available.key);

    std::fs::write(
        root.join("picture one.svg"),
        "<svg><text>changed</text></svg>",
    )
    .unwrap();
    let changed = select(&source, &cache).unwrap();
    assert_ne!(available.key, changed.key);
    assert_eq!(changed.view.readme.as_ref().unwrap().text, readme);
}

#[test]
fn optional_readme_and_missing_resource_keep_precise_locations() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let source = source(root);
    let cache = ArtifactCacheRoot::new(root.join("cache")).unwrap();
    let absent = select(&source, &cache).unwrap();
    assert!(absent.view.readme.is_none());
    assert!(absent.warnings.is_empty());
    let readme = "# 概述\n\n[Missing](absent.png)\n";
    std::fs::write(root.join("README.md"), readme).unwrap();
    let present = select(&source, &cache).unwrap();
    assert_ne!(absent.key, present.key);
    let scoop_protocol::DiagnosticOriginV1::HostPathSpan { path, span } =
        present.warnings[0].origin()
    else {
        panic!("expected README location")
    };
    assert_eq!(path.to_path_buf().unwrap(), root.join("README.md"));
    assert_eq!(
        &readme[span.start() as usize..span.end() as usize],
        "[Missing](absent.png)"
    );
}
