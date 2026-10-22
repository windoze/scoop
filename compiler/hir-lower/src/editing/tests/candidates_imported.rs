use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_candidates_keep_owner_applications_and_declared_generic_binders() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "editing-call-candidates", "1.0.0").unwrap();
    let provider = "public class Box<Element> public constructor(public val item: Element) { public fun read(next: Element): Element = next\nprivate fun hidden(next: Element): Element = next }\npublic fun <Item> Box<Item>.extension(value: Item): Item = value";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 186);
    let identity = crate::tests::test_source_identity("src/editing.scoop");
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
    let core_inputs =
        CoreProtocolInput::from(core.foundation.import_core_inputs(&core.interface).unwrap());
    for (text, expected) in [
        ("fun editing() { true.equals(", Some("Boolean")),
        ("fun editing() { (1).equals(", Some("Int")),
        (
            "fun editing(value: Box<Boolean>) { value.read(",
            Some("Boolean"),
        ),
        (
            "import extension as renamed\nfun editing(value: Box<Boolean>) { value.renamed(",
            Some("Item"),
        ),
        ("fun editing() { Box(", Some("Element")),
        (
            "typealias Alias = Box<Boolean>\nfun editing() { Alias(",
            Some("Boolean"),
        ),
        ("fun editing(value: Box<Boolean>) { value.hidden(", None),
    ] {
        let parsed = scoop_parser::parse_recovering(text).unwrap();
        let source = scoop_parser::declaration_source(
            &parsed,
            &identity,
            scoop_identity::SourceRole::Production,
            text,
        );
        let mut analysis =
            analyze_declarations(&[source], &core_inputs, &world, &[], || false).unwrap();
        let function = function(&analysis, "editing");
        let calls = analysis.incomplete_calls(function, || false).unwrap();
        assert_eq!(calls.len(), 1, "{text}: {calls:?}");
        let V::Known(candidates) = &calls[0].candidates else {
            panic!("{text}: {calls:?}");
        };
        match expected {
            Some(expected) => {
                assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
                assert_eq!(candidates[0].parameters[0].type_label, expected, "{text}");
            }
            None => assert!(candidates.is_empty(), "{text}: {candidates:?}"),
        }
    }
}
