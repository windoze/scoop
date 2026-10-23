use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_static_variants_preserve_source_and_type_state_after_trial_commit() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "static-variants", "1.0.0").unwrap();
    let provider =
        "public enum Event<T> { Empty, Item(val value: T) }\npublic typealias Alias<U> = Event<U>";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 221);
    let identity = crate::tests::test_source_identity(FILE);
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
    let text = "import Event.Empty as selected\nval first: Event<Boolean> = selected\nval second: Event<String> = Alias<String>.Empty";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[input], &protocols, &world, &[], || false).unwrap();
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert_eq!(analysis.static_variant_references.len(), 2);
    let first = variant(&analysis, text, "selected\nval second", "selected");
    let offset = text.rfind("Empty").unwrap() as u32;
    let V::Known(Some(second)) = analysis
        .static_variant_reference(&identity, ast::Span::new(offset, offset + 5), || false)
        .unwrap()
    else {
        panic!("imported alias retains the selected variant")
    };
    assert_eq!(first.target, second.target);
    assert!(matches!(first.target, hir::SemanticVariant::Imported(_)));
    assert_eq!(
        first.definition.unwrap().source.cone(),
        coordinate.identity().unwrap()
    );
    assert_eq!(type_name(&mut analysis, first.value_type), "Event<Boolean>");
    assert_eq!(type_name(&mut analysis, second.value_type), "Event<String>");
    let alias = at(FILE, text, "Alias<String>", 5);
    let V::Known(Some(ty)) = analysis
        .source_type_application(&alias.source, alias.span, || false)
        .unwrap()
    else {
        panic!("imported qualifier keeps its applied type")
    };
    assert_eq!(type_name(&mut analysis, ty), "Event<String>");
    let index = analysis.reference_index(|| false).unwrap();
    let original = index.targets_at(&second.origin)[0];
    assert_eq!(index.references_to(original).count(), 3);
}
