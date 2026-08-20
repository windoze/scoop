use super::*;

const CORE: &str =
    include_str!("../../../../../tests/fixtures/core-library/dependencies/core/Cone.toml");
const SOURCE_LOCATOR: &str = r#"{ version = "1.0.0", path = "../helper" }"#;

#[test]
fn core_manifest_dependencies_preserve_common_semantics_locators_and_spans() {
    for locator in [
        SOURCE_LOCATOR,
        r#""1.0.0""#,
        r#"{ version = "1.0.0", artifact = "../helper.slib" }"#,
    ] {
        let source = CORE.replace(SOURCE_LOCATOR, locator);
        let ordinary = source.replace("scoop.core", "ordinary");
        let core = parse_cone_manifest(&source).unwrap();
        let ordinary = parse_cone_manifest(&ordinary).unwrap();
        assert_eq!(core.semantic().dependency_count(), 1);
        assert_eq!(
            core.semantic().dependencies,
            ordinary.semantic().dependencies
        );
        assert_eq!(core.locators(), ordinary.locators());
        let (key, coordinate) = core.semantic().dependency_iter().next().unwrap();
        assert_eq!(
            coordinate,
            &ConeCoordinate::new("test", "helper", "1.0.0").unwrap()
        );
        let span = core
            .diagnostic_spans()
            .dependency(key)
            .unwrap()
            .declaration()
            .range();
        assert_eq!(&source[span], locator);
    }
}

#[test]
fn core_manifest_dependency_errors_use_common_diagnostics_and_precise_spans() {
    for (locator, kind, message, text) in [
        (
            r#"{ version = "1.0.0", path = "../helper", artifact = "../helper.slib" }"#,
            ManifestParseErrorKind::ConflictingDependencyLocators,
            "a dependency cannot specify both path and artifact",
            r#"{ version = "1.0.0", path = "../helper", artifact = "../helper.slib" }"#,
        ),
        (
            r#"{ version = "1.0.0", path = "" }"#,
            ManifestParseErrorKind::EmptyDependencyLocator,
            "a dependency locator must not be empty",
            r#""""#,
        ),
    ] {
        let source = CORE.replace(SOURCE_LOCATOR, locator);
        let ordinary = source.replace("scoop.core", "ordinary");
        let error = parse_cone_manifest(&source).unwrap_err();
        let ordinary_error = parse_cone_manifest(&ordinary).unwrap_err();
        assert_eq!(error.kind(), &kind);
        assert_eq!(error.kind(), ordinary_error.kind());
        assert_eq!(error.kind().to_string(), message);
        assert_eq!(&source[error.span().unwrap().range()], text);
    }
}
