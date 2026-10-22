use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn qualified_dependency_candidates_share_nominal_alias_and_singleton_lookup() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "qualified-editing-candidates", "1.0.0").unwrap();
    let provider = r#"
        public enum Choice<T> {
            Empty, Full(val item: T, val flag: Boolean = true)
            public companion object Factory {
                public fun make(value: T): Choice<T> = Choice.Full(value)
            }
        }
        public typealias FixedChoice = Choice<Boolean>
        public typealias GenericChoice<Item> = Choice<Item>
        public class Nest {
            public struct Value<Element>(val item: Element)
            public class Ref public constructor(public val flag: Boolean)
            public typealias Alias<Item> = Value<Item>
            private class Hidden
            public companion object {
                public fun make(flag: Boolean): Boolean = flag
            }
        }
        public object Tools { public fun ping(flag: Boolean): Boolean = flag }
    "#;
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 187);
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
        ("fun editing() { Choice.Full<Boolean>(", Some("T")),
        ("fun editing() { Choice<Boolean>.Full(", Some("Boolean")),
        ("fun editing() { Choice.Full(", Some("T")),
        ("fun editing() { FixedChoice.Full(", Some("Boolean")),
        ("fun editing() { GenericChoice.Full<Boolean>(", Some("Item")),
        (
            "typealias Local<U> = Choice<U>\nfun editing() { Local.Full<Boolean>(",
            Some("U"),
        ),
        (
            "import Choice as Renamed\nfun editing() { Renamed.Full<Boolean>(",
            Some("T"),
        ),
        ("fun editing() { Nest.Value(", Some("Element")),
        ("fun editing() { Nest.Ref(", Some("Boolean")),
        ("fun editing() { Nest.Alias<Boolean>(", Some("Item")),
        ("fun editing() { Nest.make(", Some("Boolean")),
        ("fun editing() { Choice<Boolean>.make(", Some("Boolean")),
        ("fun editing() { Tools.ping(", Some("Boolean")),
        ("fun editing(Choice: Tools) { Choice.ping(", Some("Boolean")),
        ("fun editing(Choice: Boolean) { Choice.Full(", None),
        ("fun editing() { Choice.Full<Boolean>(\"wrong\",", None),
        ("fun editing() { Choice<Boolean>.Full(\"wrong\",", None),
        (
            "fun editing() { GenericChoice.Full<Boolean>(\"wrong\",",
            None,
        ),
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
        assert!(
            analysis.diagnostics().is_empty(),
            "{text}: {:?}",
            analysis.diagnostics()
        );
        let function = function(&analysis, "editing");
        let calls = analysis.incomplete_calls(function, || false).unwrap();
        assert_eq!(calls.len(), 1, "{text}: {calls:?}");
        let V::Known(candidates) = &calls[0].candidates else {
            panic!("{text}: {calls:?}")
        };
        if let Some(expected) = expected {
            assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
            assert_eq!(candidates[0].parameters[0].type_label, expected, "{text}");
        } else {
            assert!(candidates.is_empty(), "{text}: {candidates:?}");
        }
    }
    for text in [
        "fun editing() { Nest.Hidden(",
        "fun editing() { Choice.make(",
        "fun editing() { FixedChoice.Empty(",
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
        assert!(
            matches!(calls[0].candidates, V::Unavailable(U::DeclarationFailure)),
            "{text}: {calls:?}"
        );
    }
}
