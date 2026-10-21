use super::*;

fn coordinates(source: &str) -> Vec<String> {
    parse_single_file_dependencies(source)
        .unwrap()
        .iter()
        .map(|dependency| dependency.coordinate().to_string())
        .collect()
}

#[test]
fn defaults_are_always_direct_and_explicit_defaults_merge() {
    let expected = ["scoop:scoop.core:0.1.0", "scoop:scoop.std:0.1.0"];
    assert_eq!(coordinates("fun main() {}"), expected);
    assert_eq!(
        coordinates("//! dep: scoop:scoop.std:0.1.0\n//! dep: scoop:scoop.core:0.1.0"),
        expected
    );
    assert_eq!(
        coordinates("//! dep: dev:lib:1.2.3\nfun main() {}"),
        [expected[0], expected[1], "dev:lib:1.2.3"]
    );
}

#[test]
fn only_real_header_line_comments_are_metadata() {
    assert_eq!(
        coordinates("// ordinary\r//! dep: ignored:lib:1.0.0\nfun main() {}").len(),
        2
    );
    let source = "#!scoop run\r\n// introductory text\r\n/* //! dep: ignored:lib:1.0.0 */\n/// //! dep: ignored:doc:1.0.0\n//! unknown\n//! dep: dev:lib:1.0.0\r\npackage ignored\n//! dep: ignored:late:1.0.0\nfun main() {}";
    let dependencies = parse_single_file_dependencies(source).unwrap();
    assert_eq!(dependencies.len(), 3);
    let dependency = &dependencies[2];
    assert_eq!(
        &source[dependency.span().unwrap()],
        "//! dep: dev:lib:1.0.0"
    );
    for token in [
        "import x",
        "fun main() {}",
        "package x",
        "\"// comment\"",
        "\u{feff}",
    ] {
        assert_eq!(
            coordinates(&format!("{token}\n//! dep: ignored:lib:1.0.0")).len(),
            2
        );
    }
}

#[test]
fn malformed_or_duplicate_dependencies_report_the_original_line() {
    for directive in [
        "//! dep",
        "//! dep dev:lib:1.0.0",
        "//! dep:",
        "//! dep: dev:lib:part:1.0.0",
        "//! dep: dev:lib:01.0.0",
    ] {
        let text = format!("#!scoop run\n  {directive}\n");
        let error = parse_single_file_dependencies(&text).unwrap_err();
        assert_eq!(&text[error.span], directive);
    }
    assert!(
        parse_single_file_dependencies(
            "//! dep: scoop:scoop.std:0.1.0\n//! dep: scoop:scoop.std:0.1.0"
        )
        .unwrap_err()
        .message
        .contains("duplicate")
    );
    assert_eq!(coordinates("//! dep: scoop:scoop.std:2.0.0").len(), 3);
    assert_eq!(
        coordinates("//! dependency: ignored\nfun main() {}").len(),
        2
    );
}
