use super::*;

const MINIMAL: &str = r#"
schema = 1

[cone]
group = "dev.example"
name = "sample"
version = "0.1.0"
kind = "library"
"#;

#[test]
fn parses_minimal_manifest_and_preserves_field_spans() {
    let parsed = parse_cone_manifest(MINIMAL).unwrap();

    assert_eq!(
        parsed.semantic().coordinate(),
        &ConeCoordinate::new("dev.example", "sample", "0.1.0").unwrap()
    );
    assert_eq!(
        parsed.semantic().requested_kind(),
        RequestedConeKind::Library
    );
    assert!(parsed.semantic().is_dependency_free());
    assert!(parsed.locators().is_empty());
    assert_eq!(
        &MINIMAL[parsed.diagnostic_spans().cone_fields().name().range()],
        "\"sample\""
    );
}

#[test]
fn separates_dependency_semantics_from_locators() {
    let source = format!(
        "{MINIMAL}\n[dependencies]\n\
             \"org.foo:bar\" = {{ version = \"1.2.3\", path = \"../bar\" }}\n\
             \"org.acme:util\" = {{ version = \"2.0.0\", artifact = \"../util.slib\" }}\n\
             \"org.other:log\" = \"3.1.0\"\n"
    );
    let parsed = parse_cone_manifest(&source).unwrap();
    let keys = parsed
        .semantic()
        .dependency_iter()
        .map(|(key, _)| key.to_string())
        .collect::<Vec<_>>();

    assert_eq!(keys, ["org.acme:util", "org.foo:bar", "org.other:log"]);
    assert_eq!(parsed.semantic().dependency_count(), 3);
    assert_eq!(
        parsed
            .semantic()
            .dependency_iter()
            .map(|(key, _)| key.to_string())
            .collect::<Vec<_>>(),
        keys
    );
    for (key, _) in parsed.semantic().dependency_iter() {
        match key.to_string().as_str() {
            "org.foo:bar" => {
                let locator = parsed.locators().get(key).unwrap();
                assert_eq!(
                    locator.source_path().unwrap().as_path(),
                    Path::new("../bar")
                );
                assert!(locator.artifact_path().is_none());
                let spans = parsed.diagnostic_spans().dependency(key).unwrap();
                assert_eq!(&source[spans.path().unwrap().range()], "\"../bar\"");
            }
            "org.acme:util" => assert!(matches!(
                parsed.locators().get(key).unwrap().artifact_path(),
                Some(path) if path.as_path() == Path::new("../util.slib")
            )),
            "org.other:log" => {
                assert!(parsed.locators().get(key).unwrap().uses_search_roots());
            }
            other => panic!("unexpected dependency {other}"),
        }
        let spans = parsed.diagnostic_spans().dependency(key).unwrap();
        assert_eq!(
            &source[spans.version().range()],
            match key.to_string().as_str() {
                "org.acme:util" => "\"2.0.0\"",
                "org.foo:bar" => "\"1.2.3\"",
                "org.other:log" => "\"3.1.0\"",
                _ => unreachable!("dependency keys were checked above"),
            }
        );
    }
    assert_eq!(parsed.diagnostic_spans().dependency_iter().count(), 3);
}

#[test]
fn rejects_noncanonical_and_reserved_semantics() {
    let cases = [
        (
            MINIMAL.replace("0.1.0", "0.1.00"),
            ManifestParseErrorKind::InvalidConeCoordinate(
                ConeCoordinateError::InvalidSemanticVersion,
            ),
        ),
        (
            MINIMAL
                .replace("dev.example", "scoop")
                .replace("sample", "single-file")
                .replace("0.1.0", "0.0.0"),
            ManifestParseErrorKind::ReservedConeCoordinate,
        ),
    ];

    for (source, expected) in cases {
        assert_eq!(*parse_cone_manifest(&source).unwrap_err().kind(), expected);
    }
}

#[test]
fn rejects_unknown_duplicate_and_wrong_typed_fields() {
    for source in [
        MINIMAL.replace("schema = 1", "schema = 1\nunknown = true"),
        MINIMAL.replace("name = \"sample\"", "name = \"sample\"\nname = \"again\""),
        MINIMAL.replace("schema = 1", "schema = \"1\""),
    ] {
        assert!(matches!(
            parse_cone_manifest(&source).unwrap_err().kind(),
            ManifestParseErrorKind::InvalidToml(_)
        ));
    }
}

#[test]
fn rejects_invalid_dependency_shapes() {
    let cases = [
        (
            "\"bad:key:extra\" = \"1.0.0\"",
            ManifestParseErrorKind::InvalidDependencyKey("bad:key:extra".to_owned()),
        ),
        (
            "\"org.foo:bar\" = { version = \"1.0.0\", path = \"a\", artifact = \"b\" }",
            ManifestParseErrorKind::ConflictingDependencyLocators,
        ),
        (
            "\"org.foo:bar\" = { version = \"1.0.0\", path = \"\" }",
            ManifestParseErrorKind::EmptyDependencyLocator,
        ),
    ];

    for (dependency, expected) in cases {
        let source = format!("{MINIMAL}\n[dependencies]\n{dependency}\n");
        assert_eq!(*parse_cone_manifest(&source).unwrap_err().kind(), expected);
    }
}

#[test]
fn rejects_unknown_dependency_field() {
    let source = format!(
        "{MINIMAL}\n[dependencies]\n\
             \"org.foo:bar\" = {{ version = \"1.0.0\", branch = \"main\" }}\n"
    );
    assert!(matches!(
        parse_cone_manifest(&source).unwrap_err().kind(),
        ManifestParseErrorKind::InvalidToml(_)
    ));
}

#[test]
fn core_dependency_accepts_the_same_locators_as_other_libraries() {
    for locator in [
        "\"0.1.0\"",
        "{ version = \"0.1.0\", path = \"../core\" }",
        "{ version = \"0.1.0\", artifact = \"../core.slib\" }",
    ] {
        let source = format!("{MINIMAL}\n[dependencies]\n\"scoop:scoop.core\" = {locator}\n");
        let parsed = parse_cone_manifest(&source).unwrap();
        let (key, coordinate) = parsed.semantic().dependency_iter().next().unwrap();
        assert_eq!(coordinate, &ConeCoordinate::reserved_core());
        let span = parsed
            .diagnostic_spans()
            .dependency(key)
            .unwrap()
            .declaration()
            .range();
        assert_eq!(&source[span], locator);
    }
}

#[test]
fn core_uses_the_common_manifest_parser() {
    let core = MINIMAL
        .replace("dev.example", "scoop")
        .replace("sample", "scoop.core");
    let parsed = parse_cone_manifest(&core).unwrap();
    assert_eq!(
        parsed.semantic().coordinate(),
        &ConeCoordinate::reserved_core()
    );
    assert_eq!(
        parsed.semantic().requested_kind(),
        RequestedConeKind::Library
    );
    assert!(parsed.semantic().is_dependency_free());
    let executable = core.replace("kind = \"library\"", "kind = \"executable\"");
    assert_eq!(
        *parse_cone_manifest(&executable).unwrap_err().kind(),
        ManifestParseErrorKind::TrustedCoreMustBeLibrary
    );
}

mod core_dependencies;
