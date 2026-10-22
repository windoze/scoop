use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_named_arguments_keep_the_original_parameter_after_alias_and_reordering() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "argument-hover", "1.0.0").unwrap();
    let provider = "public fun <T> choose(value: T, ignored: Boolean): T = value";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 197);
    let identity = crate::tests::test_source_identity("src/arguments.scoop");
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
    let text = "import choose as selected\nfun <Item> editing(input: Item): Item = selected(ignored = true, value = input)";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let input = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis = analyze_declarations(&[input], &protocols, &world, &[], || false).unwrap();
    for (name, index, expected) in [("value", 0, "Item"), ("ignored", 1, "Boolean")] {
        let argument = argument(&mut analysis, text, &format!("{name} ="), name);
        assert_eq!(argument.index, index);
        assert!(matches!(
            argument.callee,
            Some(hir::SemanticCallable::Imported(_))
        ));
        assert_eq!(
            type_name(&mut analysis, &argument, argument.parameter.value_type),
            expected
        );
        let origin = argument.definition.unwrap();
        assert_eq!(origin.source.cone(), coordinate.identity().unwrap());
        assert!(provider[origin.span.start as usize..origin.span.end as usize].contains(name));
    }
    assert!(analysis.reference_index.is_none());
}
