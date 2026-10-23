use super::super::file_headers::available;
use super::*;

#[test]
fn a_failed_exact_import_keeps_successful_imports_and_independent_files() {
    let provider = "package api\nfun good(value: Boolean): Boolean = value";
    let consumer = "import missing.pick as selected\nimport api.good as valid\nfun independent(): Boolean = valid(true)\nfun failed(): Boolean = selected(true)\nfun local(selected: Boolean): Boolean = selected";
    let fallback = "fun selected(value: Boolean): Boolean = value\nfun other(): Boolean = true";
    let mut analysis = analyze_files(&[
        ("src/api.scoop", provider),
        ("src/use.scoop", consumer),
        ("src/other.scoop", fallback),
    ]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    assert!(available(&mut analysis, "local"));
    assert!(available(&mut analysis, "other"));
    assert!(!available(&mut analysis, "failed"));
    let diagnostics = analysis.diagnostics();
    assert!(
        diagnostics
            .iter()
            .any(|error| error.message.contains("import binding for `selected`"))
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.declarations_available);
    assert!(index.unavailable.contains(&at(
        "src/use.scoop",
        consumer,
        "import missing.pick as selected",
        "import missing.pick as selected".len()
    )));
    assert_eq!(index.references_to(alias(index, "valid")).count(), 2);
    assert!(
        !index
            .aliases
            .iter()
            .any(|(_, alias)| alias.name == "selected")
    );
    let fallback = index.targets_at(&at("src/other.scoop", fallback, "selected", 8))[0];
    assert_eq!(index.references_to(fallback).count(), 1);
}

#[test]
fn a_failed_star_keeps_higher_lookup_layers_and_blocks_prelude_fallthrough() {
    let provider = "typealias Flag = Boolean\nfun picked(value: Flag): Flag = value\n";
    let other = "package api\nfun good(value: Boolean): Boolean = value";
    let consumer = "import missing.*\nimport api.good as valid\nfun independent(input: Flag): Flag = input\nfun exact(): Flag = valid(true)\nfun current(): Flag = picked(true)\nfun failed(value: String): Flag = true\nfun missing(): Flag = unknown(true)";
    let mut analysis = analyze_files(&[
        ("src/flags.scoop", provider),
        ("src/api.scoop", other),
        ("src/use.scoop", consumer),
    ]);
    for name in ["independent", "exact", "current", "good"] {
        assert!(
            available(&mut analysis, name),
            "{name}: {:?}",
            analysis.diagnostics()
        );
    }
    assert!(!available(&mut analysis, "failed"));
    assert!(!available(&mut analysis, "missing"));
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.contains(&at(
        "src/use.scoop",
        consumer,
        "import missing.*",
        "import missing.*".len()
    )));
    assert_eq!(index.references_to(alias(index, "valid")).count(), 2);
}

#[test]
fn failed_type_imports_block_same_named_types_and_constructors() {
    let declarations = "typealias Flag = Boolean\nstruct Box(val value: Flag)";
    let consumer = "import missing.Type as Box\nfun independent(value: Flag): Flag = value\nfun typeUse(value: Box): Flag = true\nfun constructorUse(): Flag { val box = Box(true); return true }";
    let mut analysis = analyze_files(&[
        ("src/types.scoop", declarations),
        ("src/use.scoop", consumer),
    ]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    assert!(!available(&mut analysis, "typeUse"));
    assert!(!available(&mut analysis, "constructorUse"));
}

#[test]
fn unknown_import_roles_block_implicit_operators_without_hiding_named_extensions() {
    let declarations = "typealias Flag = Boolean\nclass Receiver\noperator fun Receiver.plus(other: Flag): Flag = other";
    let consumer = "import missing.anything as unknownRole\nfun named(value: Receiver): Flag = value.plus(true)\nfun operated(value: Receiver): Flag = value + true";
    let mut analysis = analyze_files(&[
        ("src/types.scoop", declarations),
        ("src/use.scoop", consumer),
    ]);
    assert!(
        available(&mut analysis, "named"),
        "{:?}",
        analysis.diagnostics()
    );
    assert!(!available(&mut analysis, "operated"));
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|error| error.message.contains("import binding for `plus`"))
    );
}
