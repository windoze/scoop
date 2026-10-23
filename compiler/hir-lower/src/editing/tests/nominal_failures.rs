use super::file_headers::available;
use super::imports::{analyze_files, at};
use super::*;

mod cycles;

const PATH: &str = "src/failures.scoop";
const HEALTHY: &str = "fun independent(value: Boolean): Boolean = value";

fn assert_unavailable(analysis: &mut DeclarationAnalysis, text: &str, needle: &str) {
    let origin = at(PATH, text, needle, needle.len());
    assert!(
        matches!(
            analysis
                .type_reference(&origin.source, origin.span, || false)
                .unwrap(),
            V::Unavailable(U::DeclarationFailure)
        ),
        "{needle}: {:?}",
        analysis.diagnostics()
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.targets_at(&origin).is_empty(), "{needle}");
    assert!(
        index.unavailable.iter().any(|failed| {
            failed.source == origin.source
                && failed.span.start <= origin.span.start
                && origin.span.end <= failed.span.end
        }),
        "{needle}"
    );
}

#[test]
fn failed_representation_fields_never_become_smaller_constructors() {
    for (declaration, construction) in [
        (
            "struct Broken(val bad: Missing, val kept: Boolean)",
            "Broken(true)",
        ),
        (
            "class Broken(val bad: Missing, val kept: Boolean)",
            "Broken(true)",
        ),
        (
            "enum Broken { Item(val bad: Missing, val kept: Boolean), Empty }",
            "Broken.Item(true)",
        ),
    ] {
        let text = format!(
            "{declaration}\n{HEALTHY}\nfun construct(): Boolean {{ val result = {construction}; return true }}"
        );
        let mut analysis = analyze_files(&[(PATH, &text)]);
        assert!(
            available(&mut analysis, "independent"),
            "{:?}",
            analysis.diagnostics()
        );
        assert!(!available(&mut analysis, "construct"));
        assert_unavailable(&mut analysis, &text, "Broken");
        assert_unavailable(&mut analysis, &text, "kept");
        assert!(analysis.lowerer.editing_source_constructors().is_empty());
        let origin = at(PATH, &text, "Broken", 6);
        assert!(!matches!(
            analysis
                .constructor_declaration_at(&origin.source, origin.span, || false)
                .unwrap(),
            V::Known(Some(_))
        ));
    }
}

#[test]
fn forward_dependencies_aliases_and_nested_owners_share_the_failure() {
    let text = format!(
        "typealias First = Before\nstruct Before(val item: Broken)\nstruct Broken(val bad: Missing) {{ class Nested(val flag: Boolean)\ncompanion object {{ fun member(): Boolean = true }} }}\n{HEALTHY}\nfun viaAlias(value: First): Boolean = true\nfun viaNested(value: Broken.Nested): Boolean = true"
    );
    let mut analysis = analyze_files(&[(PATH, &text)]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    for name in ["viaAlias", "viaNested", "member"] {
        assert!(!available(&mut analysis, name), "{name}");
    }
    for name in ["First", "Before", "Broken", "Nested"] {
        assert_unavailable(&mut analysis, &text, name);
    }
    assert!(analysis.lowerer.editing_source_constructors().is_empty());
}

#[test]
fn failed_type_constraints_and_aliases_keep_their_real_name_barriers() {
    for declaration in [
        "struct Broken<T, T>(val item: T)",
        "class Broken<T: Missing>(val item: T)",
        "typealias Broken<T: Missing> = Boolean",
        "typealias Broken<T, T> = Boolean",
        "interface Marker {}\nclass Required<T: Marker>\ntypealias Broken = Required<Boolean>",
        "interface Marker {}\nclass Required<T: Marker>\nstruct Broken(val item: Required<Boolean>)",
    ] {
        let text = format!("{declaration}\n{HEALTHY}");
        let mut analysis = analyze_files(&[(PATH, &text)]);
        assert!(
            available(&mut analysis, "independent"),
            "{declaration}: {:?}",
            analysis.diagnostics()
        );
        assert_unavailable(&mut analysis, &text, "Broken");
    }
    let text = "package failure\nstruct Broken(val bad: Missing)";
    let usage = "import failure.Broken as Choice\nfun construct() { val item = Choice(true) }\nfun typed(value: Choice): Boolean = true";
    let mut analysis = analyze_files(&[
        (PATH, text),
        ("src/use.scoop", usage),
        ("src/fallback.scoop", "struct Choice(val flag: Boolean)"),
    ]);
    assert!(!available(&mut analysis, "construct"));
    assert!(!available(&mut analysis, "typed"));
    let origin = at("src/use.scoop", usage, "Choice", 6);
    assert!(
        analysis
            .import_definitions(&origin.source, origin.span, || false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn failed_owners_do_not_publish_imported_variants_properties_or_member_relations() {
    let text = format!(
        "package failure\nenum Broken {{ Item(val bad: Missing), Empty }}\nobject Singleton : Missing {{ const val kept: Boolean = true\nfun member(): Boolean = true }}\n{HEALTHY}"
    );
    let usage = "import failure.Broken.Empty\nimport failure.Singleton.kept\nfun variant(): Boolean { val item = Empty; return true }\nfun property(): Boolean = kept";
    let mut analysis = analyze_files(&[(PATH, &text), ("src/use.scoop", usage)]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    for name in ["variant", "property", "member"] {
        assert!(!available(&mut analysis, name), "{name}");
    }
    for name in ["Empty", "kept"] {
        let origin = at("src/use.scoop", usage, name, name.len());
        assert!(
            analysis
                .import_definitions(&origin.source, origin.span, || false)
                .unwrap()
                .is_empty()
        );
        assert!(
            analysis
                .reference_index(|| false)
                .unwrap()
                .targets_at(&origin)
                .is_empty()
        );
    }
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("independent member index");
    };
    assert!(index.implementations.is_empty());
    assert!(!index.unavailable.is_empty());
}

#[test]
fn a_failed_method_does_not_revoke_its_owner_or_independent_methods() {
    let text = "class Holder(val flag: Boolean) { fun broken(value: Missing): Boolean = true\nfun member(): Boolean = flag }\nfun independent(): Boolean = Holder(true).member()";
    let mut analysis = analyze_files(&[(PATH, text)]);
    assert!(
        available(&mut analysis, "independent"),
        "{:?}",
        analysis.diagnostics()
    );
    assert!(available(&mut analysis, "member"));
    assert!(!available(&mut analysis, "broken"));
    let origin = at(PATH, text, "Holder", 6);
    assert!(matches!(
        analysis
            .type_reference(&origin.source, origin.span, || false)
            .unwrap(),
        V::Known(Some(_))
    ));
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("independent methods");
    };
    assert!(
        index
            .implementations
            .iter()
            .any(|item| item.origin == at(PATH, text, "member", 6))
    );
    assert!(
        !index
            .implementations
            .iter()
            .any(|item| item.origin == at(PATH, text, "broken", 6))
    );
}
