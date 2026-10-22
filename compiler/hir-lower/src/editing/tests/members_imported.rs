use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_members_and_generic_extensions_use_their_declared_scopes() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "editing-members", "1.0.0").unwrap();
    let provider = "public class Box<T>(public val item: T) { public fun read(): T = item\nprivate fun hidden(): Boolean = true }\npublic fun <T> Box<T>.extension(required: T): T = required\npublic val <T> Box<T>.property: T get() = item\npublic fun String.stringOnly(): String = this";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 185);
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
    let text = "import extension as renamed\nfun editing(value: Box<Boolean>) { value. }";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[source], &core_inputs, &world, &[]).unwrap();
    let members = members::members(&mut analysis);
    for name in ["item", "read", "extension", "renamed", "property"] {
        assert!(
            members.candidates.iter().any(|member| member.name == name),
            "missing {name}: {members:?}"
        );
    }
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| matches!(member.name.as_str(), "hidden" | "stringOnly")),
        "{members:?}"
    );
    assert!(members.unavailable.is_empty(), "{members:?}");
    let property = members
        .candidates
        .iter()
        .find(|member| member.name == "property")
        .unwrap();
    assert!(matches!(
        property.target,
        hir::EditingMemberTarget::ImportedProperty(_)
    ));
    let named = |name| {
        members
            .candidates
            .iter()
            .find(|member| member.name == name)
            .unwrap()
            .target
    };
    assert_eq!(named("extension"), named("renamed"));
}
