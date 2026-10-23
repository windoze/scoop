use super::*;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

fn analyze_dependency_imports(text: &str) -> DeclarationAnalysis {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "reference-exports", "1.0.0").unwrap();
    let provider = "package api\npublic fun pick(value: Boolean): Boolean = value\npublic fun pick(value: String): String = value\npublic const val ready: Boolean = true\npublic class Box";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 199);
    let identity = crate::tests::test_source_identity("src/reexports.scoop");
    let world = hir::ImportedSemanticWorld::from_dependencies(
        identity.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let protocols = core
        .foundation
        .import_core_inputs(&core.interface)
        .unwrap()
        .into();
    let analysis = analyze_declarations(&[source], &protocols, &world, &[], || false).unwrap();
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    analysis
}

#[test]
fn public_aliases_keep_their_original_export_bindings_and_local_uses() {
    let text = "package facade\npublic import api.pick as choose\npublic import api.pick as alternate\npublic import api.ready as readyFlag\npublic import api.Box as SelectedBox\nfun editing(value: SelectedBox): Boolean { val other = choose(\"text\"); return choose(alternate(readyFlag)) }";
    let mut analysis = analyze_dependency_imports(text);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    for (name, count) in [
        ("choose", 3),
        ("alternate", 2),
        ("readyFlag", 2),
        ("SelectedBox", 2),
    ] {
        let (id, alias) = index
            .aliases
            .iter()
            .find(|(_, alias)| alias.name == name)
            .unwrap();
        assert!(alias.reexported);
        assert_eq!(
            alias.exports.len(),
            if name == "choose" || name == "alternate" {
                2
            } else {
                1
            }
        );
        assert_eq!(index.targets_at(&alias.origin), &[Target::ImportAlias(id)]);
        assert_eq!(index.references_to(Target::ImportAlias(id)).count(), count);
        let exported = alias
            .exports
            .iter()
            .map(|binding| index.references_to(Target::ExportBinding(*binding)).count())
            .sum::<usize>();
        assert_eq!(exported, count + alias.exports.len() - 1);
    }
    let choose = index
        .aliases
        .iter()
        .find(|(_, alias)| alias.name == "choose")
        .unwrap()
        .1;
    let alternate = index
        .aliases
        .iter()
        .find(|(_, alias)| alias.name == "alternate")
        .unwrap()
        .1;
    assert!(
        choose
            .exports
            .iter()
            .all(|binding| !alternate.exports.contains(binding))
    );
    let read = at("src/reexports.scoop", text, "readyFlag))", 9);
    let route = index
        .export_references
        .iter()
        .find(|reference| reference.origin == read)
        .unwrap();
    assert_eq!(route.routes.len(), 1);
    assert_eq!(route.routes[0].hops().len(), 1);
    assert_eq!(route.aliases.len(), 1);
}

#[test]
fn star_reexports_have_no_fabricated_name_and_only_selected_uses() {
    let text =
        "package facade\npublic import api.*\nfun editing(value: Box): Boolean = pick(ready)";
    let mut analysis = analyze_dependency_imports(text);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    assert!(index.aliases.is_empty());
    assert_eq!(index.reexports.len(), 4);
    assert!(
        index
            .reexports
            .iter()
            .all(|export| export.name.is_none() && export.alias.is_none())
    );
    let references = index
        .export_references
        .iter()
        .filter(|reference| {
            index
                .reexports
                .iter()
                .any(|export| export.target == reference.target)
        })
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 3);
    for reference in references {
        assert!(reference.aliases.is_empty());
        let export = index
            .reexports
            .iter()
            .find(|export| export.target == reference.target)
            .unwrap();
        assert_eq!(
            index
                .references_to(Target::ExportBinding(export.binding))
                .count(),
            1
        );
    }
}

#[test]
fn cancelled_and_failed_import_uses_do_not_publish_binding_routes() {
    let text = "package facade\npublic import api.pick as choose\nfun editing(): Boolean { val selected = choose(true); return missing }";
    let mut analysis = analyze_dependency_imports(text);
    assert!(analysis.reference_index(|| true).is_err());
    assert!(analysis.reference_index.is_none());
    let index = analysis.reference_index(|| false).unwrap();
    assert!(!index.unavailable.is_empty());
    let call = at("src/reexports.scoop", text, "choose(true)", 6);
    assert!(
        index
            .export_references
            .iter()
            .all(|reference| reference.origin != call)
    );
    assert!(index.targets_at(&call).is_empty());
}
