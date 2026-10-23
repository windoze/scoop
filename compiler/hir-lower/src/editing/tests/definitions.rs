use super::imports::{analyze_files, at};
use super::*;

#[test]
fn import_definition_navigation_reuses_frozen_targets_without_checking_bodies() {
    let provider = "object Api { fun choose(value: Boolean): Boolean = value\nfun choose(value: String): String = value }\nval flag: Boolean = true";
    let consumer = "import Api.choose as chosen\nimport flag as enabled\nfun broken() { missing }";
    let analysis = analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", consumer)]);
    let expected = [
        at("src/api.scoop", provider, "choose(value: Boolean)", 6),
        at("src/api.scoop", provider, "choose(value: String)", 6),
    ];
    for name in ["choose", "chosen"] {
        let origin = at("src/use.scoop", consumer, name, name.len());
        let definitions = analysis
            .import_definitions(&origin.source, origin.span, || false)
            .unwrap();
        assert_eq!(definitions.len(), 2);
        assert!(
            definitions
                .iter()
                .all(|definition| expected.contains(definition))
        );
        assert!(matches!(
            analysis.import_definitions(&origin.source, origin.span, || true),
            Err(EditingCancelled)
        ));
    }
    let enabled = at("src/use.scoop", consumer, "enabled", 7);
    assert_eq!(
        analysis
            .import_definitions(&enabled.source, enabled.span, || false)
            .unwrap(),
        [at("src/api.scoop", provider, "flag", 4)]
    );
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.reference_index.is_none());
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
}

#[test]
fn named_argument_definitions_use_overload_indices_and_local_function_owners() {
    let provider = "fun choose(left: Boolean, right: String): Boolean = left\nfun choose(left: String, right: Boolean): Boolean = right";
    let consumer = "import choose as selected\nfun editing(): Boolean { fun local(second: Boolean, first: Boolean): Boolean = first; return selected(right = \"text\", left = local(first = true, second = false)) }";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", consumer)]);
    let function = function(&analysis, "editing");
    for (name, path, text, declaration) in [
        ("left", "src/api.scoop", provider, "left: Boolean"),
        ("right", "src/api.scoop", provider, "right: String"),
        ("first", "src/use.scoop", consumer, "first: Boolean"),
        ("second", "src/use.scoop", consumer, "second: Boolean"),
    ] {
        let origin = at("src/use.scoop", consumer, &format!("{name} ="), name.len());
        let definition = analysis
            .argument_definition(function, origin.span, || false)
            .unwrap();
        let V::Known(Some(definition)) = definition else {
            panic!("{:?}", analysis.diagnostics())
        };
        assert_eq!(definition, at(path, text, declaration, name.len()));
    }
    assert!(analysis.reference_index.is_none());
    let span = at("src/use.scoop", consumer, "left =", 4).span;
    assert!(matches!(
        analysis.argument_definition(function, span, || true),
        Err(EditingCancelled)
    ));
}
