use super::super::file_headers::available;
use super::*;

#[test]
fn block_imports_keep_alias_navigation_and_nested_callable_definition_scope() {
    let provider = "package api\nfun pick(value: Boolean): Boolean = value";
    let consumer = "fun editing(): Boolean { import api.pick as selected; val call: (Boolean) -> Boolean = ::selected; fun nested(): Boolean = selected(true); return call(nested()) }";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", consumer)]);
    assert!(
        available(&mut analysis, "editing"),
        "{:?}",
        analysis.diagnostics()
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let selected = alias(index, "selected");
    assert_eq!(index.references_to(selected).count(), 3);
    assert_eq!(
        index.targets_at(&at("src/use.scoop", consumer, "selected(true)", 8)),
        &[selected]
    );
    let target = index.targets_at(&at("src/api.scoop", provider, "pick(value", 4))[0];
    assert!(
        index
            .targets_at(&at("src/use.scoop", consumer, "pick as", 4))
            .contains(&target)
    );
}

#[test]
fn block_imports_failed_aliases_block_outer_candidates_without_leaking() {
    let source = "fun selected(value: Boolean): Boolean = value\nfun failed(): Boolean { import missing.pick as selected; return selected(true) }\nfun unaffected(): Boolean = selected(true)";
    let mut analysis = analyze_files(&[("src/use.scoop", source)]);
    assert!(!available(&mut analysis, "failed"));
    assert!(available(&mut analysis, "unaffected"));
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.aliases.is_empty());
    let target = index.targets_at(&at("src/use.scoop", source, "selected(value", 8))[0];
    assert_eq!(index.references_to(target).count(), 2);
}

#[test]
fn block_imports_supply_incomplete_call_candidates_and_extension_completion() {
    let provider =
        "package api\nfun pick(flag: Boolean): Boolean = flag\nfun Boolean.extra(): Boolean = this";
    let call = "fun editing() { import api.pick as selected; selected(";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/call.scoop", call)]);
    let id = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(id, || false).unwrap();
    assert_eq!(calls.len(), 1);
    let V::Known(candidates) = &calls[0].candidates else {
        panic!("block import candidates are available: {calls:?}");
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
    for (prefix, visible) in [
        ("import api.extra;", true),
        ("{ import api.extra; }", false),
    ] {
        let source = format!("fun editing(value: Boolean) {{ {prefix}\nvalue. }}");
        let mut analysis =
            analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", &source)]);
        let members = super::super::members::members(&mut analysis);
        assert_eq!(
            members
                .candidates
                .iter()
                .any(|member| member.name == "extra"),
            visible
        );
    }
}
