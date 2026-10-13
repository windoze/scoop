use super::*;

#[test]
fn test_inputs_do_not_change_the_production_semantic_projection() {
    let source = format!(
        r#"{MINIMAL}
[[test.sources]]
path = "tests/common"
[[test.sources]]
path = "tests/linux"
when = {{ os = "linux" }}
[test-dependencies]
"dev.example:helper" = {{ version = "0.1.0", path = "../helper" }}
"dev.example:binary" = {{ version = "2.0.0", artifact = "../binary.slib" }}
"scoop:scoop.test" = "0.1.0"
"#
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    let production = parse_cone_manifest(MINIMAL).unwrap();
    assert_eq!(parsed.semantic(), production.semantic());
    assert_eq!(parsed.locators(), production.locators());
    assert_eq!(parsed.diagnostic_spans().dependency_iter().len(), 0);
    let SourceSelection::Explicit(paths) = parsed.tests().sources() else {
        panic!("explicit test source list was lost");
    };
    assert_eq!(paths[0].path().as_str(), "tests/common");
    assert_eq!(paths[1].path().as_str(), "tests/linux");
    let entries: Vec<_> = parsed.tests().dependency_iter().collect();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].0.to_string(), "dev.example:binary");
    assert_eq!(entries[0].1.coordinate().version(), "2.0.0");
    assert_eq!(
        entries[0].1.locator().artifact_path().unwrap().as_path(),
        Path::new("../binary.slib")
    );
    assert_eq!(
        &source[entries[0].1.spans().artifact().unwrap().range()],
        "\"../binary.slib\""
    );
    assert_eq!(
        &source[entries[1].1.spans().path().unwrap().range()],
        "\"../helper\""
    );
    assert_eq!(&source[entries[1].1.spans().version().range()], "\"0.1.0\"");
    assert!(entries[2].1.locator().uses_search_roots());
}

#[test]
fn test_sources_distinguish_default_and_explicit_empty_selection() {
    let absent = parse_cone_manifest(MINIMAL).unwrap();
    let empty_table = parse_cone_manifest(&format!("{MINIMAL}\n[test]\n")).unwrap();
    let empty_list = parse_cone_manifest(&format!("{MINIMAL}\n[test]\nsources = []\n")).unwrap();
    assert_eq!(absent.tests().sources(), &SourceSelection::Default);
    assert_eq!(empty_table.tests(), absent.tests());
    assert_eq!(
        empty_list.tests().sources(),
        &SourceSelection::Explicit(Vec::new())
    );
}

#[test]
fn duplicate_test_dependency_points_at_test_declaration() {
    for declaration in [
        "\"0.1.0\"",
        "\"2.0.0\"",
        "{ version = \"0.1.0\", path = \"../alternate\" }",
    ] {
        let source = format!(
            "{MINIMAL}\n[dependencies]\n\"dev.example:helper\" = \"0.1.0\"\n\
             [test-dependencies]\n\"dev.example:helper\" = {declaration}\n"
        );
        let error = parse_cone_manifest(&source).unwrap_err();
        assert_eq!(
            error.kind(),
            &ManifestParseErrorKind::DuplicateTestDependency("dev.example:helper".into())
        );
        assert_eq!(&source[error.span().unwrap().range()], declaration);
        assert!(error.span().unwrap().range().start > source.find("[test-dependencies]").unwrap());
    }
}

#[test]
fn malformed_test_configuration_is_not_ignored_by_production_parsing() {
    for fields in [
        "[test]\nunknown = true",
        "[test]\nsources = 'tests'",
        "[[test.sources]]\npath = '../outside'",
        "[[test.sources]]\npath = 'tests'\nwhen = { os = 'unknown' }",
        "[test-dependencies]\n'helper' = '0.1.0'",
        "[test-dependencies]\n'dev.example:helper' = '^0.1.0'",
        "[test-dependencies]\n'dev.example:helper' = { version = '0.1.0', path = '' }",
        "[test-dependencies]\n'dev.example:helper' = { version = '0.1.0', path = 'a', artifact = 'b' }",
    ] {
        let source = format!("{MINIMAL}\n{fields}\n");
        let error = parse_cone_manifest(&source).unwrap_err();
        let range = error
            .span()
            .expect("test configuration diagnostic span")
            .range();
        assert!(range.start >= MINIMAL.len(), "{fields}: {error}");
        assert!(range.end <= source.len(), "{fields}: {error}");
    }
}
