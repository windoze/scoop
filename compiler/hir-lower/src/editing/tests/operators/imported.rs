use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_generic_operators_and_equality_keep_the_original_declaration() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "operator-navigation", "1.0.0").unwrap();
    let provider = "public class Value<T> { public operator fun plus(other: T): T = other; public operator fun equals(other: Value<T>): Boolean = true }";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 199);
    let identity = crate::tests::test_source_identity("src/operators.scoop");
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
    let protocols = core
        .foundation
        .import_core_inputs(&core.interface)
        .unwrap()
        .into();
    let text = "import Value as Box\nfun editing(a: Box<Boolean>, b: Box<String>) { val one = a + true; val two = b + \"text\"; val same = a == a; val different = b != b }";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[input], &protocols, &world, &[], || false).unwrap();
    let references = references(&mut analysis);
    assert_eq!(references.len(), 4, "{references:?}");
    assert_eq!(references[0].target, references[1].target);
    assert_eq!(references[2].target, references[3].target);
    for (reference, name) in references.iter().zip(["plus", "plus", "equals", "equals"]) {
        assert!(matches!(
            reference.target.callable(),
            Some(hir::SemanticCallable::Imported(_))
        ));
        let origin = reference.definition.as_ref().unwrap();
        assert_eq!(origin.source.cone(), coordinate.identity().unwrap());
        assert!(
            provider[origin.span.start as usize..origin.span.end as usize]
                .contains(&format!("fun {name}("))
        );
    }
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}

#[test]
fn exact_extension_imports_record_the_operator_token_origin() {
    let provider =
        "package ops\nclass Value\noperator fun Value.plus(other: Boolean): Boolean = other";
    let consumer = "import ops.Value\nimport ops.plus\nfun editing(value: Value): Boolean = value /* 🌟 */ + true";
    let mut analysis = analyze_files(&[("src/ops.scoop", provider), ("src/use.scoop", consumer)]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 1);
    assert_eq!(
        references[0].definition,
        Some(at("src/ops.scoop", provider, "plus(", 4))
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&at("src/ops.scoop", provider, "plus(", 4))[0];
    let uses = index.references_to(target).collect::<Vec<_>>();
    assert_eq!(uses.len(), 3, "{uses:?}");
    assert_eq!(
        uses.iter()
            .filter(|use_| use_.origin.span == references[0].span)
            .count(),
        1
    );
}

#[test]
fn extension_operator_aliases_follow_the_selected_callable_without_name_matching() {
    let provider =
        "package ops\nclass Value\noperator fun Value.plus(other: Boolean): Boolean = other";
    let other = "package other\nclass Unrelated\noperator fun Unrelated.plus(other: Boolean): Boolean = other";
    let consumer = "import ops.Value\nimport ops.plus as combine\nimport other.plus as unused\nfun editing(value: Value): Boolean = value + true";
    let mut analysis = analyze_files(&[
        ("src/ops.scoop", provider),
        ("src/other.scoop", other),
        ("src/use.scoop", consumer),
    ]);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 1);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    for (name, count) in [("combine", 2), ("unused", 1)] {
        let (id, _) = index
            .aliases
            .iter()
            .find(|(_, alias)| alias.name == name)
            .unwrap();
        assert_eq!(
            index
                .references_to(hir::EditingReferenceTarget::ImportAlias(id))
                .count(),
            count
        );
    }
    let target = index.targets_at(&at("src/ops.scoop", provider, "plus(", 4))[0];
    let operator = index
        .references_to(target)
        .find(|reference| {
            reference.origin.source == crate::tests::test_source_identity("src/use.scoop")
                && reference.origin.span == references[0].span
        })
        .unwrap();
    assert_eq!(
        operator.aliases,
        vec![at("src/use.scoop", consumer, "combine", 7)]
    );
}
