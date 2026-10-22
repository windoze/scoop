use super::file_headers::available;
use super::imports::{analyze_file_roles, analyze_files};

#[test]
fn unknown_packages_keep_callable_names_as_lookup_barriers() {
    let source = "package api\nfun pick(): Boolean = true\nfun editing(): Boolean = pick()\nfun healthy(value: Boolean): Boolean = value";
    let mut analysis = analyze_files(&[
        ("src/broken.scoop", "package .\nfun pick(): Boolean = false"),
        ("src/api.scoop", source),
    ]);
    assert!(!available(&mut analysis, "editing"));
    assert!(available(&mut analysis, "healthy"));
}

#[test]
fn unknown_packages_block_qualified_type_lookups_without_moving_the_declaration() {
    let source = "package api\nclass Box\nfun editing(value: api.Box) {}\nfun healthy(value: Boolean): Boolean = value";
    for (header, expected) in [("package .", false), ("package other", true)] {
        let changed = format!("{header}\nclass Box");
        let mut analysis =
            analyze_files(&[("src/changed.scoop", &changed), ("src/api.scoop", source)]);
        assert_eq!(
            available(&mut analysis, "editing"),
            expected,
            "{:?}",
            analysis.diagnostics()
        );
        assert!(available(&mut analysis, "healthy"));
    }
}

#[test]
fn known_packages_do_not_block_unrelated_package_names() {
    let source = "package api\nfun pick(): Boolean = true\nfun editing(): Boolean = pick()";
    for (package, expected) in [("api", false), ("other", true)] {
        let broken = format!("package {package}\nimport api.\nfun pick(): Boolean = false");
        let mut analysis =
            analyze_files(&[("src/broken.scoop", &broken), ("src/api.scoop", source)]);
        assert_eq!(
            available(&mut analysis, "editing"),
            expected,
            "{:?}",
            analysis.diagnostics()
        );
    }
}

#[test]
fn unavailable_file_declarations_still_obey_private_visibility() {
    let mut analysis = analyze_files(&[
        (
            "src/broken.scoop",
            "package .\nprivate fun pick(): Boolean = false",
        ),
        (
            "src/use.scoop",
            "fun pick(): Boolean = true\nfun editing(): Boolean = pick()",
        ),
    ]);
    assert!(
        available(&mut analysis, "editing"),
        "{:?}",
        analysis.diagnostics()
    );
}

#[test]
fn production_queries_ignore_unavailable_test_declarations() {
    use scoop_identity::SourceRole::{Production, Test};
    let broken = "package .\nfun pick(): Boolean = false";
    let source = "fun pick(): Boolean = true\nfun editing(): Boolean = pick()";
    let mut production = analyze_file_roles(&[
        ("tests/broken.scoop", Test, broken),
        ("src/use.scoop", Production, source),
    ]);
    assert!(available(&mut production, "editing"));
    let mut tests = analyze_file_roles(&[
        ("tests/broken.scoop", Test, broken),
        ("tests/use.scoop", Test, source),
    ]);
    assert!(!available(&mut tests, "editing"));
}

#[test]
fn unavailable_names_preserve_declaration_namespace_and_lexical_priority() {
    for declaration in [
        "class pick",
        "typealias pick = Boolean",
        "fun Boolean.pick(): Boolean = this",
    ] {
        let broken = format!("package .\n{declaration}");
        let mut analysis = analyze_files(&[
            ("src/broken.scoop", &broken),
            (
                "src/use.scoop",
                "val pick: Boolean = true\nfun editing(): Boolean = pick",
            ),
        ]);
        assert!(
            available(&mut analysis, "editing"),
            "{declaration}: {:?}",
            analysis.diagnostics()
        );
    }
    let mut analysis = analyze_files(&[
        (
            "src/broken.scoop",
            "package .\nval value: Boolean = false\nfun pick(): Boolean = false",
        ),
        (
            "src/use.scoop",
            "fun editing(value: Boolean): Boolean { fun pick(): Boolean = value; return pick() }",
        ),
    ]);
    assert!(available(&mut analysis, "editing"));
}

#[test]
fn unavailable_nominal_and_property_names_do_not_fall_through() {
    for (declaration, provider, consumer) in [
        ("class Box", "class Box", "fun editing(value: Box) {}"),
        (
            "typealias Box = Boolean",
            "class Box",
            "fun editing(value: Box) {}",
        ),
        (
            "val state: Boolean = false",
            "val state: Boolean = true",
            "fun editing(): Boolean = state",
        ),
        ("object Box", "object Box", "fun editing(): Any = Box"),
    ] {
        let broken = format!("package .\n{declaration}");
        let source =
            format!("{provider}\n{consumer}\nfun healthy(value: Boolean): Boolean = value");
        let mut analysis =
            analyze_files(&[("src/broken.scoop", &broken), ("src/use.scoop", &source)]);
        assert!(!available(&mut analysis, "editing"), "{declaration}");
        assert!(
            available(&mut analysis, "healthy"),
            "{declaration}: {:?}",
            analysis.diagnostics()
        );
    }
}

#[test]
fn exact_and_star_imports_retain_unavailable_same_name_targets() {
    for import in ["import api.pick as selected", "import api.*"] {
        let name = if import.contains(" as ") {
            "selected"
        } else {
            "pick"
        };
        let consumer = format!("{import}\nfun editing(): Boolean = {name}()");
        let mut analysis = analyze_files(&[
            ("src/broken.scoop", "package .\nfun pick(): Boolean = false"),
            ("src/api.scoop", "package api\nfun pick(): Boolean = true"),
            ("src/use.scoop", &consumer),
        ]);
        assert!(!available(&mut analysis, "editing"), "{import}");
    }
}

#[test]
fn unknown_package_extensions_keep_the_original_operator_role() {
    let source =
        "operator fun String.not(): Boolean = true\nfun editing(value: String): Boolean = !value";
    for (modifier, expected) in [("operator ", false), ("", true)] {
        let broken = format!("package .\n{modifier}fun String.not(): Boolean = false");
        let mut analysis =
            analyze_files(&[("src/broken.scoop", &broken), ("src/use.scoop", source)]);
        assert_eq!(
            available(&mut analysis, "editing"),
            expected,
            "{:?}",
            analysis.diagnostics()
        );
    }
}
