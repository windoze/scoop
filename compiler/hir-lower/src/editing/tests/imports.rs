use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use hir::EditingReferenceTarget as Target;

mod reexports;

pub(super) fn analyze_files(files: &[(&str, &str)]) -> DeclarationAnalysis {
    analyze_file_roles(
        &files
            .iter()
            .map(|&(path, text)| (path, scoop_identity::SourceRole::Production, text))
            .collect::<Vec<_>>(),
    )
}

pub(super) fn analyze_file_roles(
    files: &[(&str, scoop_identity::SourceRole, &str)],
) -> DeclarationAnalysis {
    let core = trusted_core();
    let identities = files
        .iter()
        .map(|(path, _, _)| crate::tests::test_source_identity(path))
        .collect::<Vec<_>>();
    let parsed = files
        .iter()
        .map(|(_, _, text)| scoop_parser::parse_recovering(text).unwrap())
        .collect::<Vec<_>>();
    let inputs = files
        .iter()
        .zip(&identities)
        .zip(&parsed)
        .map(|(((_, role, text), identity), parsed)| {
            scoop_parser::declaration_source(parsed, identity, *role, text)
        })
        .collect::<Vec<_>>();
    let world = hir::ImportedSemanticWorld::from_dependencies(
        identities[0].cone(),
        vec![core.provider()],
        Vec::new(),
    )
    .unwrap();
    let core_inputs =
        CoreProtocolInput::from(core.foundation.import_core_inputs(&core.interface).unwrap());
    analyze_declarations(&inputs, &core_inputs, &world, &[], || false).unwrap()
}

pub(super) fn at(
    path: &str,
    text: &str,
    needle: &str,
    length: usize,
) -> hir::SemanticSourceLocation {
    let start = text.find(needle).unwrap() as u32;
    hir::SemanticSourceLocation {
        source: crate::tests::test_source_identity(path),
        span: ast::Span::new(start, start + length as u32),
    }
}

fn alias(index: &hir::EditingReferenceIndex, name: &str) -> Target {
    Target::ImportAlias(
        index
            .aliases
            .iter()
            .find(|(_, alias)| alias.name == name)
            .unwrap()
            .0,
    )
}

#[test]
fn import_aliases_keep_overloads_and_distinguish_qualified_and_local_lookup() {
    let provider = "object Api { fun pick(value: Boolean): Boolean = value\nfun pick(value: String): String = value }";
    let consumer = "import Api.pick as selected\nfun editing(): Boolean { val first = selected(true); val second = selected(\"text\"); return Api.pick(first) }\nfun shadowed(): Boolean { fun selected(value: Boolean): Boolean = value; return selected(true) }";
    let same_name = "import Api.pick as pick\nfun another(): Boolean = pick(true)";
    let mut analysis = analyze_files(&[
        ("src/api.scoop", provider),
        ("src/use.scoop", consumer),
        ("src/same.scoop", same_name),
    ]);
    let editing = function(&analysis, "editing");
    assert!(
        matches!(
            analysis.function_facts(editing, || false).unwrap(),
            V::Known(_)
        ),
        "{:?}",
        analysis.diagnostics()
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let selected = alias(index, "selected");
    assert_eq!(index.references_to(selected).count(), 3);
    assert_eq!(index.references_to(alias(index, "pick")).count(), 2);
    let targets = index.targets_at(&at("src/use.scoop", consumer, "pick as", 4));
    assert_eq!(targets.len(), 2);
    let boolean = index.targets_at(&at("src/api.scoop", provider, "pick(value: Boolean)", 4))[0];
    assert!(targets.contains(&boolean));
    let references = index.references_to(boolean).collect::<Vec<_>>();
    assert_eq!(references.len(), 6, "{references:?}");
    assert_eq!(
        references
            .iter()
            .filter(|reference| !reference.aliases.is_empty())
            .count(),
        2
    );
    assert_eq!(
        index.targets_at(&at("src/use.scoop", consumer, "pick(first)", 4)),
        &[boolean]
    );
    assert_eq!(
        index.targets_at(&at("src/use.scoop", consumer, "selected(true)", 8)),
        &[selected]
    );
}

#[test]
fn property_aliases_follow_read_write_and_implicit_extension_resolution() {
    let provider = "package api\nvar state: Boolean = false\nclass Box(var item: Boolean)\nvar Box.projected: Boolean get() = item\nset(value) { item = value }";
    let consumer = "import api.state as state\nimport api.Box\nimport api.projected as renamed\nfun editing(box: Box): Boolean { state = true; box.renamed = state; return box.renamed }\nfun Box.implicit(): Boolean { renamed = true; return renamed }";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", consumer)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.references_to(alias(index, "state")).count(), 3);
    assert_eq!(index.references_to(alias(index, "renamed")).count(), 5);
    let property = index.targets_at(&at("src/api.scoop", provider, "projected", 9))[0];
    let references = index.references_to(property).collect::<Vec<_>>();
    assert_eq!(references.len(), 6, "{references:?}");
    assert_eq!(
        references
            .iter()
            .filter(|reference| !reference.aliases.is_empty())
            .count(),
        4
    );
}

#[test]
fn callable_reference_aliases_keep_bound_and_unbound_extension_origins() {
    let provider = "package api\nfun convert(value: Boolean): Boolean = value\nfun Boolean.toggle(): Boolean = this";
    let consumer = "import api.convert as convert\nimport api.toggle as flip\nfun editing(value: Boolean): Boolean { val target: (Boolean) -> Boolean = ::convert; val bound: () -> Boolean = value::flip; val unbound: (Boolean) -> Boolean = Boolean::flip; val called = value.flip(); return target(bound()) }";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", consumer)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    assert_eq!(index.references_to(alias(index, "convert")).count(), 2);
    assert_eq!(index.references_to(alias(index, "flip")).count(), 4);
}

#[test]
fn failed_import_targets_do_not_become_partial_alias_declarations() {
    let provider = "package api\nfun good(value: Boolean): Boolean = value\nfun bad(value: Missing): Boolean = true";
    let consumer = "import api.good as goodAlias\nimport api.bad as badAlias\nfun editing(): Boolean = goodAlias(true)";
    let mut analysis = analyze_files(&[("src/api.scoop", provider), ("src/use.scoop", consumer)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.references_to(alias(index, "goodAlias")).count(), 2);
    assert!(
        !index
            .aliases
            .iter()
            .any(|(_, alias)| alias.name == "badAlias")
    );
    assert!(
        index
            .unavailable
            .contains(&at("src/use.scoop", consumer, "bad as", 3))
    );
}
