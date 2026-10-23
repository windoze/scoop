use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_conditional_members_and_incomplete_calls_keep_their_conditions() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "conditional-editing", "1.0.0").unwrap();
    let provider = r#"
        public interface Ready {
            public fun activate(value: Boolean): Boolean
            public val active: Boolean
        }
        public open class Conditional<T> public constructor() {
            override interface Ready where T : ref {
                public override fun activate(value: Boolean): Boolean = value
                public override val active: Boolean get() = true
            }
        }
        public class Derived<T> public constructor() : Conditional<T>()
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
    for (header, available) in [
        ("fun editing(value: Conditional<String>)", true),
        ("fun editing(value: Conditional<Boolean>)", false),
        ("fun <T> editing(value: Conditional<T>)", false),
        ("fun <T: ref> editing(value: Conditional<T>)", true),
        ("fun editing(value: Derived<String>)", true),
        ("fun editing(value: Derived<Boolean>)", false),
    ] {
        for call in [false, true] {
            let suffix = if call { "activate(" } else { "" };
            let text = format!("{header} {{ value.{suffix}");
            let parsed = scoop_parser::parse_recovering(&text).unwrap();
            let source = scoop_parser::declaration_source(
                &parsed,
                &identity,
                scoop_identity::SourceRole::Production,
                &text,
            );
            let mut analysis =
                analyze_declarations(&[source], &core_inputs, &world, &[], || false).unwrap();
            assert!(
                analysis.diagnostics().is_empty(),
                "{text}: {:?}",
                analysis.diagnostics()
            );
            let function = function(&analysis, "editing");
            if call {
                let calls = analysis.incomplete_calls(function, || false).unwrap();
                assert_eq!(calls.len(), 1, "{text}: {calls:?}");
                let V::Known(candidates) = &calls[0].candidates else {
                    panic!("{text}: {calls:?}");
                };
                assert_eq!(
                    candidates.len(),
                    usize::from(available),
                    "{text}: {candidates:?}"
                );
                if let Some(candidate) = candidates.first() {
                    assert_eq!(candidate.parameters[0].type_label, "Boolean");
                }
            } else {
                let inputs = analysis.receivers(function, || false).unwrap();
                assert_eq!(inputs.len(), 1);
                let V::Known(members) = &inputs[0].members else {
                    panic!("{text}: known recovered members: {inputs:?}")
                };
                for name in ["activate", "active"] {
                    assert_eq!(
                        members
                            .candidates
                            .iter()
                            .any(|candidate| candidate.name == name),
                        available,
                        "{text}: {members:?}"
                    );
                }
            }
        }
    }
}
